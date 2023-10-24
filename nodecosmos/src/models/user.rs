pub use super::udts::Address;
use crate::errors::NodecosmosError;
use crate::models::helpers::{default_to_false_bool, impl_user_updated_at_with_elastic_ext_cb};
use crate::services::elastic::{add_elastic_document, delete_elastic_document};
use crate::CbExtension;
use bcrypt::{hash, verify};
use charybdis::callbacks::ExtCallbacks;
use charybdis::macros::charybdis_model;
use charybdis::types::{Boolean, Set, Text, Timestamp, Uuid};
use chrono::Utc;
use colored::Colorize;
use scylla::CachingSession;
use serde::{Deserialize, Serialize};

const BCRYPT_COST: u32 = 6;

#[charybdis_model(
    table_name = users,
    partition_keys = [id],
    clustering_keys = [],
    global_secondary_indexes = [username, email],
)]
#[derive(Serialize, Deserialize, Default)]
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
        find_one_user!(session, "username = ?", (&self.username,)).await.ok()
    }

    pub async fn find_by_email(&self, session: &CachingSession) -> Option<User> {
        find_one_user!(session, "email = ?", (&self.email,)).await.ok()
    }

    pub async fn check_existing_user(&self, session: &CachingSession) -> Result<(), NodecosmosError> {
        if self.find_by_username(session).await.is_some() {
            return Err(NodecosmosError::ValidationError((
                "username".to_string(),
                "is taken".to_string(),
            )));
        }

        if self.find_by_email(session).await.is_some() {
            return Err(NodecosmosError::ValidationError((
                "email".to_string(),
                "is taken".to_string(),
            )));
        }

        Ok(())
    }

    pub async fn verify_password(&self, password: &String) -> Result<bool, NodecosmosError> {
        let res = verify(password, &self.password)
            .map_err(|_| NodecosmosError::ValidationError(("password".to_string(), "is incorrect".to_string())))?;

        Ok(res)
    }

    fn set_defaults(&mut self) {
        let now = Utc::now();

        self.id = Uuid::new_v4();
        self.created_at = Some(now);
        self.updated_at = Some(now);
    }

    fn set_password(&mut self) -> Result<(), NodecosmosError> {
        self.password = hash(&self.password, BCRYPT_COST).map_err(|_| {
            println!("{}", "error hashing password".bright_red().bold());

            NodecosmosError::InternalServerError(
                "There was an error processing your request. Please try again later.".to_string(),
            )
        })?;

        Ok(())
    }
}

impl ExtCallbacks<CbExtension, NodecosmosError> for User {
    async fn before_insert(&mut self, session: &CachingSession, _ext: &CbExtension) -> Result<(), NodecosmosError> {
        self.check_existing_user(session).await?;

        self.set_defaults();
        self.set_password()?;

        Ok(())
    }

    async fn after_insert(&self, _session: &CachingSession, cb_extension: &CbExtension) -> Result<(), NodecosmosError> {
        add_elastic_document(
            &cb_extension.elastic_client,
            User::ELASTIC_IDX_NAME,
            self,
            self.id.to_string(),
        )
        .await;

        Ok(())
    }

    async fn before_update(&mut self, _: &CachingSession, _ext: &CbExtension) -> Result<(), NodecosmosError> {
        self.updated_at = Some(Utc::now());
        Ok(())
    }

    async fn after_delete(&self, _: &CachingSession, cb_extension: &CbExtension) -> Result<(), NodecosmosError> {
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

partial_user!(
    CurrentUser,
    id,
    first_name,
    last_name,
    username,
    email,
    is_confirmed,
    is_blocked
);

impl CurrentUser {
    pub fn full_name(&self) -> String {
        format!("{} {}", self.first_name, self.last_name)
    }
}
