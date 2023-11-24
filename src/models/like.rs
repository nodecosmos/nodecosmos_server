mod callbacks;

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::likes_count::LikesCount;
use crate::models::node::UpdateLikesCountNode;
use crate::models::user::User;
use charybdis::macros::charybdis_model;
use charybdis::operations::{execute, Find, UpdateWithExtCallbacks};
use charybdis::types::{Text, Timestamp, Uuid};
use chrono::Utc;
use scylla::CachingSession;
use serde::{Deserialize, Serialize};
use std::fmt;

// CQL limitation is to have counters in a separate table
// https://docs.datastax.com/en/cql-oss/3.3/cql/cql_using/useCounters.html

#[charybdis_model(
    table_name = likes,
    partition_keys = [object_id],
    clustering_keys = [user_id],
    global_secondary_indexes = []
)]
#[derive(Serialize, Deserialize, Default, Clone)]
pub struct Like {
    pub object_id: Uuid,
    pub object_type: Text,
    pub user_id: Uuid,
    pub username: Text,
    pub created_at: Option<Timestamp>,
    pub updated_at: Option<Timestamp>,

    #[serde(skip)]
    #[charybdis(ignore)]
    pub likes_count: Option<LikesCount>,
}

#[derive(Deserialize)]
pub enum ObjectTypes {
    Node,
}

impl fmt::Display for ObjectTypes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ObjectTypes::Node => write!(f, "Node"),
        }
    }
}

impl ObjectTypes {
    pub fn from_string(s: &str) -> Option<Self> {
        match s {
            "Node" => Some(ObjectTypes::Node),
            _ => None,
        }
    }
}

impl Like {
    pub async fn validate_not_liked(&self, session: &CachingSession) -> Result<(), NodecosmosError> {
        let existing_like = Like::find_by_primary_key_value(session, (self.object_id, self.user_id))
            .await
            .ok();

        if existing_like.is_some() {
            return Err(NodecosmosError::ValidationError((
                "user".to_string(),
                "already liked!".to_string(),
            )));
        }

        Ok(())
    }

    fn set_defaults(&mut self) {
        let now = Utc::now();

        self.created_at = Some(now);
        self.updated_at = Some(now);
    }

    pub async fn likes_count(&mut self, session: &CachingSession) -> Result<&LikesCount, NodecosmosError> {
        if let None = &self.likes_count {
            let lc = LikesCount {
                object_id: self.object_id,
                ..Default::default()
            }
            .find_by_primary_key(session)
            .await?;

            self.likes_count = Some(lc);
        }

        match &self.likes_count {
            Some(lc) => Ok(lc),
            None => Err(NodecosmosError::InternalServerError("Likes count not found".to_string()).into()),
        }
    }

    pub async fn update_model_likes_count(
        &mut self,
        session: &CachingSession,
        data: &RequestData,
    ) -> Result<(), NodecosmosError> {
        match ObjectTypes::from_string(self.object_type.as_str()) {
            Some(ObjectTypes::Node) => {
                let mut node = UpdateLikesCountNode {
                    id: self.object_id,
                    ..Default::default()
                }
                .find_by_primary_key(session)
                .await?;
                let lc = self.likes_count(session).await?;

                node.likes_count = Some(lc.count.0);
                node.update_cb(session, data).await?;

                Ok(())
            }
            _ => Err(NodecosmosError::InternalServerError("Object type not supported".to_string()).into()),
        }
    }

    pub async fn push_to_user_liked_obj_ids(&self, session: &CachingSession) -> Result<(), NodecosmosError> {
        let q = User::PUSH_LIKED_OBJECT_IDS_QUERY;

        execute(session, q, (vec![self.object_id], self.user_id)).await?;

        Ok(())
    }

    pub async fn pull_from_user_liked_obj_ids(&self, session: &CachingSession) -> Result<(), NodecosmosError> {
        let q = User::PULL_LIKED_OBJECT_IDS_QUERY;

        execute(session, q, (vec![self.object_id], self.user_id)).await?;

        Ok(())
    }
}
