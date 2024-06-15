use charybdis::batch::CharybdisBatch;
use charybdis::macros::charybdis_model;
use charybdis::operations::Find;
use charybdis::types::{Counter, Set, Uuid};
use nodecosmos_macros::Branchable;
use scylla::CachingSession;
use serde::{Deserialize, Serialize};

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::like::likeable::Likeable;
use crate::models::like::Like;
use crate::models::traits::{Branchable, ElasticDocument, UpdateCounterNodeElasticIdx};

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
    pub async fn spawn_update_elastic_document(data: &RequestData, branch_id: Uuid, id: Uuid) {
        let data = data.clone();

        tokio::spawn(async move {
            Self::update_elastic_document(&data, branch_id, id).await;
        });
    }

    pub async fn update_elastic_document(data: &RequestData, branch_id: Uuid, id: Uuid) {
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

    #[allow(dead_code)]
    async fn increment_descendants(
        data: &RequestData,
        ancestor_ids: Set<Uuid>,
        branch_id: Uuid,
    ) -> Result<(), NodecosmosError> {
        let mut counters = Vec::new();
        let mut batch = CharybdisBatch::new();

        for ancestor_id in ancestor_ids.iter() {
            let counter = Self {
                branch_id,
                id: *ancestor_id,
                ..Default::default()
            };
            counters.push(counter);
        }

        for counter in counters.iter() {
            let q = counter.increment_descendants_count(1);
            batch.append(q);
        }

        batch.execute(data.db_session()).await?;

        let mut futures = vec![];
        for counter in counters.iter() {
            let future = Self::update_elastic_document(data, branch_id, counter.id);

            futures.push(future);
        }

        futures::future::join_all(futures).await;

        Ok(())
    }

    pub async fn increment_cr_count(data: &RequestData, branch_id: Uuid, id: Uuid) -> Result<(), NodecosmosError> {
        Self {
            branch_id,
            id,
            ..Default::default()
        }
        .increment_contribution_requests_count(1)
        .execute(data.db_session())
        .await?;

        Self::update_elastic_document(data, branch_id, id).await;

        Ok(())
    }

    pub async fn decrement_cr_count(data: &RequestData, branch_id: Uuid, id: Uuid) -> Result<(), NodecosmosError> {
        Self {
            branch_id,
            id,
            ..Default::default()
        }
        .decrement_contribution_requests_count(1)
        .execute(data.db_session())
        .await?;

        Self::update_elastic_document(data, branch_id, id).await;

        Ok(())
    }

    pub async fn increment_thread_count(data: &RequestData, branch_id: Uuid, id: Uuid) -> Result<(), NodecosmosError> {
        Self {
            branch_id,
            id,
            ..Default::default()
        }
        .increment_threads_count(1)
        .execute(data.db_session())
        .await?;

        Self::spawn_update_elastic_document(data, branch_id, id).await;

        Ok(())
    }

    pub async fn decrement_thread_count(data: &RequestData, branch_id: Uuid, id: Uuid) -> Result<(), NodecosmosError> {
        Self {
            branch_id,
            id,
            ..Default::default()
        }
        .decrement_threads_count(1)
        .execute(data.db_session())
        .await?;

        Self::spawn_update_elastic_document(data, branch_id, id).await;

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

        if like.is_original() {
            Self::spawn_update_elastic_document(data, like.branch_id, like.object_id).await;
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

        if like.is_original() {
            Self::spawn_update_elastic_document(data, like.branch_id, like.object_id).await;
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
