use charybdis::{Double, Text, Uuid};
use charybdis_macros::{charybdis_model, partial_model_generator};

#[derive(Clone)]
#[partial_model_generator]
#[charybdis_model(
    table_name = node_descendants,
    partition_keys = [root_id],
    clustering_keys = [node_id, order_index, id],
    secondary_indexes = [],
)]
pub struct NodeDescendant {
    #[serde(rename = "rootId")]
    pub root_id: Uuid,

    #[serde(rename = "nodeId")]
    pub node_id: Uuid,

    #[serde(rename = "order")]
    pub order_index: Double,

    pub id: Uuid,

    #[serde(rename = "parentId")]
    pub parent_id: Uuid,

    pub title: Text,
}
