use scylla::_macro_internal::ValueList;
use scylla::{CachingSession, QueryResult};
use scylla::transport::errors::QueryError;
use crate::model::Model;

pub trait Update {
    async fn update(&self, session: &CachingSession) -> Result<QueryResult, QueryError>;
}

impl <T:  Model + ValueList> Update  for T {
    async fn update(&self, session: &CachingSession) -> Result<QueryResult, QueryError> {
        session.execute(Self::UPDATE_QUERY, self).await
    }
}
