use scylla::{CachingSession, QueryResult};
use crate::errors::CharybdisError;

use crate::model::Model;
use crate::prelude::{
    SerializedValues,
    TypedRowIter,
};

pub trait FindByPartitionKey {
    async fn find_by_partition_key(&self, session: &CachingSession) -> Result<TypedRowIter<Self>, CharybdisError> where Self: Model;
}

impl <T:  Model> FindByPartitionKey for T {
    async fn find_by_partition_key(&self, session: &CachingSession) -> Result<TypedRowIter<Self>, CharybdisError> {
        let primary_key_values: SerializedValues = self.get_partition_key_values();
        let result: QueryResult = session.execute(Self::FIND_BY_PARTITION_KEY_QUERY, primary_key_values)
            .await.map_err(|e| CharybdisError::QueryError(e))?;

        let res = result.rows_typed::<Self>().map_err(|e| CharybdisError::RowsExpectedError(e))?;
        
        Ok(res)
    }
}
