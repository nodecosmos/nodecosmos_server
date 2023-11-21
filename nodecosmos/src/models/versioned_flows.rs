use charybdis::macros::charybdis_model;
use charybdis::types::{Map, Uuid};
use serde::{Deserialize, Serialize};

#[charybdis_model(
    table_name = versioned_flows,
    partition_keys = [id],
    clustering_keys = []
)]
#[derive(Serialize, Deserialize, Default)]
pub struct VersionedFlows {
    pub id: Uuid,
    pub flow_version_by_id: Option<Map<Uuid, Uuid>>,
}
