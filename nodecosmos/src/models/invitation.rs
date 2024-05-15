use charybdis::macros::charybdis_model;
use charybdis::types::{Boolean, Text, Uuid};
use serde::Serialize;

#[charybdis_model(
    table_name = invitations,
    partition_keys = [user_id],
    clustering_keys = [id],
)]
#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Invitation {
    pub id: Uuid,
    pub user_id: Uuid,
    pub email: Text,
    pub seen: Boolean,
    pub accepted: Boolean,
    pub node_id: Option<Uuid>,
}
