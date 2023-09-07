pub(crate) use super::udts::Address;
use crate::app::CbExtension;
use crate::models::helpers::{default_to_false_bool, impl_user_updated_at_with_elastic_ext_cb};
use crate::services::elastic::{add_elastic_document, delete_elastic_document};
use bcrypt::{hash, verify};
use charybdis::{Boolean, CharybdisError, ExtCallbacks, Find, Set, Text, Timestamp, Uuid};
use charybdis_macros::{charybdis_model, partial_model_generator};
use chrono::Utc;
use colored::Colorize;
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

    #[serde(rename = "firstName")]
    pub first_name: Text,

    #[serde(rename = "lastName")]
    pub last_name: Text,

    pub bio: Option<Text>,

    #[serde(rename = "createdAt")]
    pub created_at: Option<Timestamp>,

    #[serde(rename = "updatedAt")]
    pub updated_at: Option<Timestamp>,

    pub address: Option<Address>,

    #[serde(rename = "isConfirmed", default = "default_to_false_bool")]
    pub is_confirmed: Boolean,

    #[serde(rename = "isBlocked", default = "default_to_false_bool")]
    pub is_blocked: Boolean,

    #[serde(rename = "likedObjectIds")]
    pub liked_object_ids: Option<Set<Uuid>>,
}

impl User {
    pub const ELASTIC_IDX_NAME: &'static str = "users";

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

    pub async fn verify_password(&self, password: &String) -> Result<bool, CharybdisError> {
        let res = verify(password, &self.password).map_err(|_| {
            CharybdisError::ValidationError(("password".to_string(), "is incorrect".to_string()))
        })?;

        Ok(res)
    }

    fn set_defaults(&mut self) {
        let now = Utc::now();

        self.id = Uuid::new_v4();
        self.created_at = Some(now);
        self.updated_at = Some(now);
    }

    fn set_password(&mut self) -> Result<(), CharybdisError> {
        self.password = hash(&self.password, BCRYPT_COST).map_err(|_| {
            println!("{}", "error hashing password".bright_red().bold());

            CharybdisError::CustomError(
                "There was an error processing your request. Please try again later.".to_string(),
            )
        })?;

        Ok(())
    }
}

impl ExtCallbacks<CbExtension> for User {
    async fn before_insert(
        &mut self,
        session: &CachingSession,
        _ext: &CbExtension,
    ) -> Result<(), CharybdisError> {
        self.check_existing_user(session).await?;

        self.set_defaults();
        self.set_password()?;

        Ok(())
    }

    async fn after_insert(
        &mut self,
        _session: &CachingSession,
        cb_extension: &CbExtension,
    ) -> Result<(), CharybdisError> {
        add_elastic_document(
            &cb_extension.elastic_client,
            User::ELASTIC_IDX_NAME,
            self,
            self.id.to_string(),
        )
        .await;

        Ok(())
    }

    async fn before_update(
        &mut self,
        _: &CachingSession,
        _ext: &CbExtension,
    ) -> Result<(), CharybdisError> {
        self.updated_at = Some(Utc::now());
        Ok(())
    }

    async fn after_delete(
        &mut self,
        _session: &CachingSession,
        cb_extension: &CbExtension,
    ) -> Result<(), CharybdisError> {
        delete_elastic_document(
            &cb_extension.elastic_client,
            User::ELASTIC_IDX_NAME,
            self.id.to_string(),
        )
        .await;

        Ok(())
    }
}

partial_user!(GetUser, id, username, created_at, updated_at);

partial_user!(UpdateUser, id, first_name, last_name, updated_at, address);
impl_user_updated_at_with_elastic_ext_cb!(UpdateUser);

partial_user!(LikedObjectIdsUser, id, liked_object_ids);
