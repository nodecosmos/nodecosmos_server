use crate::models::node::Node;
use charybdis::macros::charybdis_model;
use charybdis::types::{Set, Uuid};
use serde::{Deserialize, Serialize};

#[charybdis_model(
    table_name = versioned_node_ancestors,
    partition_keys = [id],
    clustering_keys = [],
    table_options = r#"
        compression = {'sstable_compression': 'DeflateCompressor', 'chunk_length_in_kb': 4};
    "#,
)]
#[derive(Serialize, Deserialize, Default)]
pub struct VersionedNodeAncestors {
    pub id: Uuid,
    pub ancestor_ids: Option<Set<Uuid>>,
}

impl VersionedNodeAncestors {
    pub fn new(node: &Node) -> Self {
        VersionedNodeAncestors {
            id: Uuid::new_v4(),
            ancestor_ids: node.ancestor_ids.clone(),
        }
    }
}
