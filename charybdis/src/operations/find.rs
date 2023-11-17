use scylla::frame::value::ValueList;
use scylla::query::Query;
use scylla::{Bytes, IntoTypedRows};
use scylla::{CachingSession, QueryResult};
use std::future::Future;

use crate::errors::CharybdisError;
use crate::iterator::CharybdisModelIterator;
use crate::model::BaseModel;
use crate::stream::CharybdisModelStream;

pub trait Find: BaseModel {
    fn find(
        session: &CachingSession,
        query: &'static str,
        values: impl ValueList,
    ) -> impl Future<Output = Result<CharybdisModelStream<Self>, CharybdisError>>;

    fn find_first(
        session: &CachingSession,
        query: &'static str,
        values: impl ValueList,
    ) -> impl Future<Output = Result<Self, CharybdisError>>;

    fn find_paged(
        session: &CachingSession,
        query: &'static str,
        values: impl ValueList,
        page_size: Option<Bytes>,
    ) -> impl Future<Output = Result<(CharybdisModelIterator<Self>, Option<Bytes>), CharybdisError>>;

    fn find_all(
        session: &CachingSession,
        query: &'static str,
        values: impl ValueList,
    ) -> impl Future<Output = Result<CharybdisModelIterator<Self>, CharybdisError>>;

    fn find_by_primary_key_value(
        session: &CachingSession,
        value: impl ValueList,
    ) -> impl Future<Output = Result<Self, CharybdisError>>;

    fn find_by_partition_key_value(
        session: &CachingSession,
        value: impl ValueList,
    ) -> impl Future<Output = Result<CharybdisModelStream<Self>, CharybdisError>>;

    fn find_first_by_partition_key_value(
        session: &CachingSession,
        value: impl ValueList,
    ) -> impl Future<Output = Result<Self, CharybdisError>>;

    fn find_by_primary_key(&self, session: &CachingSession) -> impl Future<Output = Result<Self, CharybdisError>>;

    fn find_by_partition_key(
        &self,
        session: &CachingSession,
    ) -> impl Future<Output = Result<CharybdisModelStream<Self>, CharybdisError>>;
}

impl<T: BaseModel> Find for T {
    // find iter
    fn find(
        session: &CachingSession,
        query: &'static str,
        values: impl ValueList,
    ) -> impl Future<Output = Result<CharybdisModelStream<Self>, CharybdisError>> {
        async move {
            let query = Query::new(query);

            let rows = session.execute_iter(query, values).await?.into_typed();

            Ok(CharybdisModelStream::from(rows))
        }
    }

    fn find_first(
        session: &CachingSession,
        query: &'static str,
        values: impl ValueList,
    ) -> impl Future<Output = Result<Self, CharybdisError>> {
        async move {
            let result: QueryResult = session.execute(query, values).await?;
            let typed_row: Self = result.first_row_typed()?;

            Ok(typed_row)
        }
    }

    fn find_paged(
        session: &CachingSession,
        query: &'static str,
        values: impl ValueList,
        paging_state: Option<Bytes>,
    ) -> impl Future<Output = Result<(CharybdisModelIterator<Self>, Option<Bytes>), CharybdisError>> {
        async move {
            let res = session.execute_paged(query, values, paging_state).await?;
            let paging_state = res.paging_state.clone();
            let rows = res.rows()?;
            let typed_rows = CharybdisModelIterator::from(rows.into_typed());

            Ok((typed_rows, paging_state))
        }
    }

    fn find_all(
        session: &CachingSession,
        query: &'static str,
        values: impl ValueList,
    ) -> impl Future<Output = Result<CharybdisModelIterator<Self>, CharybdisError>> {
        async move {
            let result: QueryResult = session.execute(query, values).await?;

            let rows = result.rows()?;
            let typed_rows = CharybdisModelIterator::from(rows.into_typed());

            Ok(typed_rows)
        }
    }

    fn find_by_primary_key_value(
        session: &CachingSession,
        value: impl ValueList,
    ) -> impl Future<Output = Result<Self, CharybdisError>> {
        async move {
            let result: QueryResult = session.execute(Self::FIND_BY_PRIMARY_KEY_QUERY, value).await?;

            let res = result.first_row_typed()?;

            Ok(res)
        }
    }

    fn find_by_partition_key_value(
        session: &CachingSession,
        value: impl ValueList,
    ) -> impl Future<Output = Result<CharybdisModelStream<Self>, CharybdisError>> {
        async move {
            let rows = session
                .execute_iter(Self::FIND_BY_PARTITION_KEY_QUERY, value)
                .await?
                .into_typed::<Self>();

            Ok(CharybdisModelStream::from(rows))
        }
    }

    fn find_first_by_partition_key_value(
        session: &CachingSession,
        value: impl ValueList,
    ) -> impl Future<Output = Result<Self, CharybdisError>> {
        async move {
            let result: QueryResult = session.execute(Self::FIND_BY_PARTITION_KEY_QUERY, value).await?;

            let res = result.first_row_typed()?;

            Ok(res)
        }
    }

    /// Preferred way to find by partition key, as keys will be in correct order
    fn find_by_primary_key(&self, session: &CachingSession) -> impl Future<Output = Result<Self, CharybdisError>> {
        async move {
            let primary_key_values = self
                .primary_key_values()
                .map_err(|e| CharybdisError::SerializeValuesError(e, Self::DB_MODEL_NAME.to_string()))?;

            let result: QueryResult = session
                .execute(Self::FIND_BY_PRIMARY_KEY_QUERY, &primary_key_values)
                .await?;

            let res = result.first_row_typed()?;

            Ok(res)
        }
    }

    /// Preferred way to find by partition key, as keys will be in correct order
    fn find_by_partition_key(
        &self,
        session: &CachingSession,
    ) -> impl Future<Output = Result<CharybdisModelStream<Self>, CharybdisError>> {
        async move {
            let partition_key_values = self
                .partition_key_values()
                .map_err(|e| CharybdisError::SerializeValuesError(e, Self::DB_MODEL_NAME.to_string()))?;

            let rows = session
                .execute_iter(Self::FIND_BY_PARTITION_KEY_QUERY, partition_key_values)
                .await?
                .into_typed::<Self>();

            Ok(CharybdisModelStream::from(rows))
        }
    }
}
