pub mod update_profile_image;

pub use super::udts::Address;
use crate::api::data::RequestData;
use crate::app::App;
use crate::errors::NodecosmosError;
use crate::models::traits::{ElasticDocument, SanitizeDescription};
use bcrypt::{hash, verify};
use charybdis::callbacks::Callbacks;
use charybdis::macros::charybdis_model;
use charybdis::types::{Boolean, Text, Timestamp, Uuid};
use chrono::Utc;
use colored::Colorize;
use log::error;
use scylla::CachingSession;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

const BCRYPT_COST: u32 = 6;

#[charybdis_model(
    table_name = users,
    partition_keys = [id],
    clustering_keys = [],
    global_secondary_indexes = [username, email],
)]
#[derive(Serialize, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct User {
    #[serde(default)]
    pub id: Uuid,

    pub username: Text,
    pub email: Text,
    pub password: Text,
    pub first_name: Text,
    pub last_name: Text,
    pub bio: Option<Text>,
    pub address: Option<Address>,
    pub profile_image_filename: Option<Text>,
    pub profile_image_url: Option<Text>,

    #[serde(default)]
    pub is_confirmed: Boolean,

    #[serde(default)]
    pub is_blocked: Boolean,

    #[serde(default = "chrono::Utc::now")]
    pub created_at: Timestamp,

    #[serde(default = "chrono::Utc::now")]
    pub updated_at: Timestamp,
}

impl Callbacks for User {
    type Extension = Arc<App>;
    type Error = NodecosmosError;

    async fn before_insert(&mut self, db_session: &CachingSession, _ext: &Arc<App>) -> Result<(), NodecosmosError> {
        self.check_existing_user(db_session).await?;

        self.id = Uuid::new_v4();

        self.set_password()?;

        Ok(())
    }

    async fn after_insert(&mut self, _db_session: &CachingSession, app: &Arc<App>) -> Result<(), NodecosmosError> {
        self.add_elastic_document(&app.elastic_client).await;

        Ok(())
    }

    async fn after_delete(&mut self, _: &CachingSession, app: &Arc<App>) -> Result<(), NodecosmosError> {
        self.update_elastic_document(&app.elastic_client).await;

        Ok(())
    }
}

impl User {
    pub async fn find_by_username(&self, db_session: &CachingSession) -> Option<User> {
        find_first_user!("username = ?", (&self.username,))
            .execute(db_session)
            .await
            .ok()
    }

    pub async fn find_by_email(&self, db_session: &CachingSession) -> Option<User> {
        find_first_user!("email = ?", (&self.email,))
            .execute(db_session)
            .await
            .ok()
    }

    pub async fn check_existing_user(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        if self.find_by_username(db_session).await.is_some() {
            return Err(NodecosmosError::ValidationError((
                "username".to_string(),
                "is taken".to_string(),
            )));
        }

        if self.find_by_email(db_session).await.is_some() {
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

    fn set_password(&mut self) -> Result<(), NodecosmosError> {
        self.password = hash(&self.password, BCRYPT_COST).map_err(|_| {
            error!("{}", "error hashing password".bright_red().bold());

            NodecosmosError::InternalServerError(
                "There was an error processing your request. Please try again later.".to_string(),
            )
        })?;

        Ok(())
    }
}

macro_rules! impl_user_updated_at_with_elastic_ext_cb {
    ($struct_name:ident) => {
        impl charybdis::callbacks::Callbacks for $struct_name {
            type Extension = crate::api::data::RequestData;
            type Error = crate::errors::NodecosmosError;

            async fn before_update(
                &mut self,
                _session: &charybdis::CachingSession,
                _ext: &Self::Extension,
            ) -> Result<(), crate::errors::NodecosmosError> {
                self.updated_at = Utc::now();

                Ok(())
            }

            async fn after_update(
                &mut self,
                _session: &charybdis::CachingSession,
                data: &Self::Extension,
            ) -> Result<(), crate::errors::NodecosmosError> {
                use crate::models::node::UpdateOwnerNode;
                use crate::models::traits::ElasticDocument;

                self.update_elastic_document(data.elastic_client()).await;

                let user_id = self.id.clone();
                let data = data.clone();

                tokio::spawn(async move {
                    UpdateOwnerNode::update_owner_records(&data, user_id).await;
                });

                Ok(())
            }
        }
    };
}

impl_user_updated_at_with_elastic_ext_cb!(UpdateUser);
impl_user_updated_at_with_elastic_ext_cb!(UpdateProfileImageUser);

partial_user!(
    GetUser,
    id,
    username,
    bio,
    first_name,
    last_name,
    created_at,
    updated_at,
    profile_image_filename,
    profile_image_url,
    is_confirmed,
    is_blocked
);

impl GetUser {
    pub async fn find_by_username(db_session: &CachingSession, username: &String) -> Result<GetUser, NodecosmosError> {
        find_first_get_user!("username = ?", (username,))
            .execute(db_session)
            .await
            .map_err(NodecosmosError::from)
    }
}

partial_user!(UpdateUser, id, first_name, last_name, updated_at, address);

partial_user!(
    CurrentUser,
    id,
    first_name,
    last_name,
    username,
    email,
    profile_image_filename,
    profile_image_url,
    is_confirmed,
    is_blocked
);

impl CurrentUser {
    pub fn full_name(&self) -> String {
        format!("{} {}", self.first_name, self.last_name)
    }
}

partial_user!(
    UpdateProfileImageUser,
    id,
    profile_image_filename,
    profile_image_url,
    updated_at
);

partial_user!(UpdateBioUser, id, username, bio, updated_at);

impl Callbacks for UpdateBioUser {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_update(&mut self, _: &CachingSession, _ext: &RequestData) -> Result<(), NodecosmosError> {
        self.bio.sanitize()?;

        Ok(())
    }

    async fn after_update(&mut self, _: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        self.update_elastic_document(data.elastic_client()).await;

        Ok(())
    }
}

pub trait FullName {
    fn full_name(&self) -> String;
}

macro_rules! impl_full_name {
    ($($t:ty),+) => {
        $(
            impl FullName for $t {
                fn full_name(&self) -> String {
                    format!("{} {}", self.first_name, self.last_name)
                }
            }
        )+
    };
}

impl_full_name!(User, CurrentUser);
