use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::node::{AuthNode, Node};
use crate::models::traits::Authorization;
use crate::models::traits::Branchable;
use crate::utils::logger::log_fatal;
use charybdis::model::AsNative;
use charybdis::operations::Find;
use charybdis::types::{Set, Uuid};
use serde_json::json;

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
            native = AuthNode::find_by_id_and_branch_id(data.db_session(), node_id, branch_id)
                .await?
                .as_native();
        }

        native.auth_update(&data).await?;

        Ok(())
    }
}
