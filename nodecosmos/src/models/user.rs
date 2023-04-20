use super::udts::Address;
use charybdis::prelude::*;
use scylla::_macro_internal::FromRowError;
use scylla::transport::session::TypedRowIter;

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

impl User {
    async fn find_by_username(session: &CachingSession, username: &str) -> TypedRowIter<User> {
        let query = find_user_query!("username = ?");
        Self::find(session, query, (username,)).await?
    }

    async fn find_by_email(session: &CachingSession, email: &str) -> TypedRowIter<User> {
        let query = find_user_query!("email = ?");
        Self::find(session, query, (email,)).await?
    }
}

impl Callbacks for User {
    async fn before_insert(&self, session: &CachingSession) -> Result<(), CharybdisError> {
        let user_by_username = self.find_by_username(session, &self.username).await;
        let user_by_email = self.find_by_email(session, &self.email).await;

        if user_by_username.is_some() {
            return Err(CharybdisError::ValidationError((
                "username".to_string(),
                "is taken".to_string(),
            )));
        }

        if user_by_email.is_some() {
            return Err(CharybdisError::ValidationError((
                "email".to_string(),
                "is taken".to_string(),
            )));
        }

        if self.username.is_empty() {
            return Err(CharybdisError::ValidationError((
                "username".to_string(),
                "is empty".to_string(),
            )));
        }

        if self.email.is_empty() {
            return Err(CharybdisError::ValidationError((
                "email".to_string(),
                "is empty".to_string(),
            )));
        }

        Ok(())
    }
}
