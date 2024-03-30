use charybdis::macros::charybdis_view_model;
use charybdis::types::Uuid;
use serde::{Deserialize, Serialize};

#[charybdis_view_model(
    table_name=nodes_by_owner_id,
    base_table=nodes,
    partition_keys=[owner_id],
    clustering_keys=[id, branch_id]
)]
#[derive(Serialize, Deserialize, Default)]
pub struct NodesByOwner {
    pub owner_id: Uuid,

    pub id: Uuid,

    pub branch_id: Uuid,
}
