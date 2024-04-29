use charybdis::types::Uuid;
use scylla::CachingSession;

use crate::api::current_user::OptCurrentUser;
use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::node::AuthNode;
use crate::models::traits::Authorization;

/// We use auth node so we query only fields that are needed for authorization.
impl AuthNode {
    pub async fn auth_update(
        data: &RequestData,
        branch_id: Uuid,
        node_id: Uuid,
        root_id: Uuid,
    ) -> Result<(), NodecosmosError> {
        let mut node = AuthNode {
            branch_id,
            id: node_id,
            root_id,
            ..Default::default()
        };

        // `NodeAuthorization` derive
        node.auth_update(&data).await?;

        Ok(())
    }

    pub async fn auth_view(
        db_session: &CachingSession,
        opt_cu: &OptCurrentUser,
        branch_id: Uuid,
        node_id: Uuid,
        root_id: Uuid,
    ) -> Result<(), NodecosmosError> {
        let mut node = AuthNode {
            branch_id,
            id: node_id,
            root_id,
            ..Default::default()
        };

        node.auth_view(db_session, opt_cu).await?;

        Ok(())
    }
}
