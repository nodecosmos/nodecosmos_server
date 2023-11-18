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

pub trait ExtCallbacks<Err: From<CharybdisError>> {
    type Extension;

    async fn before_insert(&mut self, _session: &CachingSession, _extension: &Self::Extension) -> Result<(), Err> {
        Ok(())
    }
    async fn after_insert(&mut self, _session: &CachingSession, _extension: &Self::Extension) -> Result<(), Err> {
        Ok(())
    }
    async fn before_update(&mut self, _session: &CachingSession, _extension: &Self::Extension) -> Result<(), Err> {
        Ok(())
    }
    async fn after_update(&mut self, _session: &CachingSession, _extension: &Self::Extension) -> Result<(), Err> {
        Ok(())
    }
    async fn before_delete(&mut self, _session: &CachingSession, _extension: &Self::Extension) -> Result<(), Err> {
        Ok(())
    }
    async fn after_delete(&mut self, _session: &CachingSession, _extension: &Self::Extension) -> Result<(), Err> {
        Ok(())
    }
}
