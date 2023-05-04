macro_rules! impl_default_callbacks {
    ($struct_name:ident) => {
        impl Callbacks for $struct_name {
            async fn before_insert(
                &mut self,
                _session: &CachingSession,
            ) -> Result<(), CharybdisError> {
                let now = Utc::now();

                self.id = Uuid::new_v4();
                self.created_at = Some(now);
                self.updated_at = Some(now);

                Ok(())
            }

            async fn before_update(
                &mut self,
                _session: &CachingSession,
            ) -> Result<(), CharybdisError> {
                let now = Utc::now();

                self.updated_at = Some(now);

                Ok(())
            }
        }
    };
}

pub(crate) use impl_default_callbacks;
