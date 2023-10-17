use crate::errors::CharybdisError;
use scylla::CachingSession;

pub trait Callbacks<Err: From<CharybdisError>> {
    async fn before_insert(&mut self, _session: &CachingSession) -> Result<(), Err> {
        Ok(())
    }
    async fn after_insert(&mut self, _session: &CachingSession) -> Result<(), Err> {
        Ok(())
    }
    async fn before_update(&mut self, _session: &CachingSession) -> Result<(), Err> {
        Ok(())
    }
    async fn after_update(&mut self, _session: &CachingSession) -> Result<(), Err> {
        Ok(())
    }
    async fn before_delete(&mut self, _session: &CachingSession) -> Result<(), Err> {
        Ok(())
    }
    async fn after_delete(&mut self, _session: &CachingSession) -> Result<(), Err> {
        Ok(())
    }
}

/// Extended callbacks for use with extensions, where we need to pass application-specific
/// extensions to the callbacks.
///
/// Example usage:
/// Let's say we want to pass an Elasticsearch client to the callbacks:
///
/// ```rust
///
/// pub struct CbExtension {
///   pub elastic_client: Elasticsearch,
/// }
///
/// // modes/user.rs
/// use charybdis::ExtCallbacks;
///
/// #[partial_model_generator]
/// #[charybdis_model(
///     table_name = users,
///     partition_keys = [id],
///     clustering_keys = [],
///     secondary_indexes = [username, email]
/// )]
/// pub struct User {
///    ...
/// }
///
/// impl ExtCallbacks<CbExtension, CustomError> for User {
///    async fn after_update(
///        &mut self,
///        session: &CachingSession,
///        extension: &CbExtension
///    ) -> Result<(), CustomError> {
///        // do something with extension.elastic_client
///        extension.elastic_client.update(...).await?;
///        Ok(())
///    }
/// }
/// ```
///
pub trait ExtCallbacks<Ext, Err: From<CharybdisError>> {
    async fn before_insert(&mut self, _session: &CachingSession, _extension: &Ext) -> Result<(), Err> {
        Ok(())
    }
    async fn after_insert(&mut self, _session: &CachingSession, _extension: &Ext) -> Result<(), Err> {
        Ok(())
    }
    async fn before_update(&mut self, _session: &CachingSession, _extension: &Ext) -> Result<(), Err> {
        Ok(())
    }
    async fn after_update(&mut self, _session: &CachingSession, _extension: &Ext) -> Result<(), Err> {
        Ok(())
    }
    async fn before_delete(&mut self, _session: &CachingSession, _extension: &Ext) -> Result<(), Err> {
        Ok(())
    }
    async fn after_delete(&mut self, _session: &CachingSession, _extension: &Ext) -> Result<(), Err> {
        Ok(())
    }
}
