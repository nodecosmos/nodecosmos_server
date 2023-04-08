use charybdis::prelude::*;

#[charybdis_view_model(table_name="users_by_username", partition_keys=["username"], clustering_keys=["id"])]
pub struct UsersByUsername {
    pub username: Text,
    pub id: Uuid,
    pub email: Text,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}
