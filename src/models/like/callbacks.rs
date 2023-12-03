use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::like::Like;
use charybdis::callbacks::ExtCallbacks;
use scylla::CachingSession;

impl ExtCallbacks for Like {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_insert(&mut self, session: &CachingSession, _: &RequestData) -> Result<(), NodecosmosError> {
        self.validate_not_liked(session).await?;
        self.set_defaults();

        Ok(())
    }

    async fn after_insert(&mut self, _: &CachingSession, req_data: &RequestData) -> Result<(), NodecosmosError> {
        let mut self_clone = self.clone();
        let app = req_data.app.clone();
        let req_data = req_data.clone();

        tokio::spawn(async move {
            let session = &app.db_session;

            self_clone.update_model_like_count(session, &req_data).await.unwrap();
        });

        Ok(())
    }

    async fn after_delete(&mut self, _: &CachingSession, req_data: &RequestData) -> Result<(), NodecosmosError> {
        let mut self_clone = self.clone();
        let app = req_data.app.clone();
        let req_data = req_data.clone();

        tokio::spawn(async move {
            let session = &app.db_session;

            self_clone.update_model_like_count(session, &req_data).await.unwrap();
        });

        Ok(())
    }
}
