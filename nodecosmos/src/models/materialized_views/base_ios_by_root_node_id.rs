use charybdis::{Text, Uuid};
use charybdis_macros::charybdis_view_model;

#[charybdis_view_model(
    table_name=input_outputs_by_root_node_id,
    base_table=input_outputs,
    partition_keys=[root_node_id],
    clustering_keys=[node_id, workflow_id, id]
)]
pub struct InputOutputsByRootNodeId {
    #[serde(rename = "rootNodeId")]
    pub root_node_id: Uuid,

    #[serde(rename = "nodeId")]
    pub node_id: Option<Uuid>,

    #[serde(rename = "workflowId")]
    pub workflow_id: Option<Uuid>,

    #[serde(rename = "originalId")]
    pub original_id: Option<Uuid>,

    pub id: Option<Uuid>,
    pub title: Option<Text>,
    pub unit: Option<Text>,

    #[serde(rename = "dataType")]
    pub data_type: Option<Text>,
    pub value: Option<Text>,
}
