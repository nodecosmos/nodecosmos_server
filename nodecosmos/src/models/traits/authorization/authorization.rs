use charybdis::operations::Find;
use scylla::CachingSession;
use uuid::Uuid;

use crate::api::data::RequestData;
use crate::api::request::current_user::OptCurrentUser;
use crate::errors::NodecosmosError;
use crate::models::branch::Branch;
use crate::models::comment::Comment;
use crate::models::comment_thread::{CommentObject, CommentThread};
use crate::models::contribution_request::ContributionRequest;
use crate::models::invitation::{Invitation, InvitationStatus};
use crate::models::traits::AuthorizationFields;
use crate::models::user::User;

/// Authorization for nodes is implemented with the `NodeAuthorization` derive.
pub trait Authorization: AuthorizationFields {
    async fn init_auth_info(&mut self, _db_session: &CachingSession) -> Result<(), NodecosmosError> {
        Ok(())
    }

    async fn auth_creation(&mut self, _data: &RequestData) -> Result<(), NodecosmosError>;

    fn can_edit(&mut self, data: &RequestData) -> bool {
        if self.owner_id() == Some(data.current_user.id) {
            return true;
        }

        if self
            .editor_ids()
            .as_ref()
            .map_or(false, |ids| ids.contains(&data.current_user.id))
        {
            return true;
        }

        false
    }

    async fn auth_update(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if !data.current_user.is_confirmed {
            return Err(NodecosmosError::Unauthorized("User is not confirmed"));
        }

        if data.current_user.is_blocked {
            return Err(NodecosmosError::Unauthorized("User is blocked"));
        }

        self.init_auth_info(data.db_session()).await?;

        if self.is_frozen() {
            return Err(NodecosmosError::Forbidden("This object is frozen!".to_string()));
        }

        if !self.can_edit(data) {
            return Err(NodecosmosError::Unauthorized(
                "You are not allowed to update this resource!",
            ));
        }

        Ok(())
    }

    async fn auth_view(
        &mut self,
        db_session: &CachingSession,
        current_user: &OptCurrentUser,
    ) -> Result<(), NodecosmosError> {
        self.init_auth_info(&db_session).await?;

        if self.is_public() {
            return Ok(());
        }

        return match &current_user.0 {
            Some(current_user) => {
                if self.owner_id() == Some(current_user.id)
                    || self
                        .viewer_ids()
                        .as_ref()
                        .map_or(false, |ids| ids.contains(&current_user.id))
                {
                    return Ok(());
                }

                Err(NodecosmosError::Unauthorized(
                    "You are not allowed to view this resource!",
                ))
            }
            None => Err(NodecosmosError::Unauthorized(
                "You are not allowed to view this resource!",
            )),
        };
    }
}

impl Authorization for Branch {
    async fn auth_creation(&mut self, _data: &RequestData) -> Result<(), NodecosmosError> {
        Err(NodecosmosError::Forbidden("Branches cannot be created".to_string()))
    }
}

impl Authorization for ContributionRequest {
    async fn init_auth_info(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        self.branch(db_session).await?;

        Ok(())
    }

    async fn auth_creation(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if !data.current_user.is_confirmed {
            return Err(NodecosmosError::Unauthorized("User is not confirmed"));
        }

        if data.current_user.is_blocked {
            return Err(NodecosmosError::Unauthorized("User is blocked"));
        }

        let node = self.node(data.db_session()).await?;

        if node.is_public {
            return Ok(());
        }

        node.auth_update(data).await?;

        Ok(())
    }
}

impl Authorization for User {
    async fn auth_creation(&mut self, _data: &RequestData) -> Result<(), NodecosmosError> {
        Ok(())
    }
}

impl Authorization for CommentThread {
    async fn auth_creation(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if !data.current_user.is_confirmed {
            return Err(NodecosmosError::Unauthorized("User is not confirmed"));
        }

        if data.current_user.is_blocked {
            return Err(NodecosmosError::Unauthorized("User is blocked"));
        }

        let object = self.object(data.db_session()).await?;
        match object {
            CommentObject::ContributionRequest(mut contribution_request) => {
                contribution_request
                    .auth_view(
                        data.db_session(),
                        &OptCurrentUser(Option::from(data.current_user.clone())),
                    )
                    .await?;

                Ok(())
            }
        }
    }
}

impl Authorization for Comment {
    async fn init_auth_info(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        if self.author_id == Uuid::default() {
            *self = self.find_by_primary_key().execute(db_session).await?;
        }

        Ok(())
    }

    async fn auth_creation(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        return match self.thread(data.db_session()).await? {
            Some(thread) => thread.auth_creation(data).await,
            None => Err(NodecosmosError::Forbidden("Comment must have a thread!".to_string())),
        };
    }
}

impl Authorization for Invitation {
    async fn init_auth_info(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        self.node(db_session).await?;

        Ok(())
    }

    async fn auth_creation(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        self.node(data.db_session()).await?.auth_update(data).await?;

        Ok(())
    }

    async fn auth_update(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.username_or_email != data.current_user.email && self.username_or_email != data.current_user.username {
            return Err(NodecosmosError::Unauthorized("Unauthorized"));
        }

        if self.status == InvitationStatus::Accepted.to_string() {
            return Err(NodecosmosError::Unauthorized("Invitation already accepted."));
        }

        if self.expires_at < chrono::Utc::now() {
            return Err(NodecosmosError::Unauthorized(
                "Invitation expired. Please request a new invitation.",
            ));
        }

        Ok(())
    }
}
