use crate::api::current_user::OptCurrentUser;
use crate::api::data::RequestData;
use crate::app::App;
use crate::errors::NodecosmosError;
use crate::models::branch::{AuthBranch, Branch, BranchStatus};
use crate::models::comment::Comment;
use crate::models::comment_thread::{CommentObject, CommentThread};
use crate::models::contribution_request::ContributionRequest;
use crate::models::node::{AuthNode, Node};
use crate::models::traits::Branchable;
use crate::models::user::User;
use crate::models::workflow::Workflow;
use actix_web::web;
use charybdis::model::AsNative;
use charybdis::operations::Find;
use charybdis::types::{Set, Uuid};
use log::error;
use scylla::CachingSession;
use serde_json::json;

pub trait Authorization {
    async fn before_view_auth(&mut self, _db_session: &CachingSession) -> Result<(), NodecosmosError> {
        Ok(())
    }
    async fn before_update_auth(&mut self, _db_session: &CachingSession) -> Result<(), NodecosmosError> {
        Ok(())
    }

    fn is_frozen(&self) -> bool {
        false
    }

    fn is_public(&self) -> bool {
        false
    }

    fn owner_id(&self) -> Option<Uuid>;

    fn editor_ids(&self) -> Option<Set<Uuid>>;

    async fn auth_creation(&mut self, _data: &RequestData) -> Result<(), NodecosmosError>;

    async fn can_edit(&mut self, data: &RequestData) -> Result<bool, NodecosmosError> {
        if self.is_frozen() {
            return Err(NodecosmosError::Forbidden("This object is frozen!".to_string()));
        }

        if self.owner_id() == Some(data.current_user.id) {
            return Ok(true);
        }

        let editor_ids = self.editor_ids();
        if editor_ids.map_or(false, |ids| ids.contains(&data.current_user.id)) {
            return Ok(true);
        }

        Ok(false)
    }

    async fn auth_update(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        self.before_update_auth(data.db_session()).await?;

        if !self.can_edit(data).await? {
            return Err(NodecosmosError::Unauthorized(json!({
                "error": "Unauthorized!",
                "message": "You are not allowed to perform this action!"
            })));
        }

        Ok(())
    }

    async fn auth_view(&mut self, app: &web::Data<App>, current_user: OptCurrentUser) -> Result<(), NodecosmosError> {
        self.before_view_auth(&app.db_session).await?;

        if self.is_public() {
            return Ok(());
        }

        return match current_user.0 {
            Some(current_user) => {
                let req_data = RequestData::new(app.clone(), current_user);

                self.auth_update(&req_data).await?;

                Ok(())
            }
            None => Err(NodecosmosError::Unauthorized(json!({
                "error": "Unauthorized!",
                "message": "You must be logged in to perform this action!"
            }))),
        };
    }
}

/// Authorization of node actions follows simple rule:
/// If the node is a main branch, then the authorization is done by node.
/// Otherwise, the authorization is done by branch.
impl Authorization for Node {
    async fn before_update_auth(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        if self.is_original() {
            if self.owner_id.is_some() {
                return Ok(());
            }

            let auth_node = AuthNode::find_by_id_and_branch_id(self.id, self.id)
                .execute(db_session)
                .await?;
            self.owner_id = auth_node.owner_id;
            self.editor_ids = auth_node.editor_ids;
        } else {
            let branch = AuthBranch::find_by_id(self.branch_id).execute(db_session).await?;
            self.auth_branch = Some(branch);
        }

        Ok(())
    }

    fn is_frozen(&self) -> bool {
        if self.is_branched() {
            return match &self.auth_branch {
                Some(branch) => {
                    branch.status == Some(BranchStatus::Merged.to_string())
                        || branch.status == Some(BranchStatus::RecoveryFailed.to_string())
                        || branch.status == Some(BranchStatus::Closed.to_string())
                }
                None => {
                    error!("Branched node {} has no branch!", self.id);

                    false
                }
            };
        }

        false
    }

    fn is_public(&self) -> bool {
        self.is_public
    }

    fn owner_id(&self) -> Option<Uuid> {
        if self.is_original() {
            return self.owner_id;
        }

        return match &self.auth_branch {
            Some(branch) => Some(branch.owner_id),
            None => {
                error!("Branched node {} has no branch!", self.id);

                None
            }
        };
    }

    fn editor_ids(&self) -> Option<Set<Uuid>> {
        if self.is_original() {
            return self.editor_ids.clone();
        }

        return match &self.auth_branch {
            Some(branch) => branch.editor_ids.clone(),
            None => {
                error!("Branched node {} has no branch!", self.id);

                None
            }
        };
    }

    async fn auth_creation(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.id != Uuid::default() {
            return Err(NodecosmosError::Unauthorized(json!({
                "error": "Bad Request!",
                "message": "Cannot create node with id!"
            })));
        }

        if self.is_branched() {
            self.auth_update(data).await?;
        } else if let Some(parent) = self.parent(data.db_session()).await? {
            parent.as_native().auth_update(data).await?;
        }

        Ok(())
    }
}

impl Authorization for Workflow {
    async fn before_update_auth(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        self.init_node(db_session).await?;

        Ok(())
    }

    fn is_public(&self) -> bool {
        if let Some(node) = &self.node {
            return node.is_public;
        }

        error!("Workflow {} is missing node {}", self.id, self.node_id);

        false
    }

    fn owner_id(&self) -> Option<Uuid> {
        if let Some(node) = &self.node {
            return node.owner_id;
        }

        error!("Workflow {} is missing node {}", self.id, self.node_id);

        None
    }

    fn editor_ids(&self) -> Option<Set<Uuid>> {
        if let Some(node) = &self.node {
            return node.editor_ids.clone();
        }

        error!("Workflow {} is missing node {}", self.id, self.node_id);

        None
    }

    async fn auth_creation(&mut self, _data: &RequestData) -> Result<(), NodecosmosError> {
        self.auth_update(_data).await
    }
}

impl Authorization for Branch {
    fn is_public(&self) -> bool {
        self.is_public
    }

    fn owner_id(&self) -> Option<Uuid> {
        Some(self.owner_id)
    }

    fn editor_ids(&self) -> Option<Set<Uuid>> {
        self.editor_ids.clone()
    }

    async fn auth_creation(&mut self, _data: &RequestData) -> Result<(), NodecosmosError> {
        Err(NodecosmosError::Forbidden("Branches cannot be created".to_string()))
    }
}

impl Authorization for ContributionRequest {
    async fn before_view_auth(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        self.branch(db_session).await?;

        Ok(())
    }

    async fn before_update_auth(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        self.branch(db_session).await?;

        Ok(())
    }

    fn is_public(&self) -> bool {
        let branch = self.branch.borrow_mut().clone();

        branch.map_or(false, |branch| branch.is_public)
    }

    fn owner_id(&self) -> Option<Uuid> {
        let branch = self.branch.borrow_mut().clone();

        branch.map(|branch| branch.owner_id)
    }

    fn editor_ids(&self) -> Option<Set<Uuid>> {
        let branch = self.branch.borrow_mut().clone();

        branch.map(|branch| branch.editor_ids.clone().unwrap_or_default())
    }

    async fn auth_creation(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let node = self.node(data.db_session()).await?;

        if node.is_public {
            return Ok(());
        }

        node.auth_update(data).await?;

        Ok(())
    }
}

impl Authorization for User {
    fn owner_id(&self) -> Option<Uuid> {
        Some(self.id)
    }

    fn editor_ids(&self) -> Option<Set<Uuid>> {
        None
    }

    async fn auth_creation(&mut self, _data: &RequestData) -> Result<(), NodecosmosError> {
        Ok(())
    }
}

impl Authorization for CommentThread {
    fn owner_id(&self) -> Option<Uuid> {
        self.author_id
    }

    fn editor_ids(&self) -> Option<Set<Uuid>> {
        None
    }

    async fn auth_creation(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let object = self.object(data.db_session()).await?;
        match object {
            CommentObject::ContributionRequest(mut contribution_request) => {
                contribution_request
                    .auth_view(&data.app, OptCurrentUser(Option::from(data.current_user.clone())))
                    .await?;

                Ok(())
            }
        }
    }
}

impl Authorization for Comment {
    fn owner_id(&self) -> Option<Uuid> {
        self.author_id
    }

    fn editor_ids(&self) -> Option<Set<Uuid>> {
        None
    }

    async fn auth_creation(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        return match self.thread(data.db_session()).await? {
            Some(thread) => thread.auth_creation(data).await,
            None => Err(NodecosmosError::Forbidden("Comment must have a thread!".to_string())),
        };
    }
}
