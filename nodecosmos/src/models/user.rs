use super::udts::Address;
use charybdis::prelude::*;
use scylla::_macro_internal::FromRowError;

#[partial_model_generator]
#[charybdis_model(table_name = "users",
                  partition_keys = ["id"],
                  clustering_keys = [],
                  secondary_indexes = ["username", "email"])]
pub struct User {
    pub id: Uuid,
    pub username: Text,
    pub email: Text,
    pub password: Text,
    pub hashed_password: Text,
    pub first_name: Text,
    pub last_name: Text,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub address: Option<Address>,
}

impl Callbacks for User {
    async fn before_insert(&self, session: &CachingSession) -> Result<(), CharybdisError> {
        println!("before_insert");

        Ok(())
    }
}
