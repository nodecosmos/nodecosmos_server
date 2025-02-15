use std::sync::Arc;

use bcrypt::{hash, verify};
use charybdis::callbacks::Callbacks;
use charybdis::macros::charybdis_model;
use charybdis::operations::Insert;
use charybdis::types::{Boolean, Set, Text, Timestamp, Uuid};
use chrono::Utc;
use colored::Colorize;
use log::error;
use rustrict::CensorStr;
use scylla::CachingSession;
use serde::{Deserialize, Serialize};

use crate::api::data::RequestData;
use crate::app::App;
use crate::constants::BLACKLIST_USERNAMES;
use crate::errors::NodecosmosError;
use crate::models::node::UpdateCreatorNode;
use crate::models::token::Token;
use crate::models::traits::{Clean, ElasticDocument, WhereInChunksExec};
use crate::models::udts::Address;
use crate::models::user_counter::UserCounter;

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

    #[serde(skip_serializing)]
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
            return Err(NodecosmosError::ValidationError(("username", "is taken")));
        }

        if self.username.len() < 3 {
            return Err(NodecosmosError::ValidationError((
                "username",
                "must be at least 3 characters",
            )));
        }

        if BLACKLIST_USERNAMES.contains(&self.username.as_str()) {
            return Err(NodecosmosError::ValidationError(("username", "is not allowed")));
        }

        if self.username.is_inappropriate() {
            return Err(NodecosmosError::ValidationError(("username", "is inappropriate")));
        }

        if self.first_name.is_inappropriate() {
            return Err(NodecosmosError::ValidationError(("firstName", "is inappropriate")));
        }

        if self.last_name.is_inappropriate() {
            return Err(NodecosmosError::ValidationError(("lastName", "is inappropriate")));
        }

        self.id = Uuid::new_v4();

        self.hash_password()?;

        if self.ctx == Some(UserContext::ConfirmInvitationTokenValid) {
            self.is_confirmed = true;
        } else if let Some(user) = Self::maybe_find_first_by_email(self.email.clone())
            .execute(db_session)
            .await?
        {
            // If the user is not confirmed, resend the confirmation email, otherwise don't disturb them
            if !user.is_confirmed && !user.is_blocked {
                let token = self.generate_confirmation_token(db_session).await?;

                app.mailer
                    .send_confirm_user_email(self.email.clone(), self.username.clone(), token.id.to_string())
                    .await?;
            }

            return Err(NodecosmosError::EmailAlreadyExists);
        } else {
            let token = self.generate_confirmation_token(db_session).await?;

            app.mailer
                .send_confirm_user_email(self.email.clone(), self.username.clone(), token.id.to_string())
                .await?;
        }

        Ok(())
    }

    async fn after_insert(&mut self, _db_session: &CachingSession, app: &Arc<App>) -> Result<(), NodecosmosError> {
        let _ = self.add_elastic_document(&app.elastic_client).await;

        Ok(())
    }

    async fn before_update(&mut self, _: &CachingSession, _: &Arc<App>) -> Result<(), NodecosmosError> {
        self.updated_at = Utc::now();

        Ok(())
    }

    async fn after_delete(&mut self, _: &CachingSession, app: &Arc<App>) -> Result<(), NodecosmosError> {
        let _ = self.update_elastic_document(&app.elastic_client).await;

        Ok(())
    }
}

impl User {
    pub async fn verify_password(&self, password: &str) -> Result<bool, NodecosmosError> {
        let res = verify(password, &self.password)
            .map_err(|_| NodecosmosError::ValidationError(("password", "is incorrect")))?;

        Ok(res)
    }

    pub async fn generate_confirmation_token(&self, db_session: &CachingSession) -> Result<Token, NodecosmosError> {
        let token = Token::new_user_confirmation(self.email.clone());
        token.insert().execute(db_session).await?;

        Ok(token)
    }

    pub async fn reset_password_token(&self, app: &Arc<App>) -> Result<(), NodecosmosError> {
        let token = Token::new_password_reset(self.email.clone());
        token.insert().execute(&app.db_session).await?;

        app.mailer
            .send_reset_password_email(self.email.clone(), self.username.clone(), token.id.to_string())
            .await?;

        Ok(())
    }

    pub async fn resend_token_count(&self, db_session: &CachingSession) -> Result<i64, NodecosmosError> {
        let count = UserCounter::maybe_find_first_by_id(self.id).execute(db_session).await?;

        if let Some(counter) = count {
            Ok(counter.resend_token_count.0)
        } else {
            Ok(0i64)
        }
    }

    fn hash_password(&mut self) -> Result<(), NodecosmosError> {
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

                let _ = self.update_elastic_document(data.elastic_client()).await;

                let user_id = self.id.clone();
                let data = data.clone();

                tokio::spawn(async move {
                    UpdateOwnerNode::update_owner_records(&data, user_id).await;
                    UpdateCreatorNode::update_creator_records(&data, user_id).await;
                });

                Ok(())
            }
        }
    };
}

impl_user_updated_at_with_elastic_ext_cb!(UpdateUser);
impl_user_updated_at_with_elastic_ext_cb!(UpdateProfileImageUser);

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

impl ShowUser {
    pub async fn find_by_ids(db_session: &CachingSession, ids: Set<Uuid>) -> Result<Vec<ShowUser>, NodecosmosError> {
        ids.where_in_chunked_query(db_session, |ids_chunk| find_show_user!("id IN ?", (ids_chunk,)))
            .await
            .try_collect()
            .await
    }
}

partial_user!(UpdateUser, id, first_name, last_name, updated_at, address);

partial_user!(UpdatePasswordUser, id, password, email, username, updated_at);

impl Callbacks for UpdatePasswordUser {
    type Extension = App;
    type Error = NodecosmosError;

    async fn before_update(&mut self, _: &CachingSession, _: &App) -> Result<(), NodecosmosError> {
        self.password = hash(&self.password, BCRYPT_COST).map_err(|_| {
            error!("{}", "error hashing password".bright_red().bold());

            NodecosmosError::InternalServerError(
                "There was an error processing your request. Please try again later.".to_string(),
            )
        })?;

        self.updated_at = Utc::now();

        Ok(())
    }

    async fn after_update(&mut self, _: &CachingSession, app: &App) -> Result<(), NodecosmosError> {
        app.mailer
            .send_password_changed_email(self.email.clone(), self.username.clone())
            .await?;

        Ok(())
    }
}

partial_user!(ConfirmUser, id, is_confirmed, email, updated_at);

impl Callbacks for ConfirmUser {
    type Extension = App;
    type Error = NodecosmosError;

    async fn before_update(&mut self, _: &CachingSession, _ext: &Self::Extension) -> Result<(), NodecosmosError> {
        self.updated_at = Utc::now();

        Ok(())
    }

    async fn after_update(&mut self, _: &CachingSession, app: &Self::Extension) -> Result<(), NodecosmosError> {
        use crate::models::traits::ElasticDocument;

        let _ = self.update_elastic_document(&app.elastic_client).await;

        Ok(())
    }
}

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
        self.bio.clean()?;

        Ok(())
    }

    async fn after_update(&mut self, _: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        let _ = self.update_elastic_document(data.elastic_client()).await;

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
