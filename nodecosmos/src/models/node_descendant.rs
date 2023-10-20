use charybdis::macros::charybdis_model;
use charybdis::types::{Double, Text, Uuid};
use serde::{Deserialize, Serialize};

#[derive(Clone)]
#[charybdis_model(
    table_name = node_descendants,
    partition_keys = [root_id],
    clustering_keys = [node_id, order_index, id],
    global_secondary_indexes = [],
    table_options = r#"
        gc_grace_seconds = 432000
    "#,
)]
#[derive(Serialize, Deserialize, Default)]
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
