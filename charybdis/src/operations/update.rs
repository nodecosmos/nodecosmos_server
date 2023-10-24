use crate::callbacks::{Callbacks, ExtCallbacks};
use crate::errors::CharybdisError;
use crate::model::Model;
use scylla::frame::value::ValueList;
use scylla::{CachingSession, QueryResult};
use std::future::Future;

pub trait Update {
    fn update(&self, session: &CachingSession) -> impl Future<Output = Result<QueryResult, CharybdisError>>;
}

impl<T: Model + ValueList> Update for T {
    fn update(&self, session: &CachingSession) -> impl Future<Output = Result<QueryResult, CharybdisError>> {
        async move {
            let update_values = self.get_update_values()?;

            session
                .execute(Self::UPDATE_QUERY, update_values)
                .await
                .map_err(CharybdisError::from)
        }
    }
}

pub trait UpdateWithCallbacks<'a, Err> {
    fn update_cb(&mut self, session: &CachingSession) -> impl Future<Output = Result<QueryResult, Err>>;
}

impl<'a, T, Err> UpdateWithCallbacks<'a, Err> for T
where
    Err: From<CharybdisError>,
    T: Model + ValueList + Update + Callbacks<Err>,
{
    fn update_cb(&mut self, session: &CachingSession) -> impl Future<Output = Result<QueryResult, Err>> {
        async move {
            self.before_update(session).await?;
            let res = self.update(session).await;
            self.after_update(session).await?;

            res.map_err(Err::from)
        }
    }
}

pub trait UpdateWithExtCallbacks<'a, Ext, Err> {
    fn update_cb(
        &mut self,
        session: &CachingSession,
        extension: &Ext,
    ) -> impl Future<Output = Result<QueryResult, Err>>;
}

impl<'a, T, Ext, Err> UpdateWithExtCallbacks<'a, Ext, Err> for T
where
    Err: From<CharybdisError>,
    T: Model + ValueList + Update + ExtCallbacks<Ext, Err>,
{
    fn update_cb(
        &mut self,
        session: &CachingSession,
        extension: &Ext,
    ) -> impl Future<Output = Result<QueryResult, Err>> {
        async move {
            self.before_update(session, extension).await?;
            let res = self.update(session).await;
            // self.after_update(session, extension).await?;

            res.map_err(Err::from)
        }
    }
}
