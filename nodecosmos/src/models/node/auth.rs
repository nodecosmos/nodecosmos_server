use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::node::AuthNode;
use crate::models::traits::Authorization;
use crate::models::traits::Branchable;
use charybdis::model::AsNative;
use charybdis::types::Uuid;

/// We use auth node so we query only fields that are needed for authorization.
impl AuthNode {
    pub async fn auth_update(data: &RequestData, node_id: Uuid, branch_id: Uuid) -> Result<(), NodecosmosError> {
        let mut native = AuthNode {
            id: node_id,
            branch_id,
            ..Default::default()
        }
        .as_native();

        if native.is_original() {
            native = AuthNode::find_by_id_and_branch_id(node_id, branch_id)
                .execute(data.db_session())
                .await?
                .as_native();
        }

        native.auth_update(&data).await?;

        Ok(())
    }
}
