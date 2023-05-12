macro_rules! impl_default_callbacks {
    ($struct_name:ident) => {
        impl charybdis::Callbacks for $struct_name {
            async fn before_insert(
                &mut self,
                _session: &charybdis::CachingSession,
            ) -> Result<(), charybdis::CharybdisError> {
                let now = Utc::now();

                self.id = Uuid::new_v4();
                self.created_at = Some(now);
                self.updated_at = Some(now);

                Ok(())
            }

            async fn before_update(
                &mut self,
                _session: &charybdis::CachingSession,
            ) -> Result<(), charybdis::CharybdisError> {
                let now = Utc::now();

                self.updated_at = Some(now);

                Ok(())
            }
        }
    };
}
pub(crate) use impl_default_callbacks;

macro_rules! created_at_cb_fn {
    () => {
        async fn before_insert(
            &mut self,
            _session: &charybdis::CachingSession,
        ) -> Result<(), charybdis::CharybdisError> {
            let now = Utc::now();

            self.id = Uuid::new_v4();
            self.created_at = Some(now);
            self.updated_at = Some(now);

            Ok(())
        }
    };
}
pub(crate) use created_at_cb_fn;

macro_rules! impl_updated_at_cb {
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
pub(crate) use impl_updated_at_cb;

macro_rules! updated_at_cb_fn {
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
pub(crate) use updated_at_cb_fn;

macro_rules! impl_empty_cb {
    ($struct_name:ident) => {
        impl charybdis::Callbacks for $struct_name {}
    };
}
pub(crate) use impl_empty_cb;
