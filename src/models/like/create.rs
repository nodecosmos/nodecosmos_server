use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::like::likeable::Likeable;
use crate::models::like::{Like, ObjectType};
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

        self.created_at = Some(now);
        self.updated_at = Some(now);
    }

    pub async fn validate_not_liked(&self, session: &CachingSession) -> Result<(), NodecosmosError> {
        let existing_like = self.find_by_primary_key(session).await.ok();

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
        _: &CachingSession,
        data: &RequestData,
    ) -> Result<(), NodecosmosError> {
        match ObjectType::from_string(self.object_type.as_str()) {
            Some(ObjectType::Node) => {
                let lc = NodeCounter::increment_like(data, self.object_id, self.branch_id).await?;

                self.like_count = Some(lc);

                Ok(())
            }
            _ => Err(NodecosmosError::InternalServerError("Object type not supported".to_string()).into()),
        }
    }
}
