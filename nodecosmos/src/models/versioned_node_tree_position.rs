use crate::models::node::Node;
use charybdis::macros::charybdis_model;
use charybdis::types::{Double, Set, Uuid};
use serde::{Deserialize, Serialize};

#[charybdis_model(
    table_name = versioned_node_tree_position,
    partition_keys = [id],
    clustering_keys = []
)]
#[derive(Serialize, Deserialize, Default, Clone)]
pub struct VersionedNodeTreePosition {
    pub id: Uuid,

    #[serde(rename = "versionedNodeId")]
    pub order_index: Double,

    #[serde(rename = "versionedAncestorsId")]
    pub parent_id: Option<Uuid>,

    #[serde(rename = "versionedAncestorsId")]
    pub ancestor_ids: Option<Set<Uuid>>,
}

impl VersionedNodeTreePosition {
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
