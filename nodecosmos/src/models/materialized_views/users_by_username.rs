use charybdis::prelude::*;

#[charybdis_view_model(table_name="users_by_username", base_table="users", partition_keys=["username"], clustering_keys=["id"])]
pub struct UsersByUsername {
    pub username: Text,
    pub id: Uuid,
    pub email: Text,
    pub created_at: Option<Timestamp>,
    pub updated_at: Option<Timestamp>,
}
