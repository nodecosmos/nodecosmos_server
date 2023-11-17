use charybdis::macros::charybdis_model;
use charybdis::types::{List, Text, Uuid};
use serde::{Deserialize, Serialize};

/// Node version acts as snapshot of the current state of the node.
/// We keep versions only for node where change has been made, so we
/// don't duplicate data for each ancestor.
#[charybdis_model(
    table_name = versioned_workflows,
    partition_keys = [id],
    clustering_keys = [],
    table_options = r#"
        compression = {'sstable_compression': 'DeflateCompressor', 'chunk_length_in_kb': 4};
    "#,
)]
#[derive(Serialize, Deserialize, Default)]
pub struct VersionedWorkflow {
    pub workflow_id: Uuid,
    pub id: Uuid,
    pub title: Text,
    pub versioned_flow_ids: List<Uuid>,
}
