use charybdis::prelude::*;
use super::Address;

#[partial_model_generator]
#[charybdis_model(table_name = "users", partition_keys = ["id"], clustering_keys = [], secondary_indexes = [])]
pub struct User {
    pub id: Uuid,
    pub username: Text,
    pub password: Text,
    pub hashed_password: Text,
    pub email: Text,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}
