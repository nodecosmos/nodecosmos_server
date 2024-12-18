use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::archived_flow_step::ArchivedFlowStep;
use crate::models::traits::{
    Branchable, FindOrInsertBranched, GroupById, Merge, ModelBranchParams, NodeBranchParams, WhereInChunksExec,
};
use crate::models::traits::{Context, ModelContext};
use crate::models::utils::updated_at_cb_fn;
use charybdis::callbacks::Callbacks;
use charybdis::macros::charybdis_model;
use charybdis::operations::{Find, Insert, UpdateWithCallbacks};
use charybdis::types::{Decimal, Frozen, List, Map, Set, Timestamp, Uuid};
use futures::StreamExt;
use macros::{Branchable, FlowId, Id, NodeId};
use scylla::CachingSession;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

mod create;
mod delete;
mod update;
mod update_input_ids;
mod update_node_ids;
mod update_output_ids;

#[charybdis_model(
    table_name = flow_steps,
    partition_keys = [branch_id],
    clustering_keys = [node_id, flow_id, step_index, id],
    local_secondary_indexes = [id]
)]
#[derive(Branchable, Id, NodeId, FlowId, Serialize, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FlowStep {
    pub node_id: Uuid,

    pub branch_id: Uuid,
    pub flow_id: Uuid,

    #[serde(default)]
    pub step_index: Decimal,

    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,

    #[branch(original_id)]
    pub root_id: Uuid,

    pub node_ids: Option<List<Uuid>>,
    pub input_ids_by_node_id: Option<Frozen<Map<Uuid, Frozen<List<Uuid>>>>>,
    pub output_ids_by_node_id: Option<Frozen<Map<Uuid, Frozen<List<Uuid>>>>>,

    #[serde(default = "chrono::Utc::now")]
    pub created_at: Timestamp,

    #[serde(default = "chrono::Utc::now")]
    pub updated_at: Timestamp,

    #[charybdis(ignore)]
    #[serde(skip)]
    pub ctx: Context,
}

impl Callbacks for FlowStep {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_insert(&mut self, _db_session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_default_context() {
            self.set_defaults();
            self.validate_no_conflicts(data).await?;
            self.update_branch_with_creation(data).await?;
        }

        if self.is_default_context() || self.is_branch_init_context() {
            self.preserve_branch_flow(data).await?;
        }

        Ok(())
    }

    updated_at_cb_fn!();

    async fn before_delete(&mut self, _db_session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        self.delete_outputs(data).await?;
        self.preserve_branch_flow(data).await?;
        self.update_branch_with_deletion(data).await?;

        Ok(())
    }

    async fn after_delete(&mut self, _db_session: &CachingSession, data: &RequestData) -> Result<(), Self::Error> {
        // TODO: see nodecosmos/src/models/node/create.rs:258
        self.create_branched_if_original_exists(data).await?;

        let _ = ArchivedFlowStep::from(&*self)
            .insert()
            .execute(data.db_session())
            .await
            .map_err(|e| {
                log::error!("[after_delete] Failed to insert archived flow step: {:?}", e);
                e
            });

        Ok(())
    }
}

type IoIdsByNodeList = Frozen<Map<Uuid, Frozen<List<Uuid>>>>;
type IoIdsByNodeSet = Frozen<Map<Uuid, Frozen<Set<Uuid>>>>;

impl FlowStep {
    pub async fn branched(
        db_session: &CachingSession,
        params: &NodeBranchParams,
    ) -> Result<Vec<FlowStep>, NodecosmosError> {
        let flow_steps = Self::find_by_branch_id_and_node_id(params.branch_id, params.node_id)
            .execute(db_session)
            .await?;

        if params.is_original() {
            Ok(flow_steps.try_collect().await?)
        } else {
            let mut original_flow_steps = Self::find_by_branch_id_and_node_id(params.original_id(), params.node_id)
                .execute(db_session)
                .await?;
            let mut branch_flow_steps = flow_steps.group_by_id().await?;

            while let Some(original_flow_step) = original_flow_steps.next().await {
                let mut original_flow_step = original_flow_step?;
                if let Some(branched_flow_step) = branch_flow_steps.get_mut(&original_flow_step.id) {
                    branched_flow_step.merge_original_inputs(&original_flow_step);
                    branched_flow_step.merge_original_nodes(&original_flow_step);
                    branched_flow_step.merge_original_outputs(&original_flow_step);
                } else {
                    original_flow_step.branch_id = params.branch_id;
                    branch_flow_steps.insert(original_flow_step.id, original_flow_step);
                }
            }

            let mut branch_flow_steps = branch_flow_steps.into_values().collect::<Vec<FlowStep>>();

            branch_flow_steps.sort_by(|a, b| {
                a.step_index
                    .partial_cmp(&b.step_index)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            Ok(branch_flow_steps)
        }
    }

    pub async fn find_by_flow(
        db_session: &CachingSession,
        params: &NodeBranchParams,
        flow_id: Uuid,
    ) -> Result<Vec<FlowStep>, NodecosmosError> {
        if params.is_original() {
            return FlowStep::find_by_branch_id_and_node_id_and_flow_id(params.branch_id, params.node_id, flow_id)
                .execute(db_session)
                .await?
                .try_collect()
                .await
                .map_err(NodecosmosError::from);
        }

        let flow_steps = FlowStep::branched(db_session, params)
            .await?
            .into_iter()
            .filter(|flow_step| flow_step.flow_id == flow_id)
            .collect::<Vec<FlowStep>>();

        Ok(flow_steps)
    }

    pub async fn find_by_branch_id_and_node_id_and_ids(
        db_session: &CachingSession,
        branch_id: Uuid,
        node_id: Uuid,
        ids: &Set<Uuid>,
    ) -> Result<Vec<FlowStep>, NodecosmosError> {
        let flow_steps = ids
            .where_in_chunked_query(db_session, |chunk| {
                find_flow_step!(
                    "branch_id = ? AND node_id = ? AND id IN ? ALLOW FILTERING",
                    (branch_id, node_id, chunk)
                )
            })
            .await
            .try_collect()
            .await?;

        Ok(flow_steps)
    }

    pub fn ios_diff(current_db_ios: Option<IoIdsByNodeList>, new_ios: &Option<IoIdsByNodeList>) -> [IoIdsByNodeSet; 2] {
        let mut created_io_ids_by_node_id = HashMap::new();
        let mut removed_io_ids_by_node_id = HashMap::new();

        match (current_db_ios, &new_ios) {
            (Some(current_db_ios), Some(new_node_ios)) => {
                // handle new ios that are not in current db ios
                for (node_id, new_io_ids) in new_node_ios {
                    if !current_db_ios.contains_key(node_id) {
                        created_io_ids_by_node_id.insert(*node_id, new_io_ids.clone().into_iter().collect());
                    }
                }

                // handle current db ios delta
                for (node_id, current_db_io_ids) in current_db_ios {
                    match new_node_ios.get(&node_id) {
                        Some(new_io_ids) => {
                            // Calculate created and removed ios.
                            let created: HashSet<Uuid> = new_io_ids
                                .iter()
                                .filter(|id| !current_db_io_ids.contains(id))
                                .cloned()
                                .collect();

                            let removed: HashSet<Uuid> = current_db_io_ids
                                .iter()
                                .filter(|id| !new_io_ids.contains(id))
                                .cloned()
                                .collect();

                            if !created.is_empty() {
                                created_io_ids_by_node_id.insert(node_id, created);
                            }

                            if !removed.is_empty() {
                                removed_io_ids_by_node_id.insert(node_id, removed);
                            }
                        }
                        None => {
                            // All original ios for this node_id are considered removed.
                            removed_io_ids_by_node_id.insert(node_id, current_db_io_ids.into_iter().collect());
                        }
                    }
                }
            }
            (None, Some(new_node_ios)) => {
                // All new_io_ids ios are considered created.
                for (node_id, new_io_ids) in new_node_ios {
                    created_io_ids_by_node_id.insert(*node_id, new_io_ids.clone().into_iter().collect());
                }
            }
            (Some(current_db_ios), None) => {
                // All current_db ios are considered removed.
                for (node_id, current_db_io_ids) in current_db_ios {
                    removed_io_ids_by_node_id.insert(node_id, current_db_io_ids.into_iter().collect());
                }
            }
            (None, None) => {
                // No ios to update.
            }
        }

        [created_io_ids_by_node_id, removed_io_ids_by_node_id]
    }
}

partial_flow_step!(
    UpdateInputIdsFlowStep,
    node_id,
    branch_id,
    flow_id,
    step_index,
    id,
    root_id,
    input_ids_by_node_id,
    updated_at,
    ctx
);

impl Callbacks for UpdateInputIdsFlowStep {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_update(&mut self, _db_session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        self.updated_at = chrono::Utc::now();

        if self.is_branch() {
            self.update_branch(data).await?;
        }

        self.update_ios(data).await?;

        Ok(())
    }
}

impl UpdateInputIdsFlowStep {
    pub fn append_inputs(&mut self, inputs: &HashMap<Uuid, Vec<Uuid>>) {
        self.input_ids_by_node_id.merge_unique(Some(inputs.clone()));
    }

    pub fn remove_inputs(&mut self, ids: &HashMap<Uuid, Vec<Uuid>>) {
        if let Some(input_ids_by_node_id) = &mut self.input_ids_by_node_id {
            for (node_id, input_ids) in input_ids_by_node_id.iter_mut() {
                if let Some(ids) = ids.get(node_id) {
                    input_ids.retain(|input_id| !ids.contains(input_id));
                }
            }
        }
    }
}

partial_flow_step!(
    UpdateNodeIdsFlowStep,
    node_id,
    branch_id,
    flow_id,
    step_index,
    id,
    root_id,
    node_ids,
    output_ids_by_node_id,
    input_ids_by_node_id,
    updated_at,
    ctx
);

impl Callbacks for UpdateNodeIdsFlowStep {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_update(&mut self, _db_session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        self.updated_at = chrono::Utc::now();

        let fs = FlowStep::find_or_insert_branched(
            data,
            ModelBranchParams {
                original_id: self.original_id(),
                branch_id: self.branch_id,
                id: self.id,
            },
        )
        .await?;

        self.output_ids_by_node_id = fs.output_ids_by_node_id;
        self.input_ids_by_node_id = fs.input_ids_by_node_id;

        if self.is_branch() {
            self.update_branch(data).await?;
        } else {
            self.delete_output_records_from_removed_nodes(data).await?;
            self.remove_output_references_from_removed_nodes().await?;
            self.remove_input_references_from_removed_nodes().await?;
        }

        Ok(())
    }
}

impl UpdateNodeIdsFlowStep {
    pub fn append_nodes(&mut self, ids: &[Uuid]) {
        self.node_ids.merge_unique(Some(ids.to_owned()));
    }

    pub fn remove_nodes(&mut self, ids: &[Uuid]) {
        if let Some(node_ids) = &mut self.node_ids {
            node_ids.retain(|node_id| !ids.contains(node_id));
        }
    }
}

partial_flow_step!(
    UpdateOutputIdsFlowStep,
    node_id,
    branch_id,
    root_id,
    flow_id,
    step_index,
    id,
    output_ids_by_node_id,
    updated_at,
    ctx
);

impl Callbacks for UpdateOutputIdsFlowStep {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_update(&mut self, _db_session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        self.updated_at = chrono::Utc::now();

        if self.is_branch() {
            self.update_branch(data).await?;
        }

        Ok(())
    }
}

impl UpdateOutputIdsFlowStep {
    pub async fn push_output(
        &mut self,
        data: &RequestData,
        node_id: Uuid,
        output_id: Uuid,
    ) -> Result<(), NodecosmosError> {
        let mut map = HashMap::new();
        map.insert(node_id, vec![output_id]);

        self.output_ids_by_node_id.merge_unique(Some(map));

        self.update_cb(data).execute(data.db_session()).await?;

        Ok(())
    }

    pub async fn pull_output(
        &mut self,
        data: &RequestData,
        node_id: Uuid,
        output_id: Uuid,
    ) -> Result<(), NodecosmosError> {
        if let Some(output_ids_by_node_id) = &mut self.output_ids_by_node_id {
            if let Some(output_ids) = output_ids_by_node_id.get_mut(&node_id) {
                output_ids.retain(|id| id != &output_id);
            }
        }

        self.update_cb(data).execute(data.db_session()).await?;

        Ok(())
    }
}

partial_flow_step!(PkFlowStep, node_id, branch_id, root_id, flow_id, step_index, id, created_at);

impl PkFlowStep {
    pub async fn maybe_find_by_index(self, db_session: &CachingSession) -> Result<Option<Self>, NodecosmosError> {
        let fs = PkFlowStep::maybe_find_first(
            find_pk_flow_step_query!(
                r#"
                    branch_id = ?
                        AND node_id = ?
                        AND flow_id = ?
                        AND step_index = ?
                    LIMIT 1

                "#
            ),
            (self.branch_id, self.node_id, self.flow_id, self.step_index),
        )
        .execute(db_session)
        .await?;

        Ok(fs)
    }
}

impl From<&FlowStep> for PkFlowStep {
    fn from(fs: &FlowStep) -> Self {
        Self {
            root_id: fs.root_id,
            node_id: fs.node_id,
            branch_id: fs.branch_id,
            flow_id: fs.flow_id,
            step_index: fs.step_index.clone(),
            id: fs.id,
            created_at: fs.created_at,
        }
    }
}
