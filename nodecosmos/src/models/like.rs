use crate::errors::NodecosmosError;
use crate::models::likes_count::LikesCount;
use crate::models::node::{find_update_likes_count_node_query, UpdateLikesCountNode};
use crate::models::user::User;
use crate::CbExtension;
use charybdis::{execute, ExtCallbacks, Find, New, Text, Timestamp, UpdateWithExtCallbacks, Uuid};
use charybdis_macros::{charybdis_model, partial_model_generator};
use chrono::Utc;
use scylla::CachingSession;
use serde::Deserialize;
use std::fmt;

// CQL limitation is to have counters in a separate table
// https://docs.datastax.com/en/cql-oss/3.3/cql/cql_using/useCounters.html
#[partial_model_generator]
#[charybdis_model(
    table_name = likes,
    partition_keys = [object_id],
    clustering_keys = [user_id],
    secondary_indexes = []
)]
pub struct Like {
    pub object_id: Uuid,
    pub object_type: Text,
    pub user_id: Uuid,
    pub username: Text,
    pub created_at: Option<Timestamp>,
    pub updated_at: Option<Timestamp>,
}

#[derive(Debug, Deserialize)]
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
    pub async fn validate_not_liked(
        &self,
        session: &CachingSession,
    ) -> Result<(), NodecosmosError> {
        let existing_like_query = find_like_query!("object_id = ? AND user_id = ?");
        let existing_like =
            Like::find_one(session, existing_like_query, (self.object_id, self.user_id))
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

    pub async fn likes_count(
        &self,
        session: &CachingSession,
    ) -> Result<LikesCount, NodecosmosError> {
        let mut lc = LikesCount::new();
        lc.object_id = self.object_id;

        let lc = lc.find_by_primary_key(session).await?;

        Ok(lc)
    }

    pub async fn update_model_likes_count(
        &self,
        session: &CachingSession,
        ext: &CbExtension,
    ) -> Result<(), NodecosmosError> {
        match ObjectTypes::from_string(self.object_type.as_str()) {
            Some(ObjectTypes::Node) => {
                let nfq = find_update_likes_count_node_query!("id = ?");
                let mut node =
                    UpdateLikesCountNode::find_one(session, nfq, (self.object_id,)).await?;
                let lc = self.likes_count(session).await?;

                node.likes_count = Some(lc.count.0);
                node.update_cb(session, ext).await?;

                Ok(())
            }
            _ => Err(
                NodecosmosError::InternalServerError("Object type not supported".to_string())
                    .into(),
            ),
        }
    }

    pub async fn push_to_user_liked_obj_ids(
        &self,
        session: &CachingSession,
    ) -> Result<(), NodecosmosError> {
        let q = User::PUSH_TO_LIKED_OBJECT_IDS_QUERY;

        execute(session, q, (vec![self.object_id], self.user_id)).await?;

        Ok(())
    }

    pub async fn pull_from_user_liked_obj_ids(
        &self,
        session: &CachingSession,
    ) -> Result<(), NodecosmosError> {
        let q = User::PULL_FROM_LIKED_OBJECT_IDS_QUERY;

        execute(session, q, (vec![self.object_id], self.user_id)).await?;

        Ok(())
    }
}

impl ExtCallbacks<CbExtension, NodecosmosError> for Like {
    async fn before_insert(
        &mut self,
        session: &CachingSession,
        _ext: &CbExtension,
    ) -> Result<(), NodecosmosError> {
        self.validate_not_liked(session).await?;
        self.set_defaults();

        LikesCount::increment(session, self.object_id).await?;

        Ok(())
    }

    async fn after_insert(
        &mut self,
        session: &CachingSession,
        ext: &CbExtension,
    ) -> Result<(), NodecosmosError> {
        self.update_model_likes_count(session, ext).await?;
        self.push_to_user_liked_obj_ids(session).await?;

        Ok(())
    }

    async fn before_delete(
        &mut self,
        session: &CachingSession,
        _ext: &CbExtension,
    ) -> Result<(), NodecosmosError> {
        LikesCount::decrement(session, self.object_id).await?;

        Ok(())
    }

    async fn after_delete(
        &mut self,
        session: &CachingSession,
        ext: &CbExtension,
    ) -> Result<(), NodecosmosError> {
        self.update_model_likes_count(session, ext).await?;
        self.pull_from_user_liked_obj_ids(session).await?;

        Ok(())
    }
}
