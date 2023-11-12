use charybdis::macros::charybdis_model;
use charybdis::types::{Text, Uuid};
use serde::{Deserialize, Serialize};

/// Node version acts as snapshot of the current state of the node.
/// We keep versions only for node where change has been made, so we
/// don't duplicate data for each ancestor.
#[charybdis_model(
    table_name = versioned_nodes,
    partition_keys = [id],
    clustering_keys = [],
    table_options = r#"
        compression = {'sstable_compression': 'DEFLATE'};
    "#,
)]
#[derive(Serialize, Deserialize, Default)]
pub struct VersionedNode {
    pub id: Uuid,
    pub node_id: Uuid,
    pub title: Text,
    pub description_version: Uuid,
    pub workflow_version: Option<Uuid>,
    pub parent_id: Uuid,
    pub descendant_ids: Vec<Uuid>,
    pub order_index: i32,
}
