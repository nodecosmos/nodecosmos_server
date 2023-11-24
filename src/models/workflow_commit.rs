use charybdis::macros::charybdis_model;
use charybdis::types::{Map, Text, Uuid};
use serde::{Deserialize, Serialize};

/// Node version acts as snapshot of the current state of the node.
/// We keep versions only for node where change has been made, so we
/// don't duplicate data for each ancestor.
#[charybdis_model(
    table_name = workflow_commits,
    partition_keys = [id],
    clustering_keys = [],
    table_options = r#"
        compression = { 
            'sstable_compression': 'DeflateCompressor'
        }
    "#
)]
#[derive(Serialize, Deserialize, Default)]
pub struct WorkflowCommit {
    pub workflow_id: Uuid,
    pub id: Uuid,
    pub title: Text,
    pub flows_commit_id_by_id: Map<Uuid, Uuid>,
}
