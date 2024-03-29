mod create;
pub mod likeable;

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::like::likeable::Likeable;
use crate::models::node_counter::NodeCounter;
use charybdis::callbacks::Callbacks;
use charybdis::macros::charybdis_model;
use charybdis::types::{Text, Timestamp, Uuid};
use nodecosmos_macros::Branchable;
use scylla::CachingSession;
use serde::{Deserialize, Serialize};

// CQL limitation is to have counters in a separate table
// https://docs.datastax.com/en/cql-oss/3.3/cql/cql_using/useCounters.html

#[derive(Deserialize, strum_macros::Display, strum_macros::EnumString)]
pub enum ObjectType {
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
pub struct Like {
    #[serde(rename = "objectId")]
    #[branch(original_id)]
    pub object_id: Uuid,

    #[serde(default, rename = "branchId")]
    pub branch_id: Uuid,

    #[serde(default, rename = "objectType")]
    pub object_type: Text,

    #[serde(default, rename = "userId")]
    pub user_id: Uuid,

    #[serde(default, rename = "username")]
    pub username: Text,

    #[serde(default, rename = "createdAt")]
    pub created_at: Timestamp,

    #[serde(default, rename = "updatedAt")]
    pub updated_at: Timestamp,

    #[serde(skip)]
    #[charybdis(ignore)]
    pub like_count: Option<i64>,
}

impl Callbacks for Like {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_insert(&mut self, db_session: &CachingSession, _: &RequestData) -> Result<(), NodecosmosError> {
        self.validate_not_liked(db_session).await?;
        self.set_defaults();

        Ok(())
    }

    async fn after_insert(&mut self, _: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        let mut self_clone = self.clone();
        let data = data.clone();

        tokio::spawn(async move {
            self_clone.update_model_like_count(&data, true).await.unwrap();
        });

        Ok(())
    }

    async fn after_delete(&mut self, _: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        let mut self_clone = self.clone();
        let data = data.clone();

        tokio::spawn(async move {
            self_clone.update_model_like_count(&data, false).await.unwrap();
        });

        Ok(())
    }
}

impl Like {
    pub async fn like_count(&mut self, db_session: &CachingSession) -> Result<i64, NodecosmosError> {
        if let Some(c) = self.like_count {
            return Ok(c);
        }

        match ObjectType::from(self.object_type.parse()?) {
            ObjectType::Node => {
                let lc = NodeCounter::like_count(db_session, self.object_id, self.branch_id).await?;

                self.like_count = Some(lc);

                Ok(lc)
            }
            _ => Err(NodecosmosError::InternalServerError("Object type not supported".to_string()).into()),
        }
    }
}
