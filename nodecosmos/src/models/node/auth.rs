use crate::api::current_user::OptCurrentUser;
use crate::api::data::RequestData;
use crate::app::App;
use crate::errors::NodecosmosError;
use crate::models::node::AuthNode;
use crate::models::traits::Authorization;
use actix_web::web;
use charybdis::types::Uuid;

/// We use auth node so we query only fields that are needed for authorization.
impl AuthNode {
    pub async fn auth_update(data: &RequestData, node_id: Uuid, branch_id: Uuid) -> Result<(), NodecosmosError> {
        let mut node = AuthNode {
            id: node_id,
            branch_id,
            ..Default::default()
        };

        // `NodeAuthorization` derive
        node.auth_update(&data).await?;

        Ok(())
    }

    pub async fn auth_view(
        app: &web::Data<App>,
        opt_cu: OptCurrentUser,
        node_id: Uuid,
        branch_id: Uuid,
    ) -> Result<(), NodecosmosError> {
        let mut node = AuthNode {
            id: node_id,
            branch_id,
            ..Default::default()
        };

        node.auth_view(app, opt_cu).await?;

        Ok(())
    }
}
