use scylla::_macro_internal::ValueList;
use scylla::{CachingSession, QueryResult};
use scylla::transport::errors::QueryError;
use crate::model::Model;

pub trait Insert {
    async fn insert(&self, session: &CachingSession) -> Result<QueryResult, QueryError>;
}

impl <T:  Model + ValueList> Insert  for T {
    async fn insert(&self, session: &CachingSession) -> Result<QueryResult, QueryError> {
        session.execute(T::INSERT_QUERY, self).await
    }
}
