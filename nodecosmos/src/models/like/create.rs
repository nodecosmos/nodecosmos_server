use charybdis::operations::Find;
use chrono::Utc;
use scylla::CachingSession;

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::like::likeable::Likeable;
use crate::models::like::{Like, LikeObjectType};
use crate::models::materialized_views::likes_by_user::LikesByUser;
use crate::models::node_counter::NodeCounter;

impl Like {
    pub fn set_defaults(&mut self, data: &RequestData) {
        let now = Utc::now();

        self.user_id = data.current_user.id;
        self.username = data.current_user.username.clone();

        self.created_at = now;
        self.updated_at = now;
    }

    pub async fn validate_not_liked(&self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        let existing_like = LikesByUser {
            user_id: self.user_id,
            object_id: self.object_id,
            branch_id: self.branch_id,
        }
        .find_by_primary_key()
        .execute(db_session)
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

    pub async fn increment_like_count(&mut self, data: &RequestData) {
        match self.object_type.parse() {
            Ok(LikeObjectType::Node) => {
                let _ = NodeCounter::increment_like(data, self.object_id, self.branch_id)
                    .await
                    .map_err(|e| {
                        log::error!("Error incrementing like count: {:?}", e);
                    });
            }
            _ => log::error!("Like Object type not supported"),
        }
    }

    pub async fn decrement_like_count(&mut self, data: &RequestData) {
        match self.object_type.parse() {
            Ok(LikeObjectType::Node) => {
                let _ = NodeCounter::decrement_like(data, self.object_id, self.branch_id)
                    .await
                    .map_err(|e| {
                        log::error!("Error decrementing like count: {:?}", e);
                    });
            }
            _ => log::error!("Like Object type not supported"),
        }
    }
}
