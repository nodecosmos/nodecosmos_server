use crate::callbacks::{Callbacks, ExtCallbacks};
use crate::errors::CharybdisError;
use crate::model::Model;
use scylla::frame::value::ValueList;
use scylla::{CachingSession, QueryResult};

pub trait Insert {
    async fn insert(&self, session: &CachingSession) -> Result<QueryResult, CharybdisError>;
}

impl<T: Model + ValueList> Insert for T {
    async fn insert(&self, session: &CachingSession) -> Result<QueryResult, CharybdisError> {
        Ok(session.execute(T::INSERT_QUERY, self).await?)
    }
}

pub trait InsertWithCallbacks<Err> {
    async fn insert_cb(&mut self, session: &CachingSession) -> Result<QueryResult, Err>;
}

impl<Err, T> InsertWithCallbacks<Err> for T
where
    Err: From<CharybdisError>,
    T: Model + ValueList + Insert + Callbacks<Err>,
{
    async fn insert_cb(&mut self, session: &CachingSession) -> Result<QueryResult, Err> {
        self.before_insert(session).await?;
        let res = self.insert(session).await;
        self.after_insert(session).await?;

        Ok(res?)
    }
}

pub trait InsertWithExtCallbacks<Ext, Err> {
    async fn insert_cb(
        &mut self,
        session: &CachingSession,
        extension: &Ext,
    ) -> Result<QueryResult, Err>;
}

impl<T, Ext, Err> InsertWithExtCallbacks<Ext, Err> for T
where
    Err: From<CharybdisError>,
    T: Model + ValueList + Insert + ExtCallbacks<Ext, Err>,
{
    async fn insert_cb(
        &mut self,
        session: &CachingSession,
        extension: &Ext,
    ) -> Result<QueryResult, Err> {
        self.before_insert(session, extension).await?;
        let res = self.insert(session).await;
        self.after_insert(session, extension).await?;

        Ok(res?)
    }
}
