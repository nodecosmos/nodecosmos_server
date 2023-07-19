pub use super::udts::Address;
use crate::models::helpers::impl_updated_at_cb;
use bcrypt::{hash, verify};
use charybdis::{Boolean, Callbacks, CharybdisError, Find, Set, Text, Timestamp, Uuid};
use charybdis_macros::{charybdis_model, partial_model_generator};
use chrono::Utc;
use scylla::CachingSession;

const BCRYPT_COST: u32 = 6;

#[partial_model_generator]
#[charybdis_model(
table_name = users,
partition_keys = [id],
clustering_keys = [],
secondary_indexes = [username, email]
)]
pub struct User {
    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,
    pub username: Text,
    pub email: Text,
    pub password: Text,
    pub first_name: Text,
    pub last_name: Text,
    pub created_at: Option<Timestamp>,
    pub updated_at: Option<Timestamp>,
    pub address: Option<Address>,
    pub confirmed: Option<Boolean>,
    pub liked_object_ids: Option<Set<Uuid>>,
}

impl User {
    pub async fn find_by_username(&self, session: &CachingSession) -> Option<User> {
        let query = find_user_query!("username = ?");

        Self::find_one(session, query, (&self.username,)).await.ok()
    }

    pub async fn find_by_email(&self, session: &CachingSession) -> Option<User> {
        let query = find_user_query!("email = ?");

        Self::find_one(session, query, (&self.email,)).await.ok()
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
        let password_hash = hash(&self.password, BCRYPT_COST).map_err(|_| {
            // TODO: log error here

            CharybdisError::CustomError(
                "There was an error processing your request. Please try again later.".to_string(),
            )
        })?;

        verify(password, &password_hash).map_err(|_| {
            CharybdisError::ValidationError(("password".to_string(), "is incorrect".to_string()))
        })?;

        Ok(())
    }

    fn set_defaults(&mut self) {
        let now = Utc::now();

        self.id = Uuid::new_v4();
        self.created_at = Some(now);
        self.updated_at = Some(now);
        self.confirmed = Some(false);
    }

    fn set_password(&mut self) -> Result<(), CharybdisError> {
        self.password = hash(&self.password, BCRYPT_COST).map_err(|_| {
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

partial_user!(GetUser, id, username, created_at, updated_at);

partial_user!(UpdateUser, id, first_name, last_name, updated_at, address);
impl_updated_at_cb!(UpdateUser);

partial_user!(DeleteUser, id);

partial_user!(LikedObjectIdsUser, id, liked_object_ids);
