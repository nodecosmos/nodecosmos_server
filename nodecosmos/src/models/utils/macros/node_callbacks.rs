macro_rules! impl_node_updated_at_with_elastic_ext_cb {
    ($struct_name:ident) => {
        impl charybdis::callbacks::ExtCallbacks for $struct_name {
            type Extension = crate::api::data::RequestData;
            type Error = crate::errors::NodecosmosError;

            async fn before_update(
                &mut self,
                _session: &charybdis::CachingSession,
                _ext: &Self::Extension,
            ) -> Result<(), crate::errors::NodecosmosError> {
                self.updated_at = Some(chrono::Utc::now());

                Ok(())
            }

            async fn after_update(
                &mut self,
                _session: &charybdis::CachingSession,
                ext: &Self::Extension,
            ) -> Result<(), crate::errors::NodecosmosError> {
                use crate::services::elastic::index::ElasticIndex;

                crate::services::elastic::update_elastic_document(
                    &ext.app.elastic_client,
                    crate::models::node::Node::ELASTIC_IDX_NAME,
                    self,
                    self.id.to_string(),
                )
                .await;

                Ok(())
            }
        }
    };
}

pub(crate) use impl_node_updated_at_with_elastic_ext_cb;
