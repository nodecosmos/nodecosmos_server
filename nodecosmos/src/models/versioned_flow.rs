use charybdis::macros::charybdis_model;
use charybdis::types::{Frozen, List, Text, Timestamp, Uuid};
use serde::{Deserialize, Serialize};

/// Node version acts as snapshot of the current state of the node.
/// We keep versions only for node where change has been made, so we
/// don't duplicate data for each ancestor.
#[charybdis_model(
    table_name = versioned_flows,
    partition_keys = [id],
    clustering_keys = []
)]
#[derive(Serialize, Deserialize, Default)]
pub struct VersionedFlow {
    pub flow_id: Uuid,
    pub id: Uuid,
    pub title: Text,
    pub description_version: Option<Uuid>,
    pub flow_step_ids: Frozen<List<Uuid>>,
    pub created_at: Option<Timestamp>,
    pub updated_at: Option<Timestamp>,
}
