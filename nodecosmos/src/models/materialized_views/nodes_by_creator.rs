use charybdis::macros::charybdis_view_model;
use charybdis::types::{Boolean, Text, Uuid};
use serde::{Deserialize, Serialize};

#[charybdis_view_model(
    table_name = nodes_by_creator,
    base_table = nodes,
    partition_keys = [creator_id],
    clustering_keys = [id, branch_id]
)]
#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NodesByCreator {
    pub creator_id: Uuid,
    pub id: Uuid,
    pub branch_id: Uuid,
    pub root_id: Uuid,
    pub title: Text,
    pub is_root: Boolean,
    pub is_public: Boolean,
}
