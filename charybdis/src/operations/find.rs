use scylla::frame::value::ValueList;
use scylla::query::Query;
use scylla::transport::iterator::TypedRowIterator;
use scylla::transport::session::TypedRowIter;
use scylla::IntoTypedRows;
use scylla::{CachingSession, QueryResult};

use crate::errors::CharybdisError;
use crate::model::BaseModel;

pub trait Find: BaseModel {
    async fn find(
        session: &CachingSession,
        query: &'static str,
        values: impl ValueList,
    ) -> Result<TypedRowIter<Self>, CharybdisError>;

    async fn find_one(
        session: &CachingSession,
        query: &'static str,
        values: impl ValueList,
    ) -> Result<Self, CharybdisError>;

    async fn find_iter(
        session: &CachingSession,
        query: &'static str,
        values: impl ValueList,
        page_size: i32,
    ) -> Result<TypedRowIterator<Self>, CharybdisError>;

    // methods
    async fn find_by_primary_key(&self, session: &CachingSession) -> Result<Self, CharybdisError>;

    async fn find_by_partition_key(
        &self,
        session: &CachingSession,
    ) -> Result<TypedRowIter<Self>, CharybdisError>;
}

impl<T: BaseModel> Find for T {
    async fn find(
        session: &CachingSession,
        query: &'static str,
        values: impl ValueList,
    ) -> Result<TypedRowIter<Self>, CharybdisError> {
        let result: QueryResult = session.execute(query, values).await?;
        let rows = result.rows()?;
        let typed_rows: TypedRowIter<Self> = rows.into_typed();

        Ok(typed_rows)
    }

    async fn find_one(
        session: &CachingSession,
        query: &'static str,
        values: impl ValueList,
    ) -> Result<Self, CharybdisError> {
        println!("{}", query);

        let result: QueryResult = session.execute(query, values).await?;
        let typed_row: Self = result
            .first_row_typed()
            .map_err(|e| CharybdisError::FirstRowTypedError(e, Self::DB_MODEL_NAME.to_string()))?;

        Ok(typed_row)
    }

    // find iter
    async fn find_iter(
        session: &CachingSession,
        query: &'static str,
        values: impl ValueList,
        page_size: i32,
    ) -> Result<TypedRowIterator<Self>, CharybdisError> {
        let query = Query::new(query).with_page_size(page_size);

        let res = session.execute_iter(query, values).await?;
        let typed_rows: TypedRowIterator<Self> = res.into_typed();

        Ok(typed_rows)
    }

    // methods
    async fn find_by_primary_key(&self, session: &CachingSession) -> Result<Self, CharybdisError> {
        let primary_key_values = self.get_primary_key_values().map_err(|e| {
            CharybdisError::SerializeValuesError(e, Self::DB_MODEL_NAME.to_string())
        })?;

        let result: QueryResult = session
            .execute(Self::FIND_BY_PRIMARY_KEY_QUERY, &primary_key_values)
            .await?;

        let res = result
            .single_row_typed::<Self>()
            .map_err(|e| CharybdisError::SingleRowTypedError(e, Self::DB_MODEL_NAME.to_string()))?;

        Ok(res)
    }

    async fn find_by_partition_key(
        &self,
        session: &CachingSession,
    ) -> Result<TypedRowIter<Self>, CharybdisError> {
        let get_partition_key_values = self.get_partition_key_values().map_err(|e| {
            CharybdisError::SerializeValuesError(e, Self::DB_MODEL_NAME.to_string())
        })?;

        let result: QueryResult = session
            .execute(Self::FIND_BY_PARTITION_KEY_QUERY, get_partition_key_values)
            .await?;

        match result.rows {
            Some(rows) => {
                let typed_rows: TypedRowIter<Self> = rows.into_typed();
                Ok(typed_rows)
            }
            None => Err(CharybdisError::NotFoundError(
                Self::FIND_BY_PARTITION_KEY_QUERY.to_string(),
            )),
        }
    }
}
