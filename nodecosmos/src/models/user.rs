use std::sync::Arc;

use bcrypt::{hash, verify};
use charybdis::callbacks::Callbacks;
use charybdis::macros::charybdis_model;
use charybdis::operations::Insert;
use charybdis::types::{Boolean, Text, Timestamp, Uuid};
use chrono::Utc;
use colored::Colorize;
use log::error;
use scylla::CachingSession;
use serde::{Deserialize, Serialize};

use crate::api::data::RequestData;
use crate::app::App;
use crate::errors::NodecosmosError;
use crate::models::token::Token;
use crate::models::traits::{ElasticDocument, SanitizeDescription};
use crate::models::udts::Address;

pub mod search;
pub mod update_profile_image;

const BCRYPT_COST: u32 = 6;

#[derive(Default, Serialize, Deserialize, Clone, PartialEq)]
pub enum UserContext {
    #[default]
    Default,
    ConfirmInvitationTokenValid,
}

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

    #[serde(skip)]
    #[charybdis(ignore)]
    pub ctx: Option<UserContext>,
}

impl Callbacks for User {
    type Extension = Arc<App>;
    type Error = NodecosmosError;

    async fn before_insert(&mut self, db_session: &CachingSession, app: &Arc<App>) -> Result<(), NodecosmosError> {
        if Self::maybe_find_first_by_username(self.username.clone())
            .execute(db_session)
            .await?
            .is_some()
        {
            return Err(NodecosmosError::ValidationError((
                "username".to_string(),
                "is taken".to_string(),
            )));
        }

        self.id = Uuid::new_v4();

        self.set_password()?;

        if self.ctx == Some(UserContext::ConfirmInvitationTokenValid) {
            self.is_confirmed = true;
        } else if let Some(user) = Self::maybe_find_first_by_email(self.email.clone())
            .execute(db_session)
            .await?
        {
            // If the user is not confirmed, resend the confirmation email, otherwise don't disturb them
            if !user.is_confirmed && !user.is_blocked {
                let token = if let Some(token) = Token::maybe_find_first_by_email(user.email.clone())
                    .execute(db_session)
                    .await?
                {
                    token
                } else {
                    let token = Token::new_user_confirmation(user.email.clone());

                    token.insert().execute(db_session).await?;

                    token
                };

                app.mailer
                    .send_confirm_user_email(user.email, user.username, token.id.to_string())
                    .await?;
            }

            return Err(NodecosmosError::EmailAlreadyExists);
        } else {
            let token = Token::new_user_confirmation(self.email.clone());
            token.insert().execute(db_session).await?;

            app.mailer
                .send_confirm_user_email(self.email.clone(), self.username.clone(), token.id.to_string())
                .await?;
        }

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
                _session: &scylla::CachingSession,
                _ext: &Self::Extension,
            ) -> Result<(), crate::errors::NodecosmosError> {
                self.updated_at = Utc::now();

                Ok(())
            }

            async fn after_update(
                &mut self,
                _session: &scylla::CachingSession,
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
impl_user_updated_at_with_elastic_ext_cb!(ConfirmUser);

partial_user!(
    ShowUser,
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

partial_user!(UpdateUser, id, first_name, last_name, updated_at, address);

partial_user!(ConfirmUser, id, is_confirmed, email, updated_at);

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
    pub fn from_user(user: User) -> Self {
        Self {
            id: user.id,
            first_name: user.first_name,
            last_name: user.last_name,
            username: user.username,
            email: user.email,
            profile_image_filename: user.profile_image_filename,
            profile_image_url: user.profile_image_url,
            is_confirmed: user.is_confirmed,
            is_blocked: user.is_blocked,
        }
    }

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

partial_user!(EmailUser, id, email);

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
