use chrono::Utc;

use crate::app::CbExtension;
use crate::models::likes_count::LikesCount;
use crate::models::node::{find_update_node_likes_count_query, UpdateNodeLikesCount};
use crate::models::user::User;
use charybdis::*;

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

impl ObjectTypes {
    pub fn to_string(&self) -> String {
        match self {
            ObjectTypes::Node => "Node".to_string(),
        }
    }

    pub fn from_string(s: &str) -> Option<Self> {
        match s {
            "Node" => Some(ObjectTypes::Node),
            _ => None,
        }
    }
}

impl Like {
    pub async fn validate_not_liked(&self, session: &CachingSession) -> Result<(), CharybdisError> {
        let existing_like_query = find_like_query!("object_id = ? AND user_id = ?");
        let existing_like =
            Like::find_one(session, existing_like_query, (self.object_id, self.user_id))
                .await
                .ok();

        if existing_like.is_some() {
            return Err(CharybdisError::ValidationError((
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
    ) -> Result<LikesCount, CharybdisError> {
        let mut lc = LikesCount::new();
        lc.object_id = self.object_id;

        let lc = lc.find_by_primary_key(session).await?;

        Ok(lc)
    }

    pub async fn update_model_likes_count(
        &self,
        session: &CachingSession,
        ext: &CbExtension,
    ) -> Result<(), CharybdisError> {
        match ObjectTypes::from_string(self.object_type.as_str()) {
            Some(ObjectTypes::Node) => {
                let nfq = find_update_node_likes_count_query!("id = ?");
                let mut node =
                    UpdateNodeLikesCount::find_one(session, nfq, (self.object_id,)).await?;
                let lc = self.likes_count(session).await?;

                node.likes_count = Some(lc.count.0);
                node.update_cb(session, ext).await?;

                Ok(())
            }
            _ => Err(CharybdisError::CustomError(
                "Unknown ObjectType".to_string(),
            )),
        }
    }

    pub async fn push_to_user_liked_obj_ids(
        &self,
        session: &CachingSession,
    ) -> Result<(), CharybdisError> {
        let q = User::PUSH_TO_LIKED_OBJECT_IDS_QUERY;

        execute(session, q, (self.object_id, self.user_id)).await?;

        Ok(())
    }

    pub async fn pull_from_user_liked_obj_ids(
        &self,
        session: &CachingSession,
    ) -> Result<(), CharybdisError> {
        let q = User::PULL_FROM_LIKED_OBJECT_IDS_QUERY;

        execute(session, q, (self.object_id, self.user_id)).await?;

        Ok(())
    }
}

impl ExtCallbacks<CbExtension> for Like {
    async fn before_insert(
        &mut self,
        session: &CachingSession,
        _ext: &CbExtension,
    ) -> Result<(), CharybdisError> {
        self.validate_not_liked(session).await?;
        self.set_defaults();

        LikesCount::increment(session, self.object_id).await?;

        Ok(())
    }

    async fn after_insert(
        &mut self,
        session: &CachingSession,
        ext: &CbExtension,
    ) -> Result<(), CharybdisError> {
        self.update_model_likes_count(session, ext).await?;
        self.push_to_user_liked_obj_ids(session).await?;

        Ok(())
    }

    async fn before_delete(
        &mut self,
        session: &CachingSession,
        _ext: &CbExtension,
    ) -> Result<(), CharybdisError> {
        LikesCount::decrement(session, self.object_id).await?;

        Ok(())
    }

    async fn after_delete(
        &mut self,
        session: &CachingSession,
        ext: &CbExtension,
    ) -> Result<(), CharybdisError> {
        self.update_model_likes_count(session, ext).await?;
        self.pull_from_user_liked_obj_ids(session).await?;

        Ok(())
    }
}
