use charybdis::macros::charybdis_model;
use charybdis::types::{Double, Text, Uuid};
use serde::{Deserialize, Serialize};

use nodecosmos_macros::{Id, RootId};

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
#[derive(Id, RootId, Serialize, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NodeDescendant {
    pub root_id: Uuid,
    pub branch_id: Uuid,
    pub node_id: Uuid,

    #[serde(rename = "order")]
    pub order_index: Double,

    pub id: Uuid,
    pub parent_id: Uuid,
    pub title: Text,
}
