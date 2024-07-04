use charybdis::macros::charybdis_model;
use charybdis::operations::Find;
use charybdis::types::{Counter, Uuid};
use nodecosmos_macros::Branchable;
use scylla::CachingSession;
use serde::{Deserialize, Serialize};

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::like::likeable::Likeable;
use crate::models::like::Like;
use crate::models::traits::{ElasticDocument, UpdateCounterNodeElasticIdx};

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
    pub contribution_requests_count: Option<Counter>,
    pub threads_count: Option<Counter>,
}

impl NodeCounter {
    pub async fn spawn_update_elastic_document(data: &RequestData, root_id: Uuid, branch_id: Uuid, id: Uuid) {
        if root_id != branch_id {
            return;
        }

        let data = data.clone();

        tokio::spawn(async move {
            Self::update_elastic_document(&data, root_id, branch_id, id).await;
        });
    }

    pub async fn update_elastic_document(data: &RequestData, root_id: Uuid, branch_id: Uuid, id: Uuid) {
        if root_id != branch_id {
            return;
        }

        let counter = Self::find_by_primary_key_value((branch_id, id))
            .execute(data.db_session())
            .await;

        match counter {
            Ok(c) => {
                UpdateCounterNodeElasticIdx {
                    id,
                    likes_count: c.like_count.unwrap_or_else(|| Counter(0)).0 as i32,
                    descendants_count: c.descendants_count.unwrap_or_else(|| Counter(0)).0 as i32,
                    contribution_requests_count: c.contribution_requests_count.unwrap_or_else(|| Counter(0)).0 as i32,
                    threads_count: c.threads_count.unwrap_or_else(|| Counter(0)).0 as i32,
                }
                .update_elastic_document(data.elastic_client())
                .await;
            }

            Err(e) => {
                log::error!("Error getting counter: {:?}", e);
            }
        };
    }

    pub async fn increment_cr_count(
        data: &RequestData,
        root_id: Uuid,
        branch_id: Uuid,
        id: Uuid,
    ) -> Result<(), NodecosmosError> {
        Self {
            branch_id,
            id,
            ..Default::default()
        }
        .increment_contribution_requests_count(1)
        .execute(data.db_session())
        .await?;

        Self::update_elastic_document(data, root_id, branch_id, id).await;

        Ok(())
    }

    pub async fn decrement_cr_count(
        data: &RequestData,
        root_id: Uuid,
        branch_id: Uuid,
        id: Uuid,
    ) -> Result<(), NodecosmosError> {
        Self {
            branch_id,
            id,
            ..Default::default()
        }
        .decrement_contribution_requests_count(1)
        .execute(data.db_session())
        .await?;

        Self::update_elastic_document(data, root_id, branch_id, id).await;

        Ok(())
    }

    pub async fn increment_thread_count(
        data: &RequestData,
        root_id: Uuid,
        branch_id: Uuid,
        id: Uuid,
    ) -> Result<(), NodecosmosError> {
        Self {
            branch_id,
            id,
            ..Default::default()
        }
        .increment_threads_count(1)
        .execute(data.db_session())
        .await?;

        Self::spawn_update_elastic_document(data, root_id, branch_id, id).await;

        Ok(())
    }

    pub async fn decrement_thread_count(
        data: &RequestData,
        root_id: Uuid,
        branch_id: Uuid,
        id: Uuid,
    ) -> Result<(), NodecosmosError> {
        Self {
            branch_id,
            id,
            ..Default::default()
        }
        .decrement_threads_count(1)
        .execute(data.db_session())
        .await?;

        Self::spawn_update_elastic_document(data, root_id, branch_id, id).await;

        Ok(())
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

        if let Some(root_id) = like.root_id {
            Self::spawn_update_elastic_document(data, root_id, like.branch_id, like.object_id).await;
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

        if let Some(root_id) = like.root_id {
            Self::spawn_update_elastic_document(data, root_id, like.branch_id, like.object_id).await;
        }

        Ok(())
    }

    async fn like_count(db_session: &CachingSession, branch_id: Uuid, id: Uuid) -> Result<i64, NodecosmosError> {
        let res = Self::find_by_primary_key_value((branch_id, id))
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
