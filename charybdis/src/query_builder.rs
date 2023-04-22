use scylla::transport::session::TypedRowIter;
use scylla::{CachingSession, IntoTypedRows, QueryResult};

use crate::errors::CharybdisError;
use crate::model::Model;
use crate::prelude::SerializedValues;

pub struct CharybdisQuery<T: Model> {
    pub query: &'static str,
    pub values: SerializedValues,
    pub phantom_data: std::marker::PhantomData<T>,
}

impl<T: Model> CharybdisQuery<T> {
    pub async fn execute(
        &self,
        session: &CachingSession,
    ) -> Result<TypedRowIter<T>, CharybdisError> {
        let result: QueryResult = session
            .execute(self.query, &self.values)
            .await
            .map_err(|e| CharybdisError::QueryError(e))?;

        match result.rows {
            Some(rows) => {
                let typed_rows: TypedRowIter<T> = rows.into_typed();
                Ok(typed_rows)
            }
            None => Err(CharybdisError::NotFoundError(T::DB_MODEL_NAME)),
        }
    }
}
