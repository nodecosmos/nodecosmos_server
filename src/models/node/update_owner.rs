use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::materialized_views::nodes_by_owner::NodesByOwner;
use crate::models::node::UpdateOwnerNode;
use crate::models::udts::{Owner, OwnerType};
use crate::models::user::{FullName, User};
use crate::services::elastic::ElasticDocument;
use crate::utils::logger::{log_error, log_fatal};
use charybdis::batch::CharybdisModelBatch;
use charybdis::types::Uuid;
use futures::StreamExt;

impl UpdateOwnerNode {
    fn init(nodes_by_owner: &NodesByOwner, owner: Owner) -> Self {
        Self {
            id: nodes_by_owner.id,
            branch_id: nodes_by_owner.branch_id,
            owner_id: Some(nodes_by_owner.owner_id),
            owner: Some(owner),
            updated_at: Some(chrono::Utc::now()),
        }
    }

    pub async fn update_owner_records(data: &RequestData, user_id: Uuid) {
        let user = User::find_by_id(user_id).execute(data.db_session()).await;

        match user {
            Ok(user) => {
                let _ = UpdateOwnerNode::run(data, user.clone()).await;
            }
            Err(e) => {
                log_error(format!("Error find_by_id: {}", e));
            }
        }
    }

    async fn run(data: &RequestData, user: User) -> Result<(), NodecosmosError> {
        let mut nodes_by_owner = NodesByOwner::find_by_owner_id(user.id)
            .execute(data.db_session())
            .await
            .map_err(|e| {
                log_error(format!("Error finding nodes by owner: {}", e));
                e
            })?;

        let owner = Owner::init(&user);
        let mut nodes_to_update = vec![];

        while let Some(node_by_owner) = nodes_by_owner.next().await {
            nodes_to_update.push(UpdateOwnerNode::init(
                &node_by_owner.map_err(|e| {
                    log_error(format!("Error init: {}", e));
                    e
                })?,
                owner.clone(),
            ))
        }

        UpdateOwnerNode::bulk_update_elastic_documents(data.elastic_client(), nodes_to_update.clone()).await;
        CharybdisModelBatch::chunked_insert(data.db_session(), nodes_to_update, 100)
            .await
            .map_err(|e| {
                log_error(format!("Error chunked_insert: {}", e));
                e
            })?;

        Ok(())
    }
}
