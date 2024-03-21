use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::like::Like;
use charybdis::callbacks::Callbacks;
use scylla::CachingSession;

impl Callbacks for Like {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_insert(&mut self, db_session: &CachingSession, _: &RequestData) -> Result<(), NodecosmosError> {
        self.validate_not_liked(db_session).await?;
        self.set_defaults();

        Ok(())
    }

    async fn after_insert(&mut self, _: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        let mut self_clone = self.clone();
        let data = data.clone();

        tokio::spawn(async move {
            self_clone.update_model_like_count(&data, true).await.unwrap();
        });

        Ok(())
    }

    async fn after_delete(&mut self, _: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        let mut self_clone = self.clone();
        let data = data.clone();

        tokio::spawn(async move {
            self_clone.update_model_like_count(&data, false).await.unwrap();
        });

        Ok(())
    }
}
