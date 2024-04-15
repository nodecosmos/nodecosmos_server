use charybdis::macros::charybdis_model;
use charybdis::types::{Frozen, List, Text, Timestamp, Uuid};
use serde::{Deserialize, Serialize};

/// Node version acts as snapshot of the current state of the node.
/// We keep versions only for node where change has been made, so we
/// don't duplicate data for each ancestor.
#[charybdis_model(
    table_name = flow_commits,
    partition_keys = [id],
    clustering_keys = [branch_id],
    table_options = r#"
        compression = {
            'sstable_compression': 'DeflateCompressor',
            'chunk_length_in_kb': 64
        }
    "#
)]
#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FlowCommit {
    pub flow_id: Uuid,
    pub id: Uuid,
    pub branch_id: Uuid,
    pub title: Text,
    pub description_commit: Option<Uuid>,
    pub flow_step_ids: Frozen<List<Uuid>>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}
