use crate::errors::NodecosmosError;
use charybdis::macros::charybdis_model;
use charybdis::operations::execute;
use charybdis::types::{Counter, Uuid};
use scylla::CachingSession;
use serde::{Deserialize, Serialize};

// CQL limitation is to have counters in a separate table
// https://docs.datastax.com/en/cql-oss/3.3/cql/cql_using/useCounters.html

#[charybdis_model(
    table_name = likes_count,
    partition_keys = [object_id],
    clustering_keys = [],
    secondary_indexes = []
)]
#[derive(Serialize, Deserialize, Default)]
pub struct LikesCount {
    pub object_id: Uuid,
    pub count: Counter,
}

impl LikesCount {
    pub async fn increment(
        session: &CachingSession,
        object_id: Uuid,
    ) -> Result<(), NodecosmosError> {
        let query = update_likes_count_query!("count = count + 1");

        execute(session, query, (object_id,)).await?;

        Ok(())
    }

    pub async fn decrement(
        session: &CachingSession,
        object_id: Uuid,
    ) -> Result<(), NodecosmosError> {
        let query = update_likes_count_query!("count = count - 1");

        execute(session, query, (object_id,)).await?;

        Ok(())
    }
}
