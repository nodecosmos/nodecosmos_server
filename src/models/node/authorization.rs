use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::authorization::Authorization;
use crate::models::branch::branchable::Branchable;
use crate::models::node::{AuthNode, Node};
use crate::utils::logger::log_fatal;
use charybdis::model::AsNative;
use charybdis::operations::Find;
use charybdis::types::{Set, Uuid};
use serde_json::json;

/// Authorization of node actions follows simple rule:
/// If the node is a main branch, then the authorization is done by node.
/// Otherwise, the authorization is done by branch.
impl Authorization for Node {
    async fn before_auth(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_original() {
            *self = self.find_by_primary_key(data.db_session()).await?;
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
