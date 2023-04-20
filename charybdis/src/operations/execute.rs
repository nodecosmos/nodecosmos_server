use crate::model::Model;
use crate::prelude::CqlValue;
use scylla::_macro_internal::ValueList;
use scylla::transport::errors::QueryError;
use scylla::{CachingSession, QueryResult};

pub trait Execute {
    async fn execute(
        &self,
        session: &CachingSession,
        query: &'static str,
        values: Vec<_>,
    ) -> Result<QueryResult, QueryError>;
}

impl<T: Model + ValueList> Execute for T {
    async fn execute(
        &self,
        session: &CachingSession,
        query: &'static str,
        values: Vec<_>,
    ) -> Result<QueryResult, QueryError> {
        session.execute(query, values).await
    }
}
