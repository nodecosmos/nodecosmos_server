use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::like::likeable::Likeable;
use crate::models::node::UpdateLikesCountNode;
use charybdis::macros::charybdis_model;
use charybdis::operations::{Find, UpdateWithCallbacks};
use charybdis::types::{Counter, Uuid};
use scylla::CachingSession;
use serde::{Deserialize, Serialize};

#[charybdis_model(
    table_name = node_counters,
    partition_keys = [id],
    clustering_keys = [branch_id],
    global_secondary_indexes = []
)]
#[derive(Serialize, Deserialize, Default, Debug)]
pub struct NodeCounter {
    #[serde(rename = "branchId")]
    pub branch_id: Uuid,

    #[serde(rename = "id")]
    pub id: Uuid,

    #[serde(rename = "likeCount")]
    pub like_count: Option<Counter>,

    #[serde(rename = "descendantsCount")]
    pub descendants_count: Option<Counter>,
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

        let mut node = UpdateLikesCountNode {
            id,
            branch_id,
            like_count: Some(lc),
            updated_at: Some(chrono::Utc::now()),
        };

        node.update_cb(data).execute(data.db_session()).await?;

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

        let mut node = UpdateLikesCountNode {
            id,
            branch_id,
            like_count: Some(lc),
            updated_at: Some(chrono::Utc::now()),
        };

        node.update_cb(data).execute(data.db_session()).await?;

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
