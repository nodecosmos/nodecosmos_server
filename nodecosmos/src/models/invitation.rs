use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::node::Node;
use crate::models::token::Token;
use crate::models::user::User;
use charybdis::callbacks::Callbacks;
use charybdis::macros::charybdis_model;
use charybdis::operations::{Find, Insert};
use charybdis::types::{Boolean, Text, Timestamp, Uuid};
use chrono::Utc;
use scylla::CachingSession;
use serde::{Deserialize, Serialize};

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
    clustering_keys = [node_id, email]
)]
#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Invitation {
    pub id: Uuid,
    pub email: Text,
    pub seen: Boolean,
    pub status: Text,
    pub inviter_id: Uuid,
    pub branch_id: Uuid,
    pub node_id: Uuid,
    pub created_at: Timestamp,
    pub expires_at: Timestamp,
    pub updated_at: Timestamp,

    #[serde(skip)]
    #[charybdis(ignore)]
    pub node: Option<Node>,

    #[serde(skip)]
    #[charybdis(ignore)]
    pub ctx: InvitationContext,
}

impl Callbacks for Invitation {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_insert(&mut self, _db_session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        self.status = InvitationStatus::Created.to_string();
        self.inviter_id = data.current_user.id;
        self.expires_at = Utc::now() + chrono::Duration::weeks(1);

        let email = self.email.clone();
        let node = self.node(data.db_session()).await?;
        let token = Token::new_invitation(email.clone(), (node.branch_id, node.id));

        token.insert().execute(data.db_session()).await?;

        data.mailer()
            .send_invitation_email(email, &data.current_user.username, &node.title, token.id)
            .await?;

        Ok(())
    }

    async fn before_update(&mut self, _db_session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.ctx == InvitationContext::Confirm {
            let node = self.node(data.db_session()).await?;
            node.push_editor_ids(vec![data.current_user.id])
                .execute(data.db_session())
                .await?;
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
            let invitation = Self::find_by_primary_key_value(&(node_pk.0, node_pk.1, token.email))
                .execute(db_session)
                .await?;

            return Ok(invitation);
        }

        Err(NodecosmosError::NotFound("Invitation not found".to_string()))
    }

    pub async fn node(&mut self, db_session: &CachingSession) -> Result<&mut Node, NodecosmosError> {
        if self.node.is_none() {
            let node = Node::find_by_branch_id_and_id(self.branch_id, self.node_id)
                .execute(db_session)
                .await?;
            self.node = Some(node);
        }

        Ok(self.node.as_mut().unwrap())
    }

    pub async fn inviter(&self, db_session: &CachingSession) -> Result<User, NodecosmosError> {
        User::find_by_id(self.inviter_id)
            .execute(db_session)
            .await
            .map_err(NodecosmosError::from)
    }
}
