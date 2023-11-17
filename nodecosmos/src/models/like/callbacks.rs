use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::like::Like;
use crate::models::likes_count::LikesCount;
use charybdis::callbacks::ExtCallbacks;
use scylla::CachingSession;

impl ExtCallbacks<NodecosmosError> for Like {
    type Extension = RequestData;

    async fn before_insert(&mut self, session: &CachingSession, _: &RequestData) -> Result<(), NodecosmosError> {
        self.validate_not_liked(session).await?;
        self.set_defaults();

        LikesCount::increment(session, self.object_id).await?;

        Ok(())
    }

    async fn after_insert(&mut self, _: &CachingSession, ext: &RequestData) -> Result<(), NodecosmosError> {
        let mut self_clone = self.clone();
        let app = ext.app.clone();
        let ext = ext.clone();

        tokio::spawn(async move {
            let session = &app.db_session;

            self_clone.update_model_likes_count(session, &ext).await.unwrap();
            self_clone.push_to_user_liked_obj_ids(session).await.unwrap();
        });

        Ok(())
    }

    async fn before_delete(&mut self, session: &CachingSession, _ext: &RequestData) -> Result<(), NodecosmosError> {
        LikesCount::decrement(session, self.object_id).await?;

        Ok(())
    }

    async fn after_delete(&mut self, _: &CachingSession, ext: &RequestData) -> Result<(), NodecosmosError> {
        let mut self_clone = self.clone();
        let app = ext.app.clone();
        let ext = ext.clone();

        tokio::spawn(async move {
            let session = &app.db_session;

            self_clone.update_model_likes_count(session, &ext).await.unwrap();
            self_clone.pull_from_user_liked_obj_ids(session).await.unwrap();
        });

        Ok(())
    }
}
