use scylla::{CachingSession, QueryResult};
use scylla::transport::errors::QueryError;

use crate::model::Model;
use crate::prelude::{SerializedValues};

pub trait FindByPrimaryKey {
    async fn find_by_primary_key(&self, session: &CachingSession) -> Result<Self, QueryError> where Self: Model;
}

impl <T:  Model> FindByPrimaryKey for T {
    async fn find_by_primary_key(&self, session: &CachingSession) -> Result<Self, QueryError> {
        let primary_key_values: SerializedValues = self.get_primary_key_values();
        let result: QueryResult = session.execute(Self::FIND_BY_PRIMARY_KEY_QUERY, primary_key_values).await?;

        Ok(result.single_row_typed::<Self>().unwrap())
    }
}
