use crate::callback_extension::CbExtension;
use crate::errors::NodecosmosError;
use crate::models::helpers::impl_user_updated_at_with_elastic_ext_cb;
use crate::models::user::{UpdateUser, User};
use crate::services::elastic::{add_elastic_document, delete_elastic_document};
use charybdis::callbacks::ExtCallbacks;
use chrono::Utc;
use scylla::CachingSession;

impl ExtCallbacks<CbExtension, NodecosmosError> for User {
    async fn before_insert(&mut self, session: &CachingSession, _ext: &CbExtension) -> Result<(), NodecosmosError> {
        self.check_existing_user(session).await?;

        self.set_defaults();
        self.set_password()?;

        Ok(())
    }

    async fn after_insert(&self, _session: &CachingSession, cb_extension: &CbExtension) -> Result<(), NodecosmosError> {
        add_elastic_document(
            &cb_extension.elastic_client,
            User::ELASTIC_IDX_NAME,
            self,
            self.id.to_string(),
        )
        .await;

        Ok(())
    }

    async fn before_update(&mut self, _: &CachingSession, _ext: &CbExtension) -> Result<(), NodecosmosError> {
        self.updated_at = Some(Utc::now());
        Ok(())
    }

    async fn after_delete(&self, _: &CachingSession, cb_extension: &CbExtension) -> Result<(), NodecosmosError> {
        delete_elastic_document(
            &cb_extension.elastic_client,
            User::ELASTIC_IDX_NAME,
            self.id.to_string(),
        )
        .await;

        Ok(())
    }
}

impl_user_updated_at_with_elastic_ext_cb!(UpdateUser);
