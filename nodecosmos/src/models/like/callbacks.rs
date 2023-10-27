use crate::callback_extension::CbExtension;
use crate::errors::NodecosmosError;
use crate::models::like::Like;
use crate::models::likes_count::LikesCount;
use charybdis::callbacks::ExtCallbacks;
use scylla::CachingSession;

impl ExtCallbacks<CbExtension, NodecosmosError> for Like {
    async fn before_insert(&mut self, session: &CachingSession, _ext: &CbExtension) -> Result<(), NodecosmosError> {
        self.validate_not_liked(session).await?;
        self.set_defaults();

        LikesCount::increment(session, self.object_id).await?;

        Ok(())
    }

    async fn after_insert(&self, session: &CachingSession, ext: &CbExtension) -> Result<(), NodecosmosError> {
        self.update_model_likes_count(session, ext).await?;
        self.push_to_user_liked_obj_ids(session).await?;

        Ok(())
    }

    async fn before_delete(&mut self, session: &CachingSession, _ext: &CbExtension) -> Result<(), NodecosmosError> {
        LikesCount::decrement(session, self.object_id).await?;

        Ok(())
    }

    async fn after_delete(&self, session: &CachingSession, ext: &CbExtension) -> Result<(), NodecosmosError> {
        self.update_model_likes_count(session, ext).await?;
        self.pull_from_user_liked_obj_ids(session).await?;

        Ok(())
    }
}
