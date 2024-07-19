use charybdis::batch::ModelBatch;
use charybdis::types::Uuid;
use futures::StreamExt;
use log::error;

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::materialized_views::nodes_by_creator::NodesByCreator;
use crate::models::node::UpdateCreatorNode;
use crate::models::traits::ElasticDocument;
use crate::models::udts::Profile;
use crate::models::user::User;

impl UpdateCreatorNode {
    fn init(nodes_by_creator: &NodesByCreator, creator: Profile) -> Self {
        Self {
            id: nodes_by_creator.id,
            branch_id: nodes_by_creator.branch_id,
            root_id: nodes_by_creator.root_id,
            creator_id: Some(nodes_by_creator.creator_id),
            creator: Some(creator),
            updated_at: chrono::Utc::now(),
        }
    }

    pub async fn update_creator_records(data: &RequestData, user_id: Uuid) {
        let user = User::find_by_id(user_id).execute(data.db_session()).await;

        match user {
            Ok(user) => {
                let _ = Self::run(data, user.clone()).await;
            }
            Err(e) => {
                error!("[update_creator_records::find_by_id] {}", e);
            }
        }
    }

    async fn run(data: &RequestData, user: User) -> Result<(), NodecosmosError> {
        let mut nodes_by_creator = NodesByCreator::find_by_creator_id(user.id)
            .execute(data.db_session())
            .await
            .map_err(|e| {
                error!("[run::find_by_creator_id]: {}", e);
                e
            })?;

        let creator = Profile::init(&user);
        let mut nodes_to_update = vec![];

        while let Some(node_by_creator) = nodes_by_creator.next().await {
            nodes_to_update.push(UpdateCreatorNode::init(
                &node_by_creator.map_err(|e| {
                    error!("[node_by_creator] {}", e);
                    e
                })?,
                creator.clone(),
            ))
        }

        UpdateCreatorNode::bulk_update_elastic_documents(data.elastic_client(), &nodes_to_update).await;

        Self::unlogged_batch()
            .chunked_insert(data.db_session(), &nodes_to_update, crate::constants::BATCH_CHUNK_SIZE)
            .await
            .map_err(|e| {
                error!("[run::chunked_insert] {}", e);
                e
            })?;

        Ok(())
    }
}
