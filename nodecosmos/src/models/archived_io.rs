use crate::models::io::Io;
use charybdis::macros::charybdis_model;
use charybdis::types::{Set, Text, Timestamp, Uuid};
use nodecosmos_macros::{Branchable, Id, MaybeFlowId, MaybeFlowStepId};
use serde::{Deserialize, Serialize};

/// Node version acts as snapshot of the current state of the node.
/// We keep versions only for node where change has been made, so we
/// don't duplicate data for each ancestor.
#[charybdis_model(
    table_name = archived_input_outputs,
    partition_keys = [branch_id],
    clustering_keys = [root_id, id],
    local_secondary_indexes = [],
    table_options = r#"
        compression = {
            'sstable_compression': 'ZstdCompressor',
            'chunk_length_in_kb': 128
        }
    "#
)]
#[derive(Branchable, Id, MaybeFlowId, MaybeFlowStepId, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ArchivedIo {
    #[branch(original_id)]
    pub root_id: Uuid,

    pub node_id: Uuid,

    pub branch_id: Uuid,

    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,

    pub main_id: Option<Uuid>,

    pub flow_id: Option<Uuid>,

    /// outputted by flow step
    pub flow_step_id: Option<Uuid>,
    pub inputted_by_flow_steps: Option<Set<Uuid>>,
    pub title: Option<Text>,
    pub unit: Option<Text>,
    pub data_type: Option<Text>,
    pub value: Option<Text>,

    #[serde(default = "chrono::Utc::now")]
    pub created_at: Timestamp,

    #[serde(default = "chrono::Utc::now")]
    pub updated_at: Timestamp,
}

impl From<&Io> for ArchivedIo {
    fn from(io: &Io) -> Self {
        Self {
            root_id: io.root_id,
            node_id: io.node_id,
            branch_id: io.branch_id,
            id: io.id,
            main_id: io.main_id,
            flow_id: io.flow_id,
            flow_step_id: io.flow_step_id,
            inputted_by_flow_steps: io.inputted_by_flow_steps.clone(),
            title: io.title.clone(),
            unit: io.unit.clone(),
            data_type: io.data_type.clone(),
            value: io.value.clone(),
            created_at: io.created_at,
            updated_at: io.updated_at,
        }
    }
}

partial_archived_io!(PkArchivedIo, root_id, branch_id, id);

impl From<&Io> for PkArchivedIo {
    fn from(io: &Io) -> Self {
        Self {
            root_id: io.root_id,
            branch_id: io.branch_id,
            id: io.id,
        }
    }
}
