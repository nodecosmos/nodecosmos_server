use charybdis::macros::charybdis_model;
use charybdis::operations::Find;
use charybdis::stream::CharybdisModelStream;
use charybdis::types::{Counter, Set, Uuid};
use nodecosmos_macros::Branchable;
use scylla::CachingSession;
use serde::{Deserialize, Serialize};

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::like::likeable::Likeable;
use crate::models::like::Like;
use crate::models::traits::{Branchable, ElasticDocument, UpdateLikeCountNodeElasticIdx};

#[charybdis_model(
    table_name = node_counters,
    partition_keys = [branch_id],
    clustering_keys = [id],
    global_secondary_indexes = []
)]
#[derive(Serialize, Deserialize, Branchable, Default, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NodeCounter {
    pub branch_id: Uuid,

    #[branch(original_id)]
    pub id: Uuid,

    pub like_count: Option<Counter>,
    pub descendants_count: Option<Counter>,
}

impl NodeCounter {
    pub async fn find_by_ids(
        db_session: &CachingSession,
        branch_id: Uuid,
        ids: &Set<Uuid>,
    ) -> Result<CharybdisModelStream<NodeCounter>, NodecosmosError> {
        find_node_counter!("branch_id = ? AND id IN ?", (branch_id, ids))
            .execute(db_session)
            .await
            .map_err(NodecosmosError::from)
    }

    pub async fn update_elastic_document(data: &RequestData, branch_id: Uuid, id: Uuid) {
        let data = data.clone();
        tokio::spawn(async move {
            match Self::like_count(data.db_session(), branch_id, id).await {
                Ok(lc) => {
                    UpdateLikeCountNodeElasticIdx {
                        id,
                        likes_count: lc as i32,
                    }
                    .update_elastic_document(data.elastic_client())
                    .await;
                }
                Err(e) => {
                    log::error!("Error getting like count: {:?}", e);

                    return;
                }
            };
        });
    }
}

impl Likeable for NodeCounter {
    async fn increment_like(data: &RequestData, like: &Like) -> Result<(), NodecosmosError> {
        Self {
            id: like.object_id,
            branch_id: like.branch_id,
            ..Default::default()
        }
        .increment_like_count(1)
        .execute(data.db_session())
        .await?;

        if like.is_branch() {
            Self::update_elastic_document(data, like.branch_id, like.object_id).await;
        }

        Ok(())
    }

    async fn decrement_like(data: &RequestData, like: &Like) -> Result<(), NodecosmosError> {
        Self {
            id: like.object_id,
            branch_id: like.branch_id,
            ..Default::default()
        }
        .decrement_like_count(1)
        .execute(data.db_session())
        .await?;

        if like.is_branch() {
            Self::update_elastic_document(data, like.branch_id, like.object_id).await;
        }

        Ok(())
    }

    async fn like_count(db_session: &CachingSession, branch_id: Uuid, id: Uuid) -> Result<i64, NodecosmosError> {
        let res = Self::find_by_primary_key_value(&(branch_id, id))
            .execute(db_session)
            .await
            .ok();

        match res {
            Some(c) => {
                let c = c.like_count.unwrap_or_else(|| Counter(0));

                Ok(c.0)
            }
            None => Ok(0),
        }
    }
}
