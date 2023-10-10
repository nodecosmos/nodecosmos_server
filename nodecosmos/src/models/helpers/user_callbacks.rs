macro_rules! impl_user_updated_at_with_elastic_ext_cb {
    ($struct_name:ident) => {
        impl charybdis::ExtCallbacks<crate::CbExtension, crate::errors::NodecosmosError>
            for $struct_name
        {
            async fn before_update(
                &mut self,
                _session: &charybdis::CachingSession,
                _ext: &crate::CbExtension,
            ) -> Result<(), crate::errors::NodecosmosError> {
                self.updated_at = Some(Utc::now());

                Ok(())
            }

            async fn after_update(
                &mut self,
                _session: &charybdis::CachingSession,
                ext: &crate::CbExtension,
            ) -> Result<(), crate::errors::NodecosmosError> {
                crate::services::elastic::update_elastic_document(
                    &ext.elastic_client,
                    crate::models::user::User::ELASTIC_IDX_NAME,
                    self,
                    self.id.to_string(),
                )
                .await;

                Ok(())
            }
        }
    };
}

pub(crate) use impl_user_updated_at_with_elastic_ext_cb;
