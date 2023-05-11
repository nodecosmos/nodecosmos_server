use crate::models::helpers::{impl_default_callbacks, set_updated_at_cb};
use charybdis::{
    charybdis_model, partial_model_generator, Frozen, Int, List, Map, Set, Timestamp, Uuid,
};
use chrono::Utc;

#[partial_model_generator]
#[charybdis_model(
    table_name = "flow_steps",
    partition_keys = ["node_id", "workflow_id", "flow_id"],
    clustering_keys = ["step"],
    secondary_indexes = ["id"]
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

impl_default_callbacks!(FlowStep);

partial_flow_step!(
    UpdateFlowInputIds,
    node_id,
    workflow_id,
    flow_id,
    step,
    input_ids_by_node_id,
    updated_at
);
set_updated_at_cb!(UpdateFlowInputIds);

partial_flow_step!(
    UpdateFlowOutputIds,
    node_id,
    workflow_id,
    flow_id,
    step,
    output_ids_by_node_id,
    updated_at
);
set_updated_at_cb!(UpdateFlowOutputIds);
