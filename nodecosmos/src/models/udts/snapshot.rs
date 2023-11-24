use charybdis::macros::charybdis_udt_model;
use charybdis::types::Uuid;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default)]
#[charybdis_udt_model(type_name = snapshot)]
pub struct Snapshot {
    pub node_commit: Uuid,
    pub workflow_version: Uuid,
}
