#[allow(unused_macros)]
macro_rules! impl_default_callbacks {
    ($struct_name:ident) => {
        impl charybdis::Callbacks<crate::errors::NodecosmosError> for $struct_name {
            crate::models::helpers::created_at_cb_fn!();

            crate::models::helpers::updated_at_cb_fn!();
        }
    };
}

#[allow(unused_imports)]
pub(crate) use impl_default_callbacks;

macro_rules! created_at_cb_fn {
    () => {
        async fn before_insert(
            &mut self,
            _session: &charybdis::CachingSession,
        ) -> Result<(), crate::errors::NodecosmosError> {
            let now = chrono::Utc::now();

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
        impl charybdis::Callbacks<crate::errors::NodecosmosError> for $struct_name {
            crate::models::helpers::updated_at_cb_fn!();
        }
    };
}
pub(crate) use impl_updated_at_cb;

macro_rules! updated_at_cb_fn {
    () => {
        async fn before_update(
            &mut self,
            _session: &charybdis::CachingSession,
        ) -> Result<(), crate::errors::NodecosmosError> {
            self.updated_at = Some(chrono::Utc::now());

            Ok(())
        }
    };
}
pub(crate) use updated_at_cb_fn;

macro_rules! sanitize_description_cb {
    ($struct_name:ident) => {
        impl charybdis::Callbacks<crate::errors::NodecosmosError> for $struct_name {
            crate::models::helpers::sanitize_description_cb_fn!();
        }
    };
}
pub(crate) use sanitize_description_cb;

macro_rules! sanitize_description_cb_fn {
    () => {
        async fn before_update(
            &mut self,
            _session: &charybdis::CachingSession,
        ) -> Result<(), crate::errors::NodecosmosError> {
            use ammonia::clean;

            self.updated_at = Some(chrono::Utc::now());

            if let Some(description) = &self.description {
                self.description = Some(clean(description));
            }

            Ok(())
        }
    };
}

pub(crate) use sanitize_description_cb_fn;
