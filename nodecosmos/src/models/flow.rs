use crate::models::helpers::{impl_default_callbacks, set_updated_at_cb};
use charybdis::{charybdis_model, partial_model_generator, Text, Timestamp, Uuid};
use chrono::Utc;

#[partial_model_generator]
#[charybdis_model(
    table_name = "flows",
    partition_keys = ["node_id", "workflow_id"],
    clustering_keys = ["id"],
    secondary_indexes = []
)]
pub struct Flow {
    #[serde(rename = "nodeId")]
    pub node_id: Uuid,

    #[serde(rename = "id")]
    pub id: Uuid,

    #[serde(rename = "workflowId")]
    pub workflow_id: Uuid,

    pub title: Text,
    pub description: Text,

    #[serde(rename = "createdAt")]
    pub created_at: Option<Timestamp>,

    #[serde(rename = "updatedAt")]
    pub updated_at: Option<Timestamp>,
}

impl_default_callbacks!(Flow);

partial_flow!(UpdateFlowTitle, node_id, workflow_id, id, title, updated_at);
set_updated_at_cb!(UpdateFlowTitle);

partial_flow!(
    UpdateFlowDescription,
    node_id,
    workflow_id,
    id,
    description,
    updated_at
);
set_updated_at_cb!(UpdateFlowDescription);
