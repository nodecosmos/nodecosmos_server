use charybdis::macros::charybdis_model;
use charybdis::types::{Frozen, List, Map, Text, Timestamp, Uuid};
use serde::{Deserialize, Serialize};

/// Node version acts as snapshot of the current state of the node.
/// We keep versions only for node where change has been made, so we
/// don't duplicate data for each ancestor.
#[charybdis_model(
    table_name = flow_step_commits,
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
pub struct FlowStepCommits {
    pub flow_step_id: Uuid,
    pub id: Uuid,
    pub branch_id: Uuid,
    pub title: Text,
    pub description_version: Option<Uuid>,
    pub node_ids: Frozen<List<Uuid>>,
    pub input_ids_by_node_id: Option<Frozen<Map<Uuid, Frozen<List<Uuid>>>>>,
    pub output_ids_by_node_id: Option<Frozen<Map<Uuid, Frozen<List<Uuid>>>>>,
    pub prev_flow_step_id: Option<Uuid>,
    pub next_flow_step_id: Option<Uuid>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}
