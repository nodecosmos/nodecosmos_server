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
    pub username: Option<Text>,
    pub email: Option<Text>,
    pub password: Option<Text>,
    pub hashed_password: Option<Text>,
    pub first_name: Option<Text>,
    pub last_name: Option<Text>,
    pub created_at: Option<Text>,
    pub updated_at: Option<Text>,
    pub address: Option<Address>,
}

impl Callbacks for User {
    async fn before_insert(&self) -> Result<(), CharybdisError> {
        println!("before_insert");

        if true {
            return Err(CharybdisError::ValidationError((
                "username".to_string(),
                "taken".to_string(),
            )));
        }
        Ok(())
    }
}
