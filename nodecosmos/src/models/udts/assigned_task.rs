use charybdis::macros::charybdis_udt_model;
use charybdis::types::Uuid;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[charybdis_udt_model(type_name = assignedtask)]
pub struct AssignedTask {
    pub node_id: Uuid,
    pub branch_id: Uuid,
    pub task_id: Uuid,
}
