use scylla::frame::value::ValueList;
use scylla::transport::session::TypedRowIter;
use scylla::IntoTypedRows;
use scylla::{CachingSession, QueryResult};

use crate::errors::CharybdisError;
use crate::model::BaseModel;
use crate::prelude::SerializedValues;

pub trait FindByPrimaryKey: BaseModel {
    async fn find(
        session: &CachingSession,
        clause: &'static str,
        values: impl ValueList,
    ) -> Result<TypedRowIter<Self>, CharybdisError>;

    // methods
    async fn find_by_primary_key(&self, session: &CachingSession) -> Result<Self, CharybdisError>;
    async fn find_by_partition_key(
        &self,
        session: &CachingSession,
    ) -> Result<TypedRowIter<Self>, CharybdisError>;
}

impl<T: BaseModel> FindByPrimaryKey for T {
    async fn find(
        session: &CachingSession,
        query: &'static str,
        values: impl ValueList,
    ) -> Result<TypedRowIter<Self>, CharybdisError> {
        let result: QueryResult = session
            .execute(query, values)
            .await
            .map_err(|e| CharybdisError::QueryError(e))?;

        match result.rows {
            Some(rows) => {
                let typed_rows: TypedRowIter<Self> = rows.into_typed();
                Ok(typed_rows)
            }
            None => Err(CharybdisError::NotFoundError(
                Self::DB_MODEL_NAME.to_string(),
            )),
        }
    }

    // methods
    async fn find_by_primary_key(&self, session: &CachingSession) -> Result<Self, CharybdisError> {
        let primary_key_values: SerializedValues = self.get_primary_key_values();
        let result: QueryResult = session
            .execute(Self::FIND_BY_PRIMARY_KEY_QUERY, &primary_key_values)
            .await
            .map_err(|e| CharybdisError::QueryError(e))?;

        let res = result
            .single_row_typed::<Self>()
            .map_err(|e| CharybdisError::SingleRowTypedError(e, Self::DB_MODEL_NAME.to_string()))?;

        Ok(res)
    }

    async fn find_by_partition_key(
        &self,
        session: &CachingSession,
    ) -> Result<TypedRowIter<Self>, CharybdisError> {
        let get_partition_key_values: SerializedValues = self.get_partition_key_values();

        let result: QueryResult = session
            .execute(Self::FIND_BY_PARTITION_KEY_QUERY, get_partition_key_values)
            .await
            .map_err(|e| CharybdisError::QueryError(e))?;

        match result.rows {
            Some(rows) => {
                let typed_rows: TypedRowIter<Self> = rows.into_typed();
                Ok(typed_rows)
            }
            None => Err(CharybdisError::NotFoundError(
                Self::DB_MODEL_NAME.to_string(),
            )),
        }
    }
}
