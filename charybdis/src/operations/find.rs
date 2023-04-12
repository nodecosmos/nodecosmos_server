use scylla::{CachingSession, QueryResult};
use scylla::IntoTypedRows;
use scylla::transport::session::TypedRowIter;

use crate::prelude::SerializedValues;
use crate::model::Model;
use crate::errors::CharybdisError;

pub trait FindByPrimaryKey: Model {
    async fn find_by_primary_key(&self, session: &CachingSession) -> Result<Self, CharybdisError>;
    // async fn find_by_primary_key_typed<T: Model>(&self, session: &CachingSession) -> Result<T, CharybdisError>;
    async fn find_by_partition_key(&self, session: &CachingSession) -> Result<TypedRowIter<Self>, CharybdisError>;
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
    //
    // async fn find_by_primary_key_typed<Type: Model>(&self, session: &CachingSession) -> Result<Type, CharybdisError> {
    //     let primary_key_values: SerializedValues = self.get_primary_key_values();
    //     let result: QueryResult = session.execute(Self::FIND_BY_PRIMARY_KEY_QUERY, &primary_key_values)
    //         .await.map_err(|e| CharybdisError::QueryError(e))?;
    //
    //     let res = result.single_row_typed::<Type>().map_err(|e|
    //         CharybdisError::SingleRowTypedError(e, Self::DB_MODEL_NAME.to_string())
    //     )?;
    //
    //     Ok(res)
    // }

    async fn find_by_partition_key(&self, session: &CachingSession) -> Result<TypedRowIter<Self>, CharybdisError> {
        let get_partition_key_values: SerializedValues = self.get_partition_key_values();

        let result: QueryResult = session
            .execute(Self::FIND_BY_PARTITION_KEY_QUERY, get_partition_key_values)
            .await.map_err(|e| CharybdisError::QueryError(e))?;

        match result.rows {
            Some(rows) => {
                let typed_rows: TypedRowIter<Self> = rows.into_typed();
                Ok(typed_rows)
            }
            None => {
                Err(CharybdisError::NotFoundError(Self::DB_MODEL_NAME.to_string()))
            }
        }
    }
}
