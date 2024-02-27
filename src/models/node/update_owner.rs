use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::materialized_views::nodes_by_owner::NodesByProfile;
use crate::models::node::UpdateProfileNode;
use crate::models::udts::{Profile, ProfileType};
use crate::models::user::{FullName, User};
use crate::services::elastic::ElasticDocument;
use charybdis::batch::{CharybdisModelBatch, ModelBatch};
use charybdis::types::Uuid;
use futures::StreamExt;
use log::error;

impl UpdateProfileNode {
    fn init(nodes_by_owner: &NodesByProfile, owner: Profile) -> Self {
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
                let _ = UpdateProfileNode::run(data, user.clone()).await;
            }
            Err(e) => {
                error!("[update_owner_records::find_by_id] {}", e);
            }
        }
    }

    async fn run(data: &RequestData, user: User) -> Result<(), NodecosmosError> {
        let mut nodes_by_owner = NodesByProfile::find_by_owner_id(user.id)
            .execute(data.db_session())
            .await
            .map_err(|e| {
                error!("[run::find_by_owner_id]: {}", e);
                e
            })?;

        let owner = Profile::init(&user);
        let mut nodes_to_update = vec![];

        while let Some(node_by_owner) = nodes_by_owner.next().await {
            nodes_to_update.push(UpdateProfileNode::init(
                &node_by_owner.map_err(|e| {
                    error!("[node_by_owner] {}", e);
                    e
                })?,
                owner.clone(),
            ))
        }

        UpdateProfileNode::bulk_update_elastic_documents(data.elastic_client(), nodes_to_update.clone()).await;
        Self::unlogged_batch()
            .chunked_insert(data.db_session(), &nodes_to_update, 100)
            .await
            .map_err(|e| {
                error!("[run::chunked_insert] {}", e);
                e
            })?;

        Ok(())
    }
}
