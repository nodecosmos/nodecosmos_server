use charybdis::macros::charybdis_model;
use charybdis::types::{Frozen, List, Map, Text, Timestamp, Uuid};
use serde::{Deserialize, Serialize};

/// Node version acts as snapshot of the current state of the node.
/// We keep versions only for node where change has been made, so we
/// don't duplicate data for each ancestor.
#[charybdis_model(
    table_name = versioned_flow_steps,
    partition_keys = [id],
    clustering_keys = []
)]
#[derive(Serialize, Deserialize, Default)]
pub struct VersionedFlowStep {
    pub flow_step_id: Uuid,
    pub id: Uuid,
    pub title: Text,
    pub description_version: Option<Uuid>,
    pub node_ids: Frozen<List<Uuid>>,
    pub input_ids_by_node_id: Option<Frozen<Map<Uuid, Frozen<List<Uuid>>>>>,
    pub output_ids_by_node_id: Option<Frozen<Map<Uuid, Frozen<List<Uuid>>>>>,
    pub prev_flow_step_id: Option<Uuid>,
    pub next_flow_step_id: Option<Uuid>,
    pub created_at: Option<Timestamp>,
    pub updated_at: Option<Timestamp>,
}
