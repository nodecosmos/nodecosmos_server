mod callbacks;
mod create;
mod delete;
mod update;

use crate::errors::NodecosmosError;
use crate::models::workflow::Workflow;
use charybdis::macros::charybdis_model;
use charybdis::types::{Double, Frozen, List, Map, Text, Timestamp, Uuid};
use scylla::CachingSession;
use serde::{Deserialize, Serialize};
use std::rc::Rc;

#[charybdis_model(
    table_name = flow_steps,
    partition_keys = [node_id],
    clustering_keys = [workflow_id, flow_id, flow_index, id],
    global_secondary_indexes = [],
    local_secondary_indexes = [
        ([node_id], [id])
    ]
)]
#[derive(Serialize, Deserialize, Default, Clone)]
pub struct FlowStep {
    #[serde(rename = "nodeId")]
    pub node_id: Uuid,

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
    pub async fn find_by_node_id_and_id(
        session: &CachingSession,
        node_id: Uuid,
        id: Uuid,
    ) -> Result<FlowStep, NodecosmosError> {
        let fs = find_first_flow_step!(session, "node_id = ? AND id = ?", (node_id, id)).await?;

        Ok(fs)
    }

    pub(crate) async fn workflow(
        &mut self,
        session: &CachingSession,
    ) -> Result<&mut Option<Workflow>, NodecosmosError> {
        if self.workflow.is_none() {
            let workflow = Workflow::by_node_id_and_id(session, self.node_id, self.workflow_id).await?;
            self.workflow = Some(workflow);
        }

        Ok(&mut self.workflow)
    }

    pub async fn prev_flow_step(&mut self, session: &CachingSession) -> Result<Option<BaseFlowStep>, NodecosmosError> {
        if let Some(prev_flow_step_id) = self.prev_flow_step_id {
            if self.prev_flow_step.is_none() {
                let res = BaseFlowStep::find_by_node_id_and_id(session, self.node_id, prev_flow_step_id).await?;
                self.prev_flow_step = Some(res);
            }
        }

        Ok(self.prev_flow_step.clone())
    }

    pub async fn next_flow_step(&mut self, session: &CachingSession) -> Result<Option<BaseFlowStep>, NodecosmosError> {
        if let Some(next_flow_step_id) = self.next_flow_step_id {
            if self.next_flow_step.is_none() {
                let res = BaseFlowStep::find_by_node_id_and_id(session, self.node_id, next_flow_step_id).await?;
                self.next_flow_step = Some(res);
            }
        }

        Ok(self.next_flow_step.clone())
    }
}

partial_flow_step!(
    BaseFlowStep,
    node_id,
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
    pub async fn find_by_node_id_and_id(
        session: &CachingSession,
        node_id: Uuid,
        id: Uuid,
    ) -> Result<BaseFlowStep, NodecosmosError> {
        let fs = find_first_base_flow_step!(session, "node_id = ? AND id = ?", (node_id, id)).await?;

        Ok(fs)
    }
}

partial_flow_step!(
    UpdateInputIdsFlowStep,
    node_id,
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
    workflow_id,
    flow_id,
    flow_index,
    id,
    description,
    description_markdown,
    updated_at
);

partial_flow_step!(DeleteFlowStep, node_id, workflow_id, flow_id, flow_index, id);
