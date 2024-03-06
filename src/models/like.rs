mod callbacks;
mod create;
pub mod likeable;

use crate::errors::NodecosmosError;
use crate::models::like::likeable::Likeable;
use crate::models::node_counter::NodeCounter;
use charybdis::macros::charybdis_model;
use charybdis::types::{Text, Timestamp, Uuid};
use scylla::CachingSession;
use serde::{Deserialize, Serialize};
use std::fmt;

// CQL limitation is to have counters in a separate table
// https://docs.datastax.com/en/cql-oss/3.3/cql/cql_using/useCounters.html

#[charybdis_model(
    table_name = likes,
    partition_keys = [object_id],
    clustering_keys = [branch_id],
    global_secondary_indexes = []
)]
#[derive(Serialize, Deserialize, Default, Clone)]
pub struct Like {
    #[serde(rename = "objectId")]
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
    pub created_at: Option<Timestamp>,

    #[serde(default, rename = "updatedAt")]
    pub updated_at: Option<Timestamp>,

    #[serde(skip)]
    #[charybdis_model(ignore)]
    pub like_count: Option<i64>,
}

impl Like {
    pub async fn like_count(&mut self, session: &CachingSession) -> Result<i64, NodecosmosError> {
        if let Some(c) = self.like_count {
            return Ok(c);
        }

        match ObjectType::from_string(self.object_type.as_str()) {
            Some(ObjectType::Node) => {
                let lc = NodeCounter::like_count(session, self.object_id, self.branch_id).await?;

                self.like_count = Some(lc);

                Ok(lc)
            }
            _ => Err(NodecosmosError::InternalServerError("Object type not supported".to_string()).into()),
        }
    }
}

#[derive(Deserialize)]
pub enum ObjectType {
    Node,
    Like,
    Comment,
}

impl fmt::Display for ObjectType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ObjectType::Node => write!(f, "Node"),
            ObjectType::Like => write!(f, "Like"),
            ObjectType::Comment => write!(f, "Comment"),
        }
    }
}

impl ObjectType {
    pub fn from_string(s: &str) -> Option<Self> {
        match s {
            "Node" => Some(ObjectType::Node),
            "Like" => Some(ObjectType::Like),
            "Comment" => Some(ObjectType::Comment),
            _ => None,
        }
    }
}
