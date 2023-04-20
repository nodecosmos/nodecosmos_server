use crate::errors::CharybdisError;
use scylla::CachingSession;

pub trait Callbacks {
    async fn before_insert(&self, _session: &CachingSession) -> Result<(), CharybdisError> {
        Ok(())
    }
    async fn after_insert(&self, _session: &CachingSession) -> Result<(), CharybdisError> {
        Ok(())
    }
    async fn before_update(&self, _session: &CachingSession) -> Result<(), CharybdisError> {
        Ok(())
    }
    async fn after_update(&self, _session: &CachingSession) -> Result<(), CharybdisError> {
        Ok(())
    }
    async fn before_delete(&self, _session: &CachingSession) -> Result<(), CharybdisError> {
        Ok(())
    }
    async fn after_delete(&self, _session: &CachingSession) -> Result<(), CharybdisError> {
        Ok(())
    }
}
