use charybdis::operations::Find;
use chrono::Utc;
use scylla::client::caching_session::CachingSession;

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::like::Like;
use crate::models::materialized_views::likes_by_user::LikesByUser;

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
            return Err(NodecosmosError::ValidationError(("user", "already liked!")));
        }

        Ok(())
    }
}
