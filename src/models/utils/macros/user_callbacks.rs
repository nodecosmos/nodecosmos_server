macro_rules! impl_user_updated_at_with_elastic_ext_cb {
    ($struct_name:ident) => {
        impl charybdis::callbacks::ExtCallbacks for $struct_name {
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
                use crate::services::elastic::{ElasticDocument, ElasticIndex};

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

use crate::models::node::UpdateOwnerNode;
pub(crate) use impl_user_updated_at_with_elastic_ext_cb;
