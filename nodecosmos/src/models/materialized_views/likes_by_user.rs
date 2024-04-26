use charybdis::macros::charybdis_view_model;
use charybdis::types::Uuid;
use serde::{Deserialize, Serialize};

#[charybdis_view_model(
    table_name=likes_by_user_id,
    base_table=likes,
    partition_keys=[user_id],
    clustering_keys=[object_id, branch_id]
)]
#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LikesByUser {
    pub user_id: Uuid,
    pub object_id: Uuid,
    pub branch_id: Uuid,
}
