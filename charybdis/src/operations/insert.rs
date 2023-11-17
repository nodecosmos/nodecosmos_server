use crate::callbacks::{Callbacks, ExtCallbacks};
use crate::errors::CharybdisError;
use crate::model::Model;
use scylla::frame::value::ValueList;
use scylla::{CachingSession, QueryResult};
use std::future::Future;

pub trait Insert: Model + ValueList {
    // Change the return type of your method to your helper struct.
    fn insert<'a>(&'a self, session: &'a CachingSession) -> impl Future<Output = Result<QueryResult, CharybdisError>>;
}

impl<T: Model + ValueList> Insert for T {
    fn insert<'a>(&'a self, session: &'a CachingSession) -> impl Future<Output = Result<QueryResult, CharybdisError>> {
        async move {
            session
                .execute(T::INSERT_QUERY, self)
                .await
                .map_err(CharybdisError::from)
        }
    }
}

pub trait InsertWithCallbacks<Err> {
    fn insert_cb(&mut self, session: &CachingSession) -> impl Future<Output = Result<QueryResult, Err>>;
}

impl<Err, T> InsertWithCallbacks<Err> for T
where
    Err: From<CharybdisError>,
    T: Model + ValueList + Insert + Callbacks<Err>,
{
    fn insert_cb(&mut self, session: &CachingSession) -> impl Future<Output = Result<QueryResult, Err>> {
        async move {
            self.before_insert(session).await?;
            let res = self.insert(session).await;
            self.after_insert(session).await?;

            res.map_err(Err::from)
        }
    }
}

pub trait InsertWithExtCallbacks<Err, T>
where
    Err: From<CharybdisError>,
    T: Model + ValueList + Insert + ExtCallbacks<Err>,
{
    fn insert_cb(
        &mut self,
        session: &CachingSession,
        extension: &T::Extension,
    ) -> impl Future<Output = Result<QueryResult, Err>>;
}

impl<T, Err> InsertWithExtCallbacks<Err, T> for T
where
    Err: From<CharybdisError>,
    T: Model + ValueList + Insert + ExtCallbacks<Err>,
{
    fn insert_cb(
        &mut self,
        session: &CachingSession,
        extension: &T::Extension,
    ) -> impl Future<Output = Result<QueryResult, Err>> {
        async move {
            self.before_insert(session, extension).await?;
            let res = self.insert(session).await;
            self.after_insert(session, extension).await?;

            res.map_err(Err::from)
        }
    }
}
