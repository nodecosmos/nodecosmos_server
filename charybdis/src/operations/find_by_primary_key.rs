use scylla::{CachingSession, QueryResult};
use crate::errors::CharybdisError;

use crate::model::Model;
use crate::prelude::{SerializedValues};

pub trait FindByPrimaryKey {
    async fn find_by_primary_key(&self, session: &CachingSession) -> Result<Self, CharybdisError> where Self: Model;
}

impl <T:  Model> FindByPrimaryKey for T {
    async fn find_by_primary_key(&self, session: &CachingSession) -> Result<Self, CharybdisError> {
        let primary_key_values: SerializedValues = self.get_primary_key_values();
        let result: QueryResult = session.execute(Self::FIND_BY_PRIMARY_KEY_QUERY, &primary_key_values)
            .await.map_err(|e| CharybdisError::QueryError(e))?;

        let res = result.single_row_typed::<Self>().map_err(|e|
            CharybdisError::SingleRowTypedError(e, Self::DB_MODEL_NAME.to_string())
        )?;

        Ok(res)
    }
}
