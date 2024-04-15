use charybdis::callbacks::Callbacks;
use charybdis::macros::charybdis_model;
use charybdis::types::{Text, Timestamp, Uuid};
use scylla::CachingSession;
use serde::{Deserialize, Serialize};

use nodecosmos_macros::Branchable;

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::like::likeable::Likeable;
use crate::models::node_counter::NodeCounter;

mod create;
pub mod likeable;

// CQL limitation is to have counters in a separate table
// https://docs.datastax.com/en/cql-oss/3.3/cql/cql_using/useCounters.html

#[derive(Deserialize, strum_macros::Display, strum_macros::EnumString)]
pub enum LikeObjectType {
    Node,
    Comment,
}

#[charybdis_model(
    table_name = likes,
    partition_keys = [object_id],
    clustering_keys = [branch_id],
    global_secondary_indexes = []
)]
#[derive(Branchable, Serialize, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Like {
    #[branch(original_id)]
    pub object_id: Uuid,

    pub branch_id: Uuid,

    #[serde(default)]
    pub object_type: Text,

    #[serde(default)]
    pub user_id: Uuid,

    #[serde(default)]
    pub username: Text,

    #[serde(default = "chrono::Utc::now")]
    pub created_at: Timestamp,

    #[serde(default = "chrono::Utc::now")]
    pub updated_at: Timestamp,
}

impl Callbacks for Like {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_insert(&mut self, db_session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        self.set_defaults(data);
        self.validate_not_liked(db_session).await?;

        Ok(())
    }

    async fn after_insert(&mut self, _: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        let mut self_clone = self.clone();
        let data = data.clone();

        tokio::spawn(async move {
            self_clone.increment_like_count(&data).await;
        });

        Ok(())
    }

    async fn after_delete(&mut self, _: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        let mut self_clone = self.clone();
        let data = data.clone();

        tokio::spawn(async move {
            self_clone.decrement_like_count(&data).await;
        });

        Ok(())
    }
}

impl Like {
    pub async fn like_count(&mut self, db_session: &CachingSession) -> Result<i64, NodecosmosError> {
        match LikeObjectType::from(self.object_type.parse()?) {
            LikeObjectType::Node => {
                let lc = NodeCounter::like_count(db_session, self.object_id, self.branch_id).await?;

                Ok(lc)
            }
            _ => Err(NodecosmosError::InternalServerError("Object type not supported".to_string()).into()),
        }
    }
}
