use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::node::AuthNode;
use crate::models::traits::Authorization;
use charybdis::types::Uuid;

/// We use auth node so we query only fields that are needed for authorization.
impl AuthNode {
    pub async fn auth_update(data: &RequestData, node_id: Uuid, branch_id: Uuid) -> Result<(), NodecosmosError> {
        let mut node = AuthNode {
            id: node_id,
            branch_id,
            ..Default::default()
        };

        // it comes from `NodeAuthorization` derive
        node.auth_update(&data).await?;

        Ok(())
    }
}
