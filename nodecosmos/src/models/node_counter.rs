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
use crate::models::traits::{ElasticDocument, UpdateLikeCountNodeElasticIdx};

#[charybdis_model(
    table_name = node_counters,
    partition_keys = [branch_id],
    clustering_keys = [id],
    global_secondary_indexes = []
)]
#[derive(Serialize, Deserialize, Branchable, Default, Debug)]
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
}

impl Likeable for NodeCounter {
    async fn increment_like(data: &RequestData, id: Uuid, branch_id: Uuid) -> Result<i64, NodecosmosError> {
        let lc = Self::like_count(data.db_session(), id, branch_id).await? + 1;
        Self {
            id,
            branch_id,
            ..Default::default()
        }
        .increment_like_count(1)
        .execute(data.db_session())
        .await?;

        let is_original = id == branch_id;

        if is_original {
            UpdateLikeCountNodeElasticIdx {
                id,
                likes_count: lc as i32,
            }
            .update_elastic_document(data.elastic_client())
            .await;
        }

        Ok(lc)
    }

    async fn decrement_like(data: &RequestData, id: Uuid, branch_id: Uuid) -> Result<i64, NodecosmosError> {
        let lc = Self::like_count(data.db_session(), id, branch_id).await? - 1;
        Self {
            id,
            branch_id,
            ..Default::default()
        }
        .decrement_like_count(1)
        .execute(data.db_session())
        .await?;

        let is_original = id == branch_id;

        if is_original {
            UpdateLikeCountNodeElasticIdx {
                id,
                likes_count: lc as i32,
            }
            .update_elastic_document(data.elastic_client())
            .await;
        }

        Ok(lc)
    }

    async fn like_count(db_session: &CachingSession, id: Uuid, branch_id: Uuid) -> Result<i64, NodecosmosError> {
        let res = Self::find_by_primary_key_value(&(id, branch_id))
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
