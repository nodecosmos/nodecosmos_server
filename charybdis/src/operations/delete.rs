use scylla::_macro_internal::ValueList;
use scylla::{CachingSession, QueryResult};
use scylla::transport::errors::QueryError;

use crate::model::Model;
use crate::prelude::SerializedValues;

pub trait Delete {
    async fn delete(&self, session: &CachingSession) -> Result<QueryResult, QueryError>;
}

impl <T:  Model + ValueList> Delete  for T {
    /// For deleting a row we can use either base model struct or partial_model struct that has only primary key fields.
    /// For deleting columns we can use partial_model struct that has only primary key fields and columns to be deleted.
    async fn delete(&self, session: &CachingSession) -> Result<QueryResult, QueryError> {
        let primary_key_values: SerializedValues = self.get_primary_key_values();

        session.execute(T::DELETE_QUERY, primary_key_values).await
    }
}
