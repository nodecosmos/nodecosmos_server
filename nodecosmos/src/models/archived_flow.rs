use crate::models::flow::Flow;
use charybdis::macros::charybdis_model;
use charybdis::types::{Double, Int, Text, Timestamp, Uuid};
use nodecosmos_macros::{Branchable, Id};
use serde::{Deserialize, Serialize};

#[charybdis_model(
    table_name = archived_flows,
    partition_keys = [branch_id],
    clustering_keys = [node_id, vertical_index, start_index, id],
    local_secondary_indexes = [id],
    table_options = r#"
        compression = {
            'sstable_compression': 'SnappyCompressor',
            'chunk_length_in_kb': 64
        }
    "#
)]
#[derive(Id, Branchable, Serialize, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ArchivedFlow {
    #[branch(original_id)]
    pub node_id: Uuid,
    pub branch_id: Uuid,

    // vertical index
    pub vertical_index: Double,

    // start index is not conflicting, flows can start at same index
    pub start_index: Int,

    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,

    #[serde(default)]
    pub title: Text,

    #[serde(default = "chrono::Utc::now")]
    pub created_at: Timestamp,

    #[serde(default = "chrono::Utc::now")]
    pub updated_at: Timestamp,
}

impl From<&Flow> for ArchivedFlow {
    fn from(flow: &Flow) -> Self {
        Self {
            node_id: flow.node_id,
            branch_id: flow.branch_id,
            vertical_index: flow.vertical_index,
            start_index: flow.start_index,
            id: flow.id,
            title: flow.title.clone(),
            created_at: flow.created_at,
            updated_at: flow.updated_at,
        }
    }
}

partial_archived_flow!(PkArchivedFlow, node_id, branch_id, vertical_index, start_index, id);

impl From<&Flow> for PkArchivedFlow {
    fn from(flow: &Flow) -> Self {
        Self {
            node_id: flow.node_id,
            branch_id: flow.branch_id,
            vertical_index: flow.vertical_index,
            start_index: flow.start_index,
            id: flow.id,
        }
    }
}
