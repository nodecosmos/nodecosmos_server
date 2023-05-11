macro_rules! set_updated_at_cb {
    ($struct_name:ident) => {
        impl charybdis::Callbacks for $struct_name {
            async fn before_update(
                &mut self,
                _session: &charybdis::CachingSession,
            ) -> Result<(), charybdis::CharybdisError> {
                self.updated_at = Some(Utc::now());
                Ok(())
            }
        }
    };
}
pub(crate) use set_updated_at_cb;

macro_rules! set_updated_at_cb_fn {
    () => {
        async fn before_update(
            &mut self,
            _session: &charybdis::CachingSession,
        ) -> Result<(), charybdis::CharybdisError> {
            self.updated_at = Some(Utc::now());

            Ok(())
        }
    };
}
pub(crate) use set_updated_at_cb_fn;
