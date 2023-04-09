use scylla::_macro_internal::ValueList;
use scylla::{CachingSession, QueryResult, Session};
use scylla::transport::errors::QueryError;
use crate::model::Model;

pub trait Insert
{
    async fn insert(&self, session: &CachingSession) -> Result<QueryResult, QueryError>;
}


// From Scylla docs:
// Prepared queries have good performance, much better than simple queries.
// By default they use shard/token aware load balancing.
// Always pass partition key values as bound values.
// Otherwise the driver can’t hash them to compute partition key and they will be sent
// to the wrong node, which worsens performance.
impl <T:  Model + ValueList> Insert  for T {
    async fn insert(&self, session: &CachingSession) -> Result<QueryResult, QueryError> {
        session.execute(T::INSERT_QUERY, self).await
    }
}
