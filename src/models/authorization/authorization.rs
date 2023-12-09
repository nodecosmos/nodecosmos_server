use crate::api::current_user::OptCurrentUser;
use crate::api::data::RequestData;
use crate::app::App;
use crate::errors::NodecosmosError;
use actix_web::web;
use charybdis::types::{Set, Uuid};
use serde_json::json;

pub trait Authorization {
    async fn before_auth(&mut self, _data: &RequestData) -> Result<(), NodecosmosError> {
        Ok(())
    }

    fn is_public(&self) -> bool;

    fn owner_id(&self) -> Option<Uuid>;

    fn editor_ids(&self) -> Option<Set<Uuid>>;

    async fn auth_creation(&mut self, _data: &RequestData) -> Result<(), NodecosmosError> {
        Ok(())
    }

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
