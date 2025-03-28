use charybdis::operations::Find;
use scylla::client::caching_session::CachingSession;
use uuid::Uuid;

use crate::api::data::RequestData;
use crate::api::request::current_user::OptCurrentUser;
use crate::errors::NodecosmosError;
use crate::models::branch::{AuthBranch, Branch};
use crate::models::comment::Comment;
use crate::models::comment_thread::{CommentThread, ThreadObjectType};
use crate::models::contribution_request::ContributionRequest;
use crate::models::invitation::{Invitation, InvitationStatus};
use crate::models::node::{AuthNode, BaseNode, Node, UpdateTitleNode};
use crate::models::traits::{AuthorizationFields, Branchable};
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

        if !self.is_subscription_active() {
            return Err(NodecosmosError::Forbidden("Subscription is not active!".to_string()));
        }

        if !self.can_edit(data) {
            return Err(NodecosmosError::Unauthorized(
                "You are not allowed to update this resource!",
            ));
        }

        Ok(())
    }

    async fn auth_delete(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
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
        self.init_auth_info(db_session).await?;

        if self.is_public() {
            return Ok(());
        }

        match &current_user.0 {
            Some(current_user) => {
                if self.owner_id() == Some(current_user.id)
                    || self
                        .viewer_ids()
                        .as_ref()
                        .map_or(false, |ids| ids.contains(&current_user.id))
                    || self
                        .editor_ids()
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
        }
    }
}

macro_rules! imp_authorization_for_node_types {
    ($struct_name:ident) => {
        impl Authorization for $struct_name {
            async fn init_auth_info(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
                if self.is_original() {
                    // auth info is already initialized
                    if self.owner_id != Uuid::default() {
                        return Ok(());
                    }

                    let auth_node = AuthNode::find_by_branch_id_and_id(self.branch_id, self.id)
                        .execute(db_session)
                        .await?;

                    self.root_id = auth_node.root_id;
                    self.owner_id = auth_node.owner_id;
                    self.editor_ids = auth_node.editor_ids;
                    self.viewer_ids = auth_node.viewer_ids;
                    self.is_public = auth_node.is_public;
                    self.is_subscription_active = auth_node.is_subscription_active;
                    self.parent_id = auth_node.parent_id;
                } else {
                    // authorize by branch
                    let branch = AuthBranch::find_by_id(self.branch_id).execute(db_session).await?;
                    self.auth_branch = Some(Box::new(branch));
                }

                Ok(())
            }

            async fn auth_creation(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
                if !data.current_user.is_confirmed {
                    return Err(NodecosmosError::Unauthorized("User is not confirmed"));
                }

                if data.current_user.is_blocked {
                    return Err(NodecosmosError::Unauthorized("User is blocked"));
                }

                if self.id != Uuid::default() {
                    return Err(NodecosmosError::Unauthorized("Cannot create node with id"));
                }

                if self.is_branch() {
                    self.auth_update(data).await?;
                } else if let Some(parent_id) = self.parent_id {
                    let mut auth_parent_node = AuthNode::find_by_branch_id_and_id(self.original_id(), parent_id)
                        .execute(data.db_session())
                        .await?;

                    auth_parent_node.auth_update(data).await?;
                } else if !self.is_public() && data.stripe_cfg().is_some() {
                    return Err(NodecosmosError::Forbidden(
                        "You cannot create private node without a subscription!".to_string(),
                    ));
                }

                Ok(())
            }
        }
    };
}

imp_authorization_for_node_types!(Node);
imp_authorization_for_node_types!(AuthNode);
imp_authorization_for_node_types!(BaseNode);
imp_authorization_for_node_types!(UpdateTitleNode);

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

        if node.is_subscription_active() {
            Ok(())
        } else {
            Err(NodecosmosError::Forbidden("Subscription is not active!".to_string()))
        }
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

        match self.thread_object_type() {
            Ok(ThreadObjectType::ContributionRequest) => {
                let branch = self.branch(data.db_session()).await?;
                branch
                    .auth_view(
                        data.db_session(),
                        &OptCurrentUser(Option::from(data.current_user.clone())),
                    )
                    .await
            }
            Ok(ThreadObjectType::Thread) => {
                let node = self.node(data.db_session()).await?;

                node.auth_view(
                    data.db_session(),
                    &OptCurrentUser(Option::from(data.current_user.clone())),
                )
                .await
            }
            Err(e) => Err(NodecosmosError::NotFound(format!(
                "Error getting thread object_type: {}",
                e
            ))),
        }
    }

    async fn auth_update(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if !data.current_user.is_confirmed {
            return Err(NodecosmosError::Unauthorized("User is not confirmed"));
        }

        if data.current_user.is_blocked {
            return Err(NodecosmosError::Unauthorized("User is blocked"));
        }

        match self.thread_object_type() {
            Ok(ThreadObjectType::ContributionRequest) => {
                let branch = self.branch(data.db_session()).await?;
                branch.auth_update(data).await
            }
            Ok(ThreadObjectType::Thread) => {
                let node = self.node(data.db_session()).await?;

                node.auth_update(data).await
            }
            Err(e) => Err(NodecosmosError::NotFound(format!(
                "Error getting thread object_type: {}",
                e
            ))),
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
        match self.thread(data.db_session()).await? {
            Some(thread) => thread.auth_creation(data).await,
            None => Err(NodecosmosError::Forbidden("Comment must have a thread!".to_string())),
        }
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
