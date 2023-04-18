use super::udts::Address;
use charybdis::prelude::*;
use scylla::_macro_internal::FromRowError;

#[partial_model_generator]
#[charybdis_model(table_name = "users",
                  partition_keys = ["id"],
                  clustering_keys = [],
                  secondary_indexes = [])]
pub struct User {
    pub id: Uuid,
    pub username: Text,
    pub email: Text,
    pub password: Text,
    pub hashed_password: Text,
    pub first_name: Option<Text>,
    pub last_name: Option<Text>,
    pub created_at: Option<Timestamp>,
    pub updated_at: Option<Timestamp>,
    pub address: Option<Address>,
}
