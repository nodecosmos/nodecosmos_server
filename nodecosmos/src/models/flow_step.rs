use std::collections::HashMap;

use charybdis::callbacks::Callbacks;
use charybdis::macros::charybdis_model;
use charybdis::operations::Find;
use charybdis::types::{Double, Frozen, List, Map, Set, Timestamp, Uuid};
use futures::StreamExt;
use scylla::CachingSession;
use serde::{Deserialize, Serialize};

use nodecosmos_macros::{Branchable, FlowId, Id};

use crate::api::data::RequestData;
use crate::api::WorkflowParams;
use crate::errors::NodecosmosError;
use crate::models::branch::update::BranchUpdate;
use crate::models::branch::Branch;
use crate::models::traits::{Branchable, FindOrInsertBranchedFromParams, GroupById, Merge};
use crate::models::traits::{Context, ModelContext};
use crate::models::utils::updated_at_cb_fn;

mod create;
mod delete;
mod update;
mod update_input_ids;
mod update_node_ids;
mod update_output_ids;

#[charybdis_model(
    table_name = flow_steps,
    partition_keys = [node_id, branch_id],
    clustering_keys = [flow_id, flow_index, id],
    local_secondary_indexes = [id]
)]
#[derive(Branchable, Id, FlowId, Serialize, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FlowStep {
    #[branch(original_id)]
    pub node_id: Uuid,

    pub branch_id: Uuid,
    pub flow_id: Uuid,

    #[serde(default)]
    pub flow_index: Double,

    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,

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

        if self.is_default_context() || self.is_branched_init_context() {
            self.preserve_branch_flow(data).await?;
        }

        Ok(())
    }

    updated_at_cb_fn!();

    async fn before_delete(&mut self, _db_session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        self.delete_fs_outputs(data).await?;
        self.preserve_branch_flow(data).await?;
        self.update_branch_with_deletion(data).await?;

        Ok(())
    }

    async fn after_delete(&mut self, _db_session: &CachingSession, data: &RequestData) -> Result<(), Self::Error> {
        self.create_branched_if_original_exists(data).await?;

        Ok(())
    }
}

impl FlowStep {
    pub async fn branched(
        db_session: &CachingSession,
        params: &WorkflowParams,
    ) -> Result<Vec<FlowStep>, NodecosmosError> {
        let flow_steps = Self::find_by_node_id_and_branch_id(params.node_id, params.branch_id)
            .execute(db_session)
            .await?;

        if params.is_original() {
            Ok(flow_steps.try_collect().await?)
        } else {
            let mut original_flow_steps = Self::find_by_node_id_and_branch_id(params.node_id, params.node_id)
                .execute(db_session)
                .await?;
            let mut branch_flow_steps = flow_steps.group_by_id().await?;

            while let Some(original_flow_step) = original_flow_steps.next().await {
                let mut original_flow_step = original_flow_step?;
                if let Some(branched_flow_step) = branch_flow_steps.get_mut(&original_flow_step.id) {
                    original_flow_step.merge_original_inputs(&branched_flow_step);
                    original_flow_step.merge_original_nodes(&branched_flow_step);
                    original_flow_step.merge_original_outputs(&branched_flow_step);

                    branched_flow_step.input_ids_by_node_id = original_flow_step.input_ids_by_node_id;
                    branched_flow_step.node_ids = original_flow_step.node_ids;
                    branched_flow_step.output_ids_by_node_id = original_flow_step.output_ids_by_node_id;
                } else {
                    original_flow_step.branch_id = params.branch_id;
                    branch_flow_steps.insert(original_flow_step.id, original_flow_step);
                }
            }

            let mut branch_flow_steps = branch_flow_steps.into_values().collect::<Vec<FlowStep>>();

            branch_flow_steps.sort_by(|a, b| {
                a.flow_index
                    .partial_cmp(&b.flow_index)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            Ok(branch_flow_steps)
        }
    }

    pub async fn find_by_flow(
        db_session: &CachingSession,
        params: &WorkflowParams,
        flow_id: Uuid,
    ) -> Result<Vec<FlowStep>, NodecosmosError> {
        if params.is_original() {
            return FlowStep::find_by_node_id_and_branch_id_and_flow_id(params.node_id, params.branch_id, flow_id)
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

    pub async fn find_by_node_id_and_branch_id_and_ids(
        db_session: &CachingSession,
        node_id: Uuid,
        branch_id: Uuid,
        ids: &Set<Uuid>,
    ) -> Result<Vec<FlowStep>, NodecosmosError> {
        let flow_steps = find_flow_step!(
            "node_id = ? AND branch_id = ? AND id IN ? ALLOW FILTERING",
            (node_id, branch_id, ids)
        )
        .execute(db_session)
        .await?
        .try_collect()
        .await?;

        Ok(flow_steps)
    }

    pub async fn maybe_find_by_flow_index(
        &self,
        db_session: &CachingSession,
    ) -> Result<Option<FlowStep>, NodecosmosError> {
        let q = find_flow_step_query!("node_id = ? AND branch_id = ? AND flow_id = ? AND flow_index = ? LIMIT 1");
        let fs = FlowStep::maybe_find_first(q, (self.node_id, self.branch_id, self.flow_id, self.flow_index))
            .execute(db_session)
            .await?;

        Ok(fs)
    }

    pub async fn maybe_find_original(
        &mut self,
        db_session: &CachingSession,
    ) -> Result<Option<FlowStep>, NodecosmosError> {
        let original = Self {
            branch_id: self.original_id(),
            ..self.clone()
        }
        .maybe_find_by_primary_key()
        .execute(db_session)
        .await?;

        Ok(original)
    }
}

partial_flow_step!(
    UpdateInputIdsFlowStep,
    node_id,
    branch_id,
    flow_id,
    flow_index,
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
        if self.is_branched() {
            Branch::update(data, self.branch_id, BranchUpdate::EditNodeWorkflow(self.node_id)).await?;
            FlowStep::find_or_insert_branched(
                data,
                &WorkflowParams {
                    node_id: self.node_id,
                    branch_id: self.branch_id,
                },
                self.id,
            )
            .await?;

            self.update_branch(data).await?;
            self.preserve_branch_ios(data).await?;
            self.update_ios(data).await?;
        }

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
    flow_index,
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
        if self.is_branched() {
            FlowStep::find_or_insert_branched(
                data,
                &WorkflowParams {
                    node_id: self.node_id,
                    branch_id: self.branch_id,
                },
                self.id,
            )
            .await?;
        }

        let current = self.find_by_primary_key().execute(data.db_session()).await?;
        self.output_ids_by_node_id = current.output_ids_by_node_id;
        self.input_ids_by_node_id = current.input_ids_by_node_id;

        self.delete_output_records_from_removed_nodes(data).await?;
        self.remove_output_references_from_removed_nodes().await?;
        self.remove_input_references_from_removed_nodes().await?;

        if self.is_branched() {
            Branch::update(data, self.branch_id, BranchUpdate::EditNodeWorkflow(self.node_id)).await?;
            self.update_branch(data).await?;
        }

        Ok(())
    }
}

impl UpdateNodeIdsFlowStep {
    pub fn append_nodes(&mut self, ids: &Vec<Uuid>) {
        self.node_ids.merge_unique(Some(ids.clone()));
    }

    pub fn remove_nodes(&mut self, ids: &Vec<Uuid>) {
        if let Some(node_ids) = &mut self.node_ids {
            node_ids.retain(|node_id| !ids.contains(node_id));
        }
    }
}

partial_flow_step!(
    UpdateOutputIdsFlowStep,
    node_id,
    branch_id,
    flow_id,
    flow_index,
    id,
    output_ids_by_node_id,
    updated_at,
    ctx
);

impl Callbacks for UpdateOutputIdsFlowStep {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_update(&mut self, _db_session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_branched() {
            Branch::update(data, self.branch_id, BranchUpdate::EditNodeWorkflow(self.node_id)).await?;
            FlowStep::find_or_insert_branched(
                data,
                &WorkflowParams {
                    node_id: self.node_id,
                    branch_id: self.branch_id,
                },
                self.id,
            )
            .await?;

            self.update_branch(data).await?;
        }

        Ok(())
    }
}

impl UpdateOutputIdsFlowStep {
    pub fn append_outputs(&mut self, outputs: &HashMap<Uuid, Vec<Uuid>>) {
        self.output_ids_by_node_id.merge_unique(Some(outputs.clone()));
    }

    pub fn remove_outputs(&mut self, ids: &HashMap<Uuid, Vec<Uuid>>) {
        if let Some(output_ids_by_node_id) = &mut self.output_ids_by_node_id {
            for (node_id, output_ids) in output_ids_by_node_id.iter_mut() {
                if let Some(ids) = ids.get(node_id) {
                    output_ids.retain(|output_id| !ids.contains(output_id));
                }
            }
        }
    }
}

partial_flow_step!(DeleteFlowStep, node_id, branch_id, flow_id, flow_index, id);
