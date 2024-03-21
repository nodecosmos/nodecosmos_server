mod callbacks;
mod create;
mod delete;
mod update;
mod update_input_ids;
mod update_node_ids;
mod update_output_ids;

use crate::api::WorkflowParams;
use crate::errors::NodecosmosError;
use crate::models::traits::Branchable;
use crate::models::workflow::Workflow;
use charybdis::macros::charybdis_model;
use charybdis::operations::Find;
use charybdis::stream::CharybdisModelStream;
use charybdis::types::{Double, Frozen, List, Map, Text, Timestamp, Uuid};
use futures::StreamExt;
use nodecosmos_macros::BranchableNodeId;
use scylla::CachingSession;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[charybdis_model(
    table_name = flow_steps,
    partition_keys = [node_id, branch_id],
    clustering_keys = [workflow_id, flow_id, flow_index, id],
    local_secondary_indexes = [id]
)]
#[derive(BranchableNodeId, Serialize, Deserialize, Default, Clone)]
pub struct FlowStep {
    #[serde(rename = "nodeId")]
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

    pub description: Option<Text>,

    #[serde(rename = "descriptionMarkdown")]
    pub description_markdown: Option<Text>,

    #[serde(rename = "descriptionBase64")]
    pub description_base64: Option<Text>,

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
    pub prev_flow_step: Option<BaseFlowStep>,

    #[serde(skip)]
    #[charybdis(ignore)]
    pub next_flow_step: Option<BaseFlowStep>,
}

impl FlowStep {
    pub async fn branched(
        db_session: &CachingSession,
        params: &WorkflowParams,
    ) -> Result<Vec<FlowStep>, NodecosmosError> {
        let mut flow_steps = Self::find_by_node_id_and_branch_id(params.node_id, params.branch_id)
            .execute(db_session)
            .await?;

        if params.is_original() {
            Ok(flow_steps.try_collect().await?)
        } else {
            let mut original_flow_steps = Self::find_by_node_id_and_branch_id(params.node_id, params.node_id)
                .execute(db_session)
                .await?;
            let mut branched_flow_steps_set = HashSet::new();
            let mut branch_flow_steps = vec![];

            while let Some(flow_step) = flow_steps.next().await {
                let flow_step = flow_step?;
                branched_flow_steps_set.insert(flow_step.id);
                branch_flow_steps.push(flow_step);
            }

            while let Some(flow_step) = original_flow_steps.next().await {
                let flow_step = flow_step?;
                if !branched_flow_steps_set.contains(&flow_step.id) {
                    branch_flow_steps.push(flow_step);
                }
            }

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
        node_id: Uuid,
        branch_id: Uuid,
        workflow_id: Uuid,
        flow_id: Uuid,
    ) -> Result<CharybdisModelStream<FlowStep>, NodecosmosError> {
        let res = find_flow_step!(
            "node_id = ? AND branch_id = ? AND workflow_id = ? AND flow_id = ?",
            (node_id, branch_id, workflow_id, flow_id)
        )
        .execute(db_session)
        .await?;

        Ok(res)
    }

    pub async fn find_by_node_id_and_branch_id_and_id(
        db_session: &CachingSession,
        node_id: Uuid,
        branch_id: Uuid,
        id: Uuid,
    ) -> Result<FlowStep, NodecosmosError> {
        let fs = find_first_flow_step!("node_id = ? AND branch_id = ? AND id = ?", (node_id, branch_id, id))
            .execute(db_session)
            .await?;

        Ok(fs)
    }

    pub(crate) async fn workflow(
        &mut self,
        db_session: &CachingSession,
    ) -> Result<&mut Option<Workflow>, NodecosmosError> {
        if self.workflow.is_none() {
            let workflow =
                Workflow::find_by_node_id_and_branch_id_and_id(self.node_id, self.branch_id, self.workflow_id)
                    .execute(db_session)
                    .await?;
            self.workflow = Some(workflow);
        }

        Ok(&mut self.workflow)
    }

    pub async fn prev_flow_step(
        &mut self,
        db_session: &CachingSession,
    ) -> Result<Option<BaseFlowStep>, NodecosmosError> {
        if let Some(prev_flow_step_id) = self.prev_flow_step_id {
            if self.prev_flow_step.is_none() {
                let res = BaseFlowStep::find_by_node_id_and_branch_id_and_id(
                    db_session,
                    self.node_id,
                    self.branch_id,
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
    ) -> Result<Option<BaseFlowStep>, NodecosmosError> {
        if let Some(next_flow_step_id) = self.next_flow_step_id {
            if self.next_flow_step.is_none() {
                let res = BaseFlowStep::find_by_node_id_and_branch_id_and_id(
                    db_session,
                    self.node_id,
                    self.branch_id,
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

partial_flow_step!(
    BaseFlowStep,
    node_id,
    branch_id,
    workflow_id,
    flow_id,
    flow_index,
    id,
    description,
    description_markdown,
    description_base64,
    node_ids,
    input_ids_by_node_id,
    output_ids_by_node_id,
    prev_flow_step_id,
    next_flow_step_id,
    created_at,
    updated_at
);

impl BaseFlowStep {
    pub async fn find_by_node_id_and_branch_id_and_id(
        db_session: &CachingSession,
        node_id: Uuid,
        branch_id: Uuid,
        id: Uuid,
    ) -> Result<BaseFlowStep, NodecosmosError> {
        let fs = find_first_base_flow_step!("node_id = ? AND branch_id = ? AND id = ?", (node_id, branch_id, id))
            .execute(db_session)
            .await?;

        Ok(fs)
    }
}

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

partial_flow_step!(
    UpdateDescriptionFlowStep,
    node_id,
    branch_id,
    workflow_id,
    flow_id,
    flow_index,
    id,
    description,
    description_markdown,
    updated_at
);

partial_flow_step!(DeleteFlowStep, node_id, branch_id, workflow_id, flow_id, flow_index, id);
