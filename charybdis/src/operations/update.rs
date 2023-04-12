use scylla::_macro_internal::ValueList;
use scylla::{CachingSession, QueryResult};
use scylla::transport::errors::QueryError;
use crate::model::Model;
use crate::prelude::SerializedValues;

pub trait Update {
    async fn update(&self, session: &CachingSession) -> Result<QueryResult, QueryError>;
}

impl <T:  Model + ValueList> Update  for T {
    async fn update(&self, session: &CachingSession) -> Result<QueryResult, QueryError> {
        println!("update query: {}", Self::UPDATE_QUERY);
        let update_values: SerializedValues = self.get_update_values();

        session.execute(Self::UPDATE_QUERY, update_values).await
    }
}
