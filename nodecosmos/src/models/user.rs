pub(crate) use super::udts::Address;
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2,
};
use charybdis::prelude::*;
use chrono::Utc;
use scylla::transport::session::TypedRowIter;

#[partial_model_generator]
#[charybdis_model(table_name = "users",
                  partition_keys = ["id"],
                  clustering_keys = [],
                  secondary_indexes = ["username", "email"])]
pub struct User {
    pub id: Option<Uuid>,
    pub username: Text,
    pub email: Text,
    pub password: Text,
    pub first_name: Text,
    pub last_name: Text,
    pub created_at: Option<Timestamp>,
    pub updated_at: Option<Timestamp>,
    pub address: Option<Address>,
    pub email_verified: Option<bool>,
}

impl User {
    pub(crate) async fn find_by_username(
        &self,
        session: &CachingSession,
    ) -> Result<Option<TypedRowIter<User>>, CharybdisError> {
        let query = find_user_query!("username = ?");

        Self::find(session, query, (&self.username,)).await
    }

    pub(crate) async fn find_by_email(
        &self,
        session: &CachingSession,
    ) -> Result<Option<TypedRowIter<User>>, CharybdisError> {
        let query = find_user_query!("email = ?");

        Self::find(session, query, (&self.email,)).await
    }

    async fn check_existing_user(&self, session: &CachingSession) -> Result<(), CharybdisError> {
        let user_by_username = self.find_by_username(session).await?;
        let user_by_email = self.find_by_email(session).await?;

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

        Ok(())
    }

    fn set_builtins(&mut self) {
        let now = Utc::now();

        self.id = Some(Uuid::new_v4());
        self.created_at = Some(now);
        self.updated_at = Some(now);
        self.email_verified = Some(false);
    }

    fn set_password(&mut self) -> Result<(), CharybdisError> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();

        let password_hash = argon2
            .hash_password(self.password.as_ref(), &salt)
            .map_err(|e| {
                CharybdisError::ValidationError(("password".to_string(), e.to_string()))
            })?;

        self.password = password_hash.to_string();

        Ok(())
    }
}

impl Callbacks for User {
    async fn before_insert(&mut self, session: &CachingSession) -> Result<(), CharybdisError> {
        self.check_existing_user(session).await?;

        self.set_builtins();
        self.set_password()?;

        Ok(())
    }

    async fn before_update(&mut self, _: &CachingSession) -> Result<(), CharybdisError> {
        self.updated_at = Some(Utc::now());
        Ok(())
    }
}
