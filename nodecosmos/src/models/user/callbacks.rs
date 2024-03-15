use crate::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::traits::ElasticDocument;
use crate::models::traits::SanitizeDescription;
use crate::models::user::{UpdateBioUser, UpdateProfileImageUser, UpdateUser, User};
use crate::App;
use charybdis::callbacks::Callbacks;
use chrono::Utc;
use scylla::CachingSession;
use std::sync::Arc;

impl Callbacks for User {
    type Extension = Arc<App>;
    type Error = NodecosmosError;

    async fn before_insert(&mut self, session: &CachingSession, _ext: &Arc<App>) -> Result<(), NodecosmosError> {
        self.check_existing_user(session).await?;

        self.set_defaults();
        self.set_password()?;

        Ok(())
    }

    async fn after_insert(&mut self, _session: &CachingSession, app: &Arc<App>) -> Result<(), NodecosmosError> {
        self.add_elastic_document(&app.elastic_client).await;

        Ok(())
    }

    async fn before_update(&mut self, _: &CachingSession, _ext: &Arc<App>) -> Result<(), NodecosmosError> {
        self.updated_at = Some(Utc::now());
        Ok(())
    }

    async fn after_delete(&mut self, _: &CachingSession, app: &Arc<App>) -> Result<(), NodecosmosError> {
        self.update_elastic_document(&app.elastic_client).await;

        Ok(())
    }
}

macro_rules! impl_user_updated_at_with_elastic_ext_cb {
    ($struct_name:ident) => {
        impl charybdis::callbacks::Callbacks for $struct_name {
            type Extension = crate::api::data::RequestData;
            type Error = crate::errors::NodecosmosError;

            async fn before_update(
                &mut self,
                _session: &charybdis::CachingSession,
                _ext: &Self::Extension,
            ) -> Result<(), crate::errors::NodecosmosError> {
                self.updated_at = Some(Utc::now());

                Ok(())
            }

            async fn after_update(
                &mut self,
                _session: &charybdis::CachingSession,
                req_data: &Self::Extension,
            ) -> Result<(), crate::errors::NodecosmosError> {
                use crate::models::node::UpdateOwnerNode;
                use crate::models::traits::ElasticDocument;

                self.update_elastic_document(req_data.elastic_client()).await;

                let user_id = self.id.clone();
                let req_data = req_data.clone();

                tokio::spawn(async move {
                    UpdateOwnerNode::update_owner_records(&req_data, user_id).await;
                });

                Ok(())
            }
        }
    };
}

impl_user_updated_at_with_elastic_ext_cb!(UpdateUser);
impl_user_updated_at_with_elastic_ext_cb!(UpdateProfileImageUser);

impl Callbacks for UpdateBioUser {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_update(&mut self, _: &CachingSession, _ext: &RequestData) -> Result<(), NodecosmosError> {
        self.updated_at = Some(Utc::now());
        self.bio.sanitize()?;

        Ok(())
    }

    async fn after_update(&mut self, _: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        self.update_elastic_document(data.elastic_client()).await;

        Ok(())
    }
}
