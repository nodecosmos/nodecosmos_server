pub use super::udts::Address;

use charybdis::prelude::*;
use chrono::Utc;

use bcrypt::{hash, verify};

const BCRYPT_COST: u32 = 6;

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
    pub email_verified: Option<Boolean>,
}

impl User {
    pub async fn find_by_username(&self, session: &CachingSession) -> Option<User> {
        let query = find_user_query!("username = ?");

        let res = Self::find(session, query, (&self.username,)).await;
        match res {
            Ok(mut res) => match res.next() {
                Some(Ok(user)) => Some(user),
                _ => None,
            },
            Err(_) => None,
        }
    }

    pub async fn find_by_email(&self, session: &CachingSession) -> Option<User> {
        let query = find_user_query!("email = ?");

        let res = Self::find(session, query, (&self.email,)).await;
        match res {
            Ok(mut res) => match res.next() {
                Some(Ok(user)) => Some(user),
                _ => None,
            },
            Err(_) => None,
        }
    }

    pub async fn check_existing_user(
        &self,
        session: &CachingSession,
    ) -> Result<(), CharybdisError> {
        if self.find_by_username(session).await.is_some() {
            return Err(CharybdisError::ValidationError((
                "username".to_string(),
                "is taken".to_string(),
            )));
        }

        if self.find_by_email(session).await.is_some() {
            return Err(CharybdisError::ValidationError((
                "email".to_string(),
                "is taken".to_string(),
            )));
        }

        Ok(())
    }

    pub async fn verify_password(&self, password: &String) -> Result<(), CharybdisError> {
        let password_hash = hash(&self.password, BCRYPT_COST).map_err(|e| {
            // TODO: log error here

            CharybdisError::CustomError(
                "There was an error processing your request. Please try again later.".to_string(),
            )
        })?;

        verify(password, &password_hash).map_err(|_e| {
            CharybdisError::ValidationError(("password".to_string(), "is incorrect".to_string()))
        })?;

        Ok(())
    }

    fn set_defaults(&mut self) {
        let now = Utc::now();

        self.id = Some(Uuid::new_v4());
        self.created_at = Some(now);
        self.updated_at = Some(now);
        self.email_verified = Some(false);
    }

    fn set_password(&mut self) -> Result<(), CharybdisError> {
        self.password = hash(&self.password, BCRYPT_COST).map_err(|e| {
            // TODO: log error here

            CharybdisError::CustomError(
                "There was an error processing your request. Please try again later.".to_string(),
            )
        })?;

        Ok(())
    }
}

impl Callbacks for User {
    async fn before_insert(&mut self, session: &CachingSession) -> Result<(), CharybdisError> {
        self.check_existing_user(session).await?;

        self.set_defaults();
        self.set_password()?;

        Ok(())
    }

    async fn before_update(&mut self, _: &CachingSession) -> Result<(), CharybdisError> {
        self.updated_at = Some(Utc::now());
        Ok(())
    }
}

partial_user!(GetUser, id, username, email, created_at, updated_at);

partial_user!(UpdateUser, id, first_name, last_name, updated_at, address);
impl Callbacks for UpdateUser {
    async fn before_update(&mut self, _: &CachingSession) -> Result<(), CharybdisError> {
        self.updated_at = Some(Utc::now());
        Ok(())
    }
}

partial_user!(DeleteUser, id);

partial_user!(CurrentUser, id, username, email, email_verified);
