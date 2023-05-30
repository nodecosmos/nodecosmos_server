use crate::models::flow::Flow;
use crate::models::helpers::{
    created_at_cb_fn, impl_empty_cb, impl_updated_at_cb, updated_at_cb_fn,
};
use charybdis::{
    charybdis_model, partial_model_generator, Callbacks, CharybdisError, Frozen, Int, List, Map,
    New, Set, Timestamp, Uuid,
};
use chrono::Utc;
use scylla::CachingSession;

#[partial_model_generator]
#[charybdis_model(
    table_name = flow_steps,
    partition_keys = [node_id, workflow_id],
    clustering_keys = [id],
    secondary_indexes = []
)]
pub struct FlowStep {
    #[serde(rename = "nodeId")]
    pub node_id: Uuid,

    #[serde(rename = "workflowId")]
    pub workflow_id: Uuid,

    #[serde(rename = "flowId")]
    pub flow_id: Uuid,

    pub step: Int,
    pub id: Uuid,

    #[serde(rename = "nodeIds")]
    pub node_ids: List<Uuid>,

    #[serde(rename = "inputIdsByNodeId")]
    pub input_ids_by_node_id: Option<Map<Uuid, Frozen<Set<Uuid>>>>,

    #[serde(rename = "outputIdsByNodeId")]
    pub output_ids_by_node_id: Option<Map<Uuid, Frozen<Set<Uuid>>>>,

    #[serde(rename = "createdAt")]
    pub created_at: Option<Timestamp>,

    #[serde(rename = "updatedAt")]
    pub updated_at: Option<Timestamp>,
}

impl Callbacks for FlowStep {
    created_at_cb_fn!();

    async fn after_insert(&mut self, session: &CachingSession) -> Result<(), CharybdisError> {
        let mut flow = Flow::new();

        flow.node_id = self.node_id;
        flow.workflow_id = self.workflow_id;
        flow.id = self.flow_id;

        flow.append_step(session, self.id).await?;

        Ok(())
    }

    updated_at_cb_fn!();

    async fn after_delete(&mut self, session: &CachingSession) -> Result<(), CharybdisError> {
        let mut flow = Flow::new();

        flow.node_id = self.node_id;
        flow.workflow_id = self.workflow_id;
        flow.id = self.flow_id;

        flow.remove_step(session, self.id).await?;

        Ok(())
    }
}

partial_flow_step!(
    UpdateFlowStepInputIds,
    node_id,
    workflow_id,
    flow_id,
    id,
    input_ids_by_node_id,
    updated_at
);
impl_updated_at_cb!(UpdateFlowStepInputIds);

partial_flow_step!(
    UpdateFlowStepOutputIds,
    node_id,
    workflow_id,
    flow_id,
    id,
    output_ids_by_node_id,
    updated_at
);
impl_updated_at_cb!(UpdateFlowStepOutputIds);

partial_flow_step!(DeleteFlowStep, node_id, workflow_id, flow_id, id);
impl_empty_cb!(DeleteFlowStep);
