use crate::models::node::Node;
use charybdis::macros::charybdis_model;
use charybdis::types::{Double, Set, Uuid};
use serde::{Deserialize, Serialize};

#[charybdis_model(
    table_name = node_tree_position_commits,
    partition_keys = [id],
    clustering_keys = [],
    table_options = r#"
        compression = { 
            'sstable_compression': 'DeflateCompressor'
        }
    "#
)]
#[derive(Serialize, Deserialize, Default)]
pub struct NodeTreePositionCommit {
    pub id: Uuid,

    #[serde(rename = "versionedNodeId")]
    pub order_index: Double,

    #[serde(rename = "versionedAncestorsId")]
    pub parent_id: Option<Uuid>,

    #[serde(rename = "versionedAncestorsId")]
    pub ancestor_ids: Option<Set<Uuid>>,
}

impl NodeTreePositionCommit {
    pub fn from_node(node: &Node) -> Self {
        Self {
            id: Uuid::new_v4(),
            order_index: node.order_index,
            parent_id: node.parent_id,
            ancestor_ids: node.ancestor_ids.clone(),
        }
    }

    pub fn init_from(vtp: &Self) -> Self {
        Self {
            id: Uuid::new_v4(),
            order_index: vtp.order_index,
            parent_id: vtp.parent_id,
            ancestor_ids: vtp.ancestor_ids.clone(),
        }
    }
}
