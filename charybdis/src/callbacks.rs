use crate::errors::CharybdisError;
use scylla::CachingSession;
use std::future::Future;

pub trait Callbacks<Err: From<CharybdisError>> {
    fn before_insert(&mut self, _session: &CachingSession) -> impl Future<Output = Result<(), Err>> {
        async move { Ok(()) }
    }
    fn after_insert(&mut self, _session: &CachingSession) -> impl Future<Output = Result<(), Err>> {
        async move { Ok(()) }
    }
    fn before_update(&mut self, _session: &CachingSession) -> impl Future<Output = Result<(), Err>> {
        async move { Ok(()) }
    }
    fn after_update(&mut self, _session: &CachingSession) -> impl Future<Output = Result<(), Err>> {
        async move { Ok(()) }
    }
    fn before_delete(&mut self, _session: &CachingSession) -> impl Future<Output = Result<(), Err>> {
        async move { Ok(()) }
    }
    fn after_delete(&mut self, _session: &CachingSession) -> impl Future<Output = Result<(), Err>> {
        async move { Ok(()) }
    }
}

pub trait ExtCallbacks<Err: From<CharybdisError>> {
    type Extension;

    fn before_insert(
        &mut self,
        _session: &CachingSession,
        _extension: &Self::Extension,
    ) -> impl Future<Output = Result<(), Err>> {
        async move { Ok(()) }
    }
    fn after_insert(
        &mut self,
        _session: &CachingSession,
        _extension: &Self::Extension,
    ) -> impl Future<Output = Result<(), Err>> {
        async move { Ok(()) }
    }
    fn before_update(
        &mut self,
        _session: &CachingSession,
        _extension: &Self::Extension,
    ) -> impl Future<Output = Result<(), Err>> {
        async move { Ok(()) }
    }
    fn after_update(
        &mut self,
        _session: &CachingSession,
        _extension: &Self::Extension,
    ) -> impl Future<Output = Result<(), Err>> {
        async move { Ok(()) }
    }
    fn before_delete(
        &mut self,
        _session: &CachingSession,
        _extension: &Self::Extension,
    ) -> impl Future<Output = Result<(), Err>> {
        async move { Ok(()) }
    }
    fn after_delete(
        &mut self,
        _session: &CachingSession,
        _extension: &Self::Extension,
    ) -> impl Future<Output = Result<(), Err>> {
        async move { Ok(()) }
    }
}
