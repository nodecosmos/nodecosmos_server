mod fields;
pub use fields::AuthorizationFields;

use crate::api::data::RequestData;
use crate::api::request::current_user::OptCurrentUser;
use crate::app::App;
use crate::errors::NodecosmosError;
use crate::models::branch::Branch;
use crate::models::comment::Comment;
use crate::models::comment_thread::{CommentObject, CommentThread};
use crate::models::contribution_request::ContributionRequest;
use crate::models::user::User;
use crate::models::workflow::Workflow;
use actix_web::web;
use charybdis::operations::Find;
use scylla::CachingSession;
use serde_json::json;

/// Authorization for nodes is implemented with the `NodeAuthorization` derive macro.
pub trait Authorization: AuthorizationFields {
    async fn init_auth_info(&mut self, _db_session: &CachingSession) -> Result<(), NodecosmosError> {
        Ok(())
    }

    async fn auth_creation(&mut self, _data: &RequestData) -> Result<(), NodecosmosError>;

    async fn can_edit(&mut self, data: &RequestData) -> Result<bool, NodecosmosError> {
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
        self.init_auth_info(data.db_session()).await?;

        if self.is_frozen() {
            return Err(NodecosmosError::Forbidden("This object is frozen!".to_string()));
        }

        if !self.can_edit(data).await? {
            return Err(NodecosmosError::Unauthorized(json!({
                "error": "Unauthorized!",
                "message": "You are not allowed to perform this action!"
            })));
        }

        Ok(())
    }

    async fn auth_deletion(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        self.auth_update(data).await
    }

    async fn auth_view(&mut self, app: &web::Data<App>, current_user: OptCurrentUser) -> Result<(), NodecosmosError> {
        self.init_auth_info(&app.db_session).await?;

        if self.is_public() {
            return Ok(());
        }

        return match current_user.0 {
            Some(current_user) => {
                let data = RequestData::new(app.clone(), current_user);

                self.auth_update(&data).await?;

                Ok(())
            }
            None => Err(NodecosmosError::Unauthorized(json!({
                "error": "Unauthorized!",
                "message": "You must be logged in to perform this action!"
            }))),
        };
    }
}

impl Authorization for Workflow {
    async fn init_auth_info(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        self.init_node(db_session).await?;

        Ok(())
    }

    async fn auth_creation(&mut self, _data: &RequestData) -> Result<(), NodecosmosError> {
        self.auth_update(_data).await
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
    async fn init_auth_info(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        if self.author_id.is_none() {
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
