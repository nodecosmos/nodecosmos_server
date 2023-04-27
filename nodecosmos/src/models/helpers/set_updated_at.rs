macro_rules! set_updated_at_cb {
    ($struct_name:ident) => {
        impl Callbacks for $struct_name {
            async fn before_update(
                &mut self,
                _session: &CachingSession,
            ) -> Result<(), CharybdisError> {
                self.updated_at = Some(Utc::now());
                Ok(())
            }
        }
    };
}

pub(crate) use set_updated_at_cb;
