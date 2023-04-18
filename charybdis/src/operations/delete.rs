use scylla::_macro_internal::ValueList;
use scylla::transport::errors::QueryError;
use scylla::{CachingSession, QueryResult};

use crate::model::Model;
use crate::prelude::SerializedValues;

pub trait Delete {
    async fn delete(&self, session: &CachingSession) -> Result<QueryResult, QueryError>;
}

impl<T: Model + ValueList> Delete for T {
    async fn delete(&self, session: &CachingSession) -> Result<QueryResult, QueryError> {
        let primary_key_values: SerializedValues = self.get_primary_key_values();

        session.execute(T::DELETE_QUERY, primary_key_values).await
    }
}
