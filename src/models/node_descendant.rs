use crate::errors::NodecosmosError;
use crate::models::node::Node;
use charybdis::macros::charybdis_model;
use charybdis::stream::CharybdisModelStream;
use charybdis::types::{Double, Text, Uuid};
use scylla::CachingSession;
use serde::{Deserialize, Serialize};

#[derive(Clone)]
#[charybdis_model(
    table_name = node_descendants,
    partition_keys = [root_id, branch_id],
    clustering_keys = [node_id, order_index, id],
    global_secondary_indexes = [],
    table_options = r#"
        gc_grace_seconds = 432000 AND
        compression = { 
            'sstable_compression': 'LZ4Compressor',
            'chunk_length_in_kb': '64kb'
        }
    "#,
)]
#[derive(Serialize, Deserialize, Default)]
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

impl NodeDescendant {
    pub async fn all_node_descendants(
        db_session: &CachingSession,
        node: &Node,
    ) -> Result<CharybdisModelStream<NodeDescendant>, NodecosmosError> {
        let all_descendants = find_node_descendant!(
            db_session,
            "root_id = ? AND branch_id in ? AND node_id = ?",
            (node.root_id, vec![node.id, node.branch_id], node.id)
        )
        .await?;

        Ok(all_descendants)
    }
}
