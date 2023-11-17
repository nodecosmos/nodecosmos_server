use crate::errors::NodecosmosError;
use crate::models::user::{UpdateUser, User};
use crate::models::utils::impl_user_updated_at_with_elastic_ext_cb;
use crate::services::elastic::index::ElasticIndex;
use crate::services::elastic::{add_elastic_document, delete_elastic_document};
use crate::App;
use charybdis::callbacks::ExtCallbacks;
use chrono::Utc;
use scylla::CachingSession;
use std::sync::Arc;

impl ExtCallbacks<NodecosmosError> for User {
    type Extension = Arc<App>;

    async fn before_insert(&mut self, session: &CachingSession, _ext: &Arc<App>) -> Result<(), NodecosmosError> {
        self.check_existing_user(session).await?;

        self.set_defaults();
        self.set_password()?;

        Ok(())
    }

    async fn after_insert(&mut self, _session: &CachingSession, app: &Arc<App>) -> Result<(), NodecosmosError> {
        add_elastic_document(&app.elastic_client, User::ELASTIC_IDX_NAME, self, self.id.to_string()).await;

        Ok(())
    }

    async fn before_update(&mut self, _: &CachingSession, _ext: &Arc<App>) -> Result<(), NodecosmosError> {
        self.updated_at = Some(Utc::now());
        Ok(())
    }

    async fn after_delete(&mut self, _: &CachingSession, app: &Arc<App>) -> Result<(), NodecosmosError> {
        delete_elastic_document(&app.elastic_client, User::ELASTIC_IDX_NAME, self.id.to_string()).await;

        Ok(())
    }
}

impl_user_updated_at_with_elastic_ext_cb!(UpdateUser);
