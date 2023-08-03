use crate::CharybdisError;
use scylla::_macro_internal::ValueList;
use scylla::query::Query;
use scylla::{CachingSession, QueryResult};

pub async fn execute(
    session: &CachingSession,
    query: impl Into<Query>,
    values: impl ValueList,
) -> Result<QueryResult, CharybdisError> {
    let res = session.execute(query, values).await?;
    Ok(res)
}
