use crate::callbacks::Callbacks;
use crate::model::Model;

use crate::errors::CharybdisError;
use scylla::frame::value::ValueList;
use scylla::{CachingSession, QueryResult};

pub trait Insert {
    async fn insert(&self, session: &CachingSession) -> Result<QueryResult, CharybdisError>;
}

impl<T: Model + ValueList> Insert for T {
    async fn insert(&self, session: &CachingSession) -> Result<QueryResult, CharybdisError> {
        session
            .execute(T::INSERT_QUERY, self)
            .await
            .map_err(|e| CharybdisError::QueryError(e))
    }
}

/// If model implements callbacks, then we can use insert_cb to call before_insert and after_insert
pub trait InsertWithCallbacks {
    async fn insert_cb(&self, session: &CachingSession) -> Result<QueryResult, CharybdisError>;
}

impl<T: Model + ValueList + Callbacks + Insert> InsertWithCallbacks for T {
    async fn insert_cb(&self, session: &CachingSession) -> Result<QueryResult, CharybdisError> {
        self.before_insert().await?;
        let res = self.insert(session).await;
        self.after_insert().await?;

        res
    }
}
