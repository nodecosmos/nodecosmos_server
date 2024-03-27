mod create;
mod delete;
mod update;
mod update_input_ids;
mod update_node_ids;
mod update_output_ids;

use crate::api::data::RequestData;
use crate::api::WorkflowParams;
use crate::errors::NodecosmosError;
use crate::models::branch::update::BranchUpdate;
use crate::models::branch::Branch;
use crate::models::flow::Flow;
use crate::models::traits::{Branchable, FindOrInsertBranchedFromParams, GroupById, Merge};
use crate::models::workflow::Workflow;
use charybdis::callbacks::Callbacks;
use charybdis::macros::charybdis_model;
use charybdis::model::AsNative;
use charybdis::operations::Find;
use charybdis::types::{Double, Frozen, List, Map, Timestamp, Uuid};
use futures::StreamExt;
use nodecosmos_macros::{Branchable, Id};
use scylla::CachingSession;
use serde::{Deserialize, Serialize};

#[charybdis_model(
    table_name = flow_steps,
    partition_keys = [node_id, branch_id],
    clustering_keys = [workflow_id, flow_id, flow_index, id],
    local_secondary_indexes = [id]
)]
#[derive(Branchable, Id, Serialize, Deserialize, Default, Clone)]
pub struct FlowStep {
    #[serde(rename = "nodeId")]
    #[branch(original_id)]
    pub node_id: Uuid,

    #[serde(rename = "branchId")]
    pub branch_id: Uuid,

    #[serde(rename = "workflowId")]
    pub workflow_id: Uuid,

    #[serde(rename = "flowId")]
    pub flow_id: Uuid,

    #[serde(default, rename = "flowIndex")]
    pub flow_index: Double,

    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,

    #[serde(rename = "nodeIds")]
    pub node_ids: Option<List<Uuid>>,

    #[serde(rename = "inputIdsByNodeId")]
    pub input_ids_by_node_id: Option<Frozen<Map<Uuid, Frozen<List<Uuid>>>>>,

    #[serde(rename = "outputIdsByNodeId")]
    pub output_ids_by_node_id: Option<Frozen<Map<Uuid, Frozen<List<Uuid>>>>>,

    #[serde(rename = "prevFlowStepId")]
    pub prev_flow_step_id: Option<Uuid>,

    #[serde(rename = "nextFlowStepId")]
    pub next_flow_step_id: Option<Uuid>,

    #[serde(rename = "createdAt")]
    pub created_at: Option<Timestamp>,

    #[serde(rename = "updatedAt")]
    pub updated_at: Option<Timestamp>,

    #[serde(skip)]
    #[charybdis(ignore)]
    pub workflow: Option<Workflow>,

    #[serde(skip)]
    #[charybdis(ignore)]
    pub prev_flow_step: Option<SiblingFlowStep>,

    #[serde(skip)]
    #[charybdis(ignore)]
    pub next_flow_step: Option<SiblingFlowStep>,
}

impl Callbacks for FlowStep {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_insert(&mut self, db_session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        self.set_defaults();
        self.validate_conflicts(db_session).await?;
        self.calculate_index(db_session).await?;
        self.sync_surrounding_fs_on_creation(data).await?;

        if self.is_branched() {
            let params = WorkflowParams {
                node_id: self.node_id,
                branch_id: self.branch_id,
            };

            Flow::find_or_insert_branched(db_session, &params, self.flow_id).await?;
            Workflow::find_or_insert_branched(db_session, &params, self.workflow_id).await?;

            Branch::update(data, self.branch_id, BranchUpdate::CreateFlowStep(self.id)).await?;
        }

        Ok(())
    }

    async fn before_delete(&mut self, _db_session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        self.pull_outputs_from_next_workflow_step(data).await?;
        self.delete_fs_outputs(data).await?;
        self.sync_surrounding_fs_on_del(data).await?;

        if self.is_branched() {
            Branch::update(data, self.branch_id, BranchUpdate::DeleteFlowStep(self.id)).await?;
        }

        Ok(())
    }

    async fn after_delete(&mut self, _db_session: &CachingSession, data: &RequestData) -> Result<(), Self::Error> {
        if self.is_branched() {
            self.create_branched_if_original_exists(data).await?;
        }

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
                    branched_flow_step.merge_inputs(&original_flow_step);
                    branched_flow_step.merge_nodes(&original_flow_step);
                    branched_flow_step.merge_outputs(&original_flow_step);
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

    pub fn merge_inputs(&mut self, original: &FlowStep) {
        self.input_ids_by_node_id.merge(original.input_ids_by_node_id.clone());
    }

    pub fn merge_nodes(&mut self, original: &FlowStep) {
        self.node_ids.merge_unique(original.node_ids.clone());
    }

    pub fn merge_outputs(&mut self, original: &FlowStep) {
        self.output_ids_by_node_id.merge(original.output_ids_by_node_id.clone());
    }

    pub async fn find_by_flow(
        db_session: &CachingSession,
        params: &WorkflowParams,
        workflow_id: Uuid,
        flow_id: Uuid,
    ) -> Result<Vec<FlowStep>, NodecosmosError> {
        if params.is_original() {
            return find_flow_step!(
                "node_id = ? AND branch_id = ? AND workflow_id = ? AND flow_id = ?",
                (params.node_id, params.branch_id, workflow_id, flow_id)
            )
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

    pub async fn workflow(&mut self, db_session: &CachingSession) -> Result<&mut Workflow, NodecosmosError> {
        if self.workflow.is_none() {
            let params = WorkflowParams {
                node_id: self.node_id,
                branch_id: self.branch_id,
            };

            let workflow = Workflow::branched(db_session, &params).await?;

            self.workflow = Some(workflow);
        }

        Ok(self.workflow.as_mut().unwrap())
    }

    pub async fn prev_flow_step(
        &mut self,
        db_session: &CachingSession,
    ) -> Result<Option<SiblingFlowStep>, NodecosmosError> {
        if let Some(prev_flow_step_id) = self.prev_flow_step_id {
            if self.prev_flow_step.is_none() {
                let res = SiblingFlowStep::find_or_insert_branched(
                    db_session,
                    &WorkflowParams {
                        node_id: self.node_id,
                        branch_id: self.branch_id,
                    },
                    prev_flow_step_id,
                )
                .await?;
                self.prev_flow_step = Some(res);
            }
        }

        Ok(self.prev_flow_step.clone())
    }

    pub async fn next_flow_step(
        &mut self,
        db_session: &CachingSession,
    ) -> Result<Option<SiblingFlowStep>, NodecosmosError> {
        if let Some(next_flow_step_id) = self.next_flow_step_id {
            if self.next_flow_step.is_none() {
                let res = SiblingFlowStep::find_or_insert_branched(
                    db_session,
                    &WorkflowParams {
                        node_id: self.node_id,
                        branch_id: self.branch_id,
                    },
                    next_flow_step_id,
                )
                .await?;
                self.next_flow_step = Some(res);
            }
        }

        Ok(self.next_flow_step.clone())
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

// SiblingFlowStep is same as FlowStep, but we use it to avoid recursive structs
// as we hold a reference to the next and previous flow steps in FlowStep
partial_flow_step!(
    SiblingFlowStep,
    node_id,
    branch_id,
    workflow_id,
    flow_id,
    flow_index,
    id,
    node_ids,
    input_ids_by_node_id,
    output_ids_by_node_id,
    prev_flow_step_id,
    next_flow_step_id,
    created_at,
    updated_at
);

partial_flow_step!(
    UpdateInputIdsFlowStep,
    node_id,
    branch_id,
    workflow_id,
    flow_id,
    flow_index,
    id,
    input_ids_by_node_id,
    updated_at
);

impl Callbacks for UpdateInputIdsFlowStep {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_update(&mut self, _db_session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        self.updated_at = Some(chrono::Utc::now());

        if self.is_branched() {
            FlowStep::find_or_insert_branched(
                data.db_session(),
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

partial_flow_step!(
    UpdateOutputIdsFlowStep,
    node_id,
    branch_id,
    workflow_id,
    flow_id,
    flow_index,
    id,
    output_ids_by_node_id,
    updated_at
);

impl Callbacks for UpdateOutputIdsFlowStep {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_update(&mut self, _db_session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        self.updated_at = Some(chrono::Utc::now());

        if self.is_branched() {
            FlowStep::find_or_insert_branched(
                data.db_session(),
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

partial_flow_step!(
    UpdateNodeIdsFlowStep,
    node_id,
    branch_id,
    workflow_id,
    flow_id,
    flow_index,
    id,
    node_ids,
    updated_at
);

impl Callbacks for UpdateNodeIdsFlowStep {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_update(&mut self, _db_session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        self.updated_at = Some(chrono::Utc::now());

        if self.is_branched() {
            FlowStep::find_or_insert_branched(
                data.db_session(),
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

    async fn after_update(&mut self, db_session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        let mut flow_step = self.as_native().find_by_primary_key().execute(db_session).await?;

        flow_step.delete_outputs_from_removed_nodes(data).await?;
        flow_step.remove_outputs_from_removed_nodes(data).await?;
        flow_step.remove_inputs_from_removed_nodes(data).await?;

        Ok(())
    }
}

partial_flow_step!(DeleteFlowStep, node_id, branch_id, workflow_id, flow_id, flow_index, id);
