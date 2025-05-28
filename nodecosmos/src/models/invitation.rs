use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::node::{Node, UpdateEditorsNode};
use crate::models::notification::{Notification, NotificationType};
use crate::models::token::Token;
use crate::models::user::User;
use charybdis::batch::ModelBatch;
use charybdis::callbacks::Callbacks;
use charybdis::macros::charybdis_model;
use charybdis::operations::{Find, Insert};
use charybdis::types::{Text, Timestamp, Uuid};
use chrono::Utc;
use email_address::EmailAddress;
use log::error;
use scylla::client::caching_session::CachingSession;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Default, PartialEq)]
pub enum InvitationContext {
    #[default]
    Default,
    Confirm,
}

#[derive(strum_macros::Display, strum_macros::EnumString)]
pub enum InvitationStatus {
    Created,
    Seen,
    Accepted,
    Rejected,
}

#[charybdis_model(
    table_name = invitations,
    partition_keys = [branch_id],
    clustering_keys = [node_id, username_or_email]
)]
#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Invitation {
    pub branch_id: Uuid,
    pub node_id: Uuid,
    pub username_or_email: Text,

    #[serde(default)]
    pub status: Text,

    #[serde(default)]
    pub inviter_id: Uuid,

    #[serde(default = "chrono::Utc::now")]
    pub created_at: Timestamp,

    #[serde(default = "chrono::Utc::now")]
    pub updated_at: Timestamp,

    #[serde(default)]
    pub expires_at: Timestamp,

    #[serde(skip)]
    #[charybdis(ignore)]
    pub node: Option<Node>,

    #[serde(skip)]
    #[charybdis(ignore)]
    pub user: Option<User>,

    #[serde(skip)]
    #[charybdis(ignore)]
    pub ctx: InvitationContext,

    #[serde(skip)]
    #[charybdis(ignore)]
    pub email: Option<Text>,
}

impl Callbacks for Invitation {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_insert(&mut self, _db_session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        let existing = self.maybe_find_by_primary_key().execute(data.db_session()).await?;
        let node = self.node(data.db_session()).await?;
        if let Some(existing) = existing {
            if existing.status == InvitationStatus::Accepted.to_string()
                && node
                    .editor_ids
                    .as_ref()
                    .is_some_and(|ids| ids.contains(&data.current_user.id))
            {
                return Err(NodecosmosError::PreconditionFailed("Invitation already accepted."));
            }

            if existing.expires_at > Utc::now() && existing.status == InvitationStatus::Created.to_string() {
                return Err(NodecosmosError::PreconditionFailed("Invitation already sent."));
            }
        }

        self.status = InvitationStatus::Created.to_string();
        self.inviter_id = data.current_user.id;
        self.expires_at = Utc::now() + chrono::Duration::weeks(1);

        if EmailAddress::is_valid(&self.username_or_email) {
            self.email = Some(self.username_or_email.clone());
            self.user = User::maybe_find_first_by_email(self.username_or_email.clone())
                .execute(data.db_session())
                .await?;
        } else {
            self.user = User::maybe_find_first_by_username(self.username_or_email.clone())
                .execute(data.db_session())
                .await?;

            if let Some(user) = &self.user {
                self.email = Some(user.email.clone());
            } else {
                return Err(NodecosmosError::ValidationError((
                    "usernameOrEmail",
                    "username not found",
                )));
            }
        };

        Ok(())
    }

    async fn after_insert(&mut self, _: &CachingSession, data: &RequestData) -> Result<(), Self::Error> {
        let node = self.node(data.db_session()).await?;
        let node_title = node.title.clone();
        let branch_id = node.branch_id;
        let node_id = node.id;
        let token;

        if EmailAddress::is_valid(&self.username_or_email) {
            // invite by email
            token = Token::new_invitation(self.username_or_email.clone(), None, (branch_id, node_id));
        } else {
            // invite by username
            if let Some(email) = &self.email {
                token = Token::new_invitation(
                    email.clone(),
                    Some(self.username_or_email.clone()),
                    (branch_id, node_id),
                );
            } else {
                error!("[after_insert] email must be present if username is not found");
                return Err(NodecosmosError::ValidationError(("usernameOrEmail", "email not found")));
            }
        }

        token.insert().execute(data.db_session()).await?;

        if let Some(email) = &self.email {
            data.mailer()
                .send_invitation_email(
                    email.clone(),
                    &data.current_user.username,
                    &node_title,
                    token.id.clone(),
                )
                .await?;

            if let Some(user) = self.user.as_ref() {
                let mut receiver_ids = HashSet::new();
                receiver_ids.insert(user.id);

                let _ = Notification::new(
                    NotificationType::NewInvitation,
                    format!("invited you to collaborate on the node {}", node_title),
                    format!("{}/invitations?token={}", data.app.config.client_url, token.id),
                    Some((&data.current_user).into()),
                )
                .create_for_receivers(data, receiver_ids)
                .await
                .map_err(|e| {
                    error!("Error while creating notification: {}", e);
                });
            }
        }

        Ok(())
    }

    async fn before_update(&mut self, _: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.ctx == InvitationContext::Confirm {
            let node = self.node(data.db_session()).await?;

            UpdateEditorsNode::update_editor_ids(data, node, &[data.current_user.id], &[]).await?;
        }

        Ok(())
    }

    async fn after_update(&mut self, _: &CachingSession, data: &RequestData) -> Result<(), Self::Error> {
        if self.ctx == InvitationContext::Confirm {
            let inviter = self.inviter(data.db_session()).await?;

            data.mailer()
                .send_invitation_accepted_email(
                    inviter.email.clone(),
                    &data.current_user.username,
                    self.branch_id,
                    self.node_id,
                    &self.node(data.db_session()).await?.title,
                )
                .await?;
        }

        Ok(())
    }
}

impl Invitation {
    pub async fn find_by_token(db_session: &CachingSession, token: String) -> Result<Invitation, NodecosmosError> {
        let token = Token::find_by_id(token).execute(db_session).await?;
        if let Some(node_pk) = token.node_pk {
            let username_or_email;

            if let Some(username) = token.username {
                username_or_email = username;
            } else {
                username_or_email = token.email;
            }

            let invitation = Self::find_by_primary_key_value((node_pk.0, node_pk.1, username_or_email))
                .execute(db_session)
                .await?;

            return Ok(invitation);
        }

        Err(NodecosmosError::NotFound("Invitation not found".to_string()))
    }

    pub async fn delete_by_editor_id(
        db_session: &CachingSession,
        branch_id: Uuid,
        node_id: Uuid,
        editor_id: Uuid,
    ) -> Result<(), NodecosmosError> {
        let editor = User::maybe_find_first_by_id(editor_id).execute(db_session).await?;

        if let Some(editor) = editor {
            let mut batch = DeleteIvitation::delete_batch();

            let by_username = DeleteIvitation {
                branch_id,
                node_id,
                username_or_email: editor.username,
            };

            let by_email = DeleteIvitation {
                branch_id,
                node_id,
                username_or_email: editor.email,
            };

            batch
                .append_delete(&by_username)
                .append_delete(&by_email)
                .execute(db_session)
                .await?;
        }

        Ok(())
    }

    pub async fn node(&mut self, db_session: &CachingSession) -> Result<&mut Node, NodecosmosError> {
        if self.node.is_none() {
            let node = Node::find_by_branch_id_and_id(self.branch_id, self.node_id)
                .execute(db_session)
                .await?;
            self.node = Some(node);
        }

        self.node
            .as_mut()
            .ok_or_else(|| NodecosmosError::NotFound("Node not found".to_string()))
    }

    pub async fn inviter(&self, db_session: &CachingSession) -> Result<User, NodecosmosError> {
        User::find_by_id(self.inviter_id)
            .execute(db_session)
            .await
            .map_err(NodecosmosError::from)
    }

    pub async fn invitee(&self, db_session: &CachingSession) -> Result<Option<User>, NodecosmosError> {
        if EmailAddress::is_valid(&self.username_or_email) {
            User::maybe_find_first_by_email(self.username_or_email.clone())
                .execute(db_session)
                .await
                .map_err(NodecosmosError::from)
        } else {
            User::maybe_find_first_by_username(self.username_or_email.clone())
                .execute(db_session)
                .await
                .map_err(NodecosmosError::from)
        }
    }
}

partial_invitation!(DeleteIvitation, branch_id, node_id, username_or_email);
