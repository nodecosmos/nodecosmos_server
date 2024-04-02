use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::like::likeable::Likeable;
use crate::models::like::{Like, ObjectType};
use crate::models::materialized_views::likes_by_user::LikesByUser;
use crate::models::node_counter::NodeCounter;
use charybdis::operations::Find;
use charybdis::types::Uuid;
use chrono::Utc;
use scylla::CachingSession;

impl Like {
    pub fn set_defaults(&mut self) {
        let now = Utc::now();

        if self.branch_id == Uuid::default() {
            self.branch_id = self.object_id;
        }

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

    pub async fn update_model_like_count(
        &mut self,
        data: &RequestData,
        increment: bool,
    ) -> Result<(), NodecosmosError> {
        match ObjectType::from(self.object_type.parse()?) {
            ObjectType::Node => {
                if increment {
                    NodeCounter::increment_like(data, self.object_id, self.branch_id).await?;
                } else {
                    NodeCounter::decrement_like(data, self.object_id, self.branch_id).await?;
                }

                Ok(())
            }
            _ => Err(NodecosmosError::InternalServerError("Object type not supported".to_string()).into()),
        }
    }
}
