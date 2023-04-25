use crate::model::Model;
use scylla::_macro_internal::ValueList;
use scylla::query::Query;
use scylla::transport::errors::QueryError;
use scylla::{CachingSession, QueryResult};

pub trait Execute {
    async fn execute(
        &self,
        session: &CachingSession,
        query: impl Into<Query>,
        values: impl ValueList,
    ) -> Result<QueryResult, QueryError>;
}

impl<T: Model + ValueList> Execute for T {
    async fn execute(
        &self,
        session: &CachingSession,
        query: impl Into<Query>,
        values: impl ValueList,
    ) -> Result<QueryResult, QueryError> {
        session.execute(query, values).await
    }
}
