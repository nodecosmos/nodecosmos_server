use crate::callbacks::Callbacks;
use crate::errors::CharybdisError;
use crate::model::Model;
use crate::ExtCallbacks;
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

pub trait InsertWithCallbacks {
    async fn insert_cb(&mut self, session: &CachingSession) -> Result<QueryResult, CharybdisError>;
}

impl<T: Model + ValueList + Callbacks + Insert> InsertWithCallbacks for T {
    async fn insert_cb(&mut self, session: &CachingSession) -> Result<QueryResult, CharybdisError> {
        self.before_insert(session).await?;
        let res = self.insert(session).await;
        self.after_insert(session).await?;

        res
    }
}

pub trait InsertWithExtCallbacks<E> {
    async fn insert_cb(
        &mut self,
        session: &CachingSession,
        extension: &E,
    ) -> Result<QueryResult, CharybdisError>;
}

impl<T: Model + ValueList + Insert + ExtCallbacks<E>, E> InsertWithExtCallbacks<E> for T {
    async fn insert_cb(
        &mut self,
        session: &CachingSession,
        extension: &E,
    ) -> Result<QueryResult, CharybdisError> {
        self.before_insert(session, extension).await?;
        let res = self.insert(session).await;
        self.after_insert(session, extension).await?;

        res
    }
}
