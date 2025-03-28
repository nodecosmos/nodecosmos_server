#[allow(unused_macros)]
macro_rules! impl_default_callbacks {
    ($struct_name:ident) => {
        impl charybdis::callbacks::Callbacks for $struct_name {
            type Error = NodecosmosError;
            type Extension = Option<()>;

            crate::models::utils::created_at_cb_fn!();

            crate::models::utils::updated_at_cb_fn!();
        }
    };
}

pub(crate) use impl_default_callbacks;

macro_rules! created_at_cb_fn {
    () => {
        async fn before_insert(
            &mut self,
            _session: &scylla::client::caching_session::CachingSession,
            _ext: &Self::Extension,
        ) -> Result<(), NodecosmosError> {
            let now = chrono::Utc::now();

            self.id = charybdis::types::Uuid::new_v4();
            self.created_at = now;
            self.updated_at = now;

            Ok(())
        }
    };
}
pub(crate) use created_at_cb_fn;

macro_rules! impl_updated_at_cb {
    ($struct_name:ident) => {
        impl charybdis::callbacks::Callbacks for $struct_name {
            type Error = NodecosmosError;
            type Extension = Option<()>;

            crate::models::utils::updated_at_cb_fn!();
        }
    };
}
pub(crate) use impl_updated_at_cb;

macro_rules! updated_at_cb_fn {
    () => {
        async fn before_update(
            &mut self,
            _session: &scylla::client::caching_session::CachingSession,
            _ext: &Self::Extension,
        ) -> Result<(), NodecosmosError> {
            self.updated_at = chrono::Utc::now();

            Ok(())
        }
    };
}
pub(crate) use updated_at_cb_fn;

macro_rules! sanitize_description_cb {
    ($struct_name:ident) => {
        impl charybdis::callbacks::Callbacks for $struct_name {
            type Error = NodecosmosError;
            type Extension = Option<()>;

            crate::models::utils::sanitize_description_cb_fn!();
        }
    };
}
pub(crate) use sanitize_description_cb;

macro_rules! sanitize_description_cb_fn {
    () => {
        async fn before_update(
            &mut self,
            _session: &scylla::client::caching_session::CachingSession,
            _ext: &Self::Extension,
        ) -> Result<(), NodecosmosError> {
            use crate::models::traits::Clean;

            self.updated_at = chrono::Utc::now();

            self.description.clean()?;

            Ok(())
        }
    };
}

pub(crate) use sanitize_description_cb_fn;
