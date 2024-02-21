use crate::api::current_user::OptCurrentUser;
use crate::api::data::RequestData;
use crate::app::App;
use crate::errors::NodecosmosError;
use crate::models::branch::Branch;
use crate::models::comment::{Comment, CommentObject};
use crate::models::contribution_request::ContributionRequest;
use crate::models::node::Node;
use crate::models::traits::Branchable;
use crate::models::user::User;
use crate::utils::logger::log_fatal;
use actix_web::web;
use charybdis::model::AsNative;
use charybdis::operations::Find;
use charybdis::types::{Set, Uuid};
use serde_json::json;

pub trait Authorization {
    async fn before_auth(&mut self, _data: &RequestData) -> Result<(), NodecosmosError> {
        Ok(())
    }

    fn is_public(&self) -> bool {
        false
    }

    fn owner_id(&self) -> Option<Uuid>;

    fn editor_ids(&self) -> Option<Set<Uuid>>;

    async fn auth_creation(&mut self, _data: &RequestData) -> Result<(), NodecosmosError>;

    async fn can_edit(&mut self, data: &RequestData) -> Result<bool, NodecosmosError> {
        self.before_auth(data).await?;

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
        if !self.can_edit(data).await? {
            return Err(NodecosmosError::Unauthorized(json!({
                "error": "Unauthorized!",
                "message": "You are not allowed to perform this action!"
            })));
        }

        Ok(())
    }

    async fn auth_view(&mut self, app: &web::Data<App>, current_user: OptCurrentUser) -> Result<(), NodecosmosError> {
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
    async fn before_auth(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_original() {
            *self = self.find_by_primary_key().execute(data.db_session()).await?;
        } else {
            self.init_auth_branch(data.db_session()).await?;
        }

        Ok(())
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
                log_fatal(format!("Branched node {} has no branch!", self.id));

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
                log_fatal(format!("Branched node {} has no branch!", self.id));

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
    async fn before_auth(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        self.branch(data.db_session()).await?;

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

impl Authorization for Comment {
    fn owner_id(&self) -> Option<Uuid> {
        Some(self.author_id)
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

            _ => Err(NodecosmosError::NotFound("Object not found".to_string())),
        }
    }
}
