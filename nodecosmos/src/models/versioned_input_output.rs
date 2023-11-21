use charybdis::macros::charybdis_model;
use charybdis::types::{Text, Uuid};
use serde::{Deserialize, Serialize};

/// Node version acts as snapshot of the current state of the node.
/// We keep versions only for node where change has been made, so we
/// don't duplicate data for each ancestor.
#[charybdis_model(
    table_name = versioned_input_outputs,
    partition_keys = [id],
    clustering_keys = []
)]
#[derive(Serialize, Deserialize, Default)]
pub struct VersionedIo {
    pub input_id: Uuid,
    pub id: Uuid,
    pub title: Text,
    pub description_version: Option<Uuid>,
}
