use charybdis::prelude::*;

#[charybdis_view_model(table_name="users_by_username", partition_keys=["email"], clustering_keys=["id"])]
pub struct UsersByEmail{
    pub email: Text,
    pub id: Uuid,
    pub username: Text,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}
