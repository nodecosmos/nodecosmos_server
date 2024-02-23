use charybdis::macros::charybdis_model;
use charybdis::types::{Double, Text, Uuid};
use serde::{Deserialize, Serialize};

#[charybdis_model(
    table_name = node_descendants,
    partition_keys = [root_id],
    clustering_keys = [branch_id, node_id, order_index, id],
    table_options = r#"
        gc_grace_seconds = 432000 AND
        compression = { 
            'sstable_compression': 'LZ4Compressor',
            'chunk_length_in_kb': '64kb'
        }
    "#,
)]
#[derive(Serialize, Deserialize, Default, Clone)]
pub struct NodeDescendant {
    #[serde(rename = "rootId")]
    pub root_id: Uuid,

    #[serde(rename = "branchId")]
    pub branch_id: Uuid,

    #[serde(rename = "nodeId")]
    pub node_id: Uuid,

    #[serde(rename = "order")]
    pub order_index: Double,

    pub id: Uuid,

    #[serde(rename = "parentId")]
    pub parent_id: Uuid,

    pub title: Text,
}
