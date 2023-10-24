use crate::callbacks::{Callbacks, ExtCallbacks};
use crate::errors::CharybdisError;
use crate::model::Model;
use scylla::frame::value::ValueList;
use scylla::{CachingSession, QueryResult};
use std::future::Future;

pub trait Delete {
    fn delete(&self, session: &CachingSession) -> impl Future<Output = Result<QueryResult, CharybdisError>>;
    fn delete_by_partition_key(
        &self,
        session: &CachingSession,
    ) -> impl Future<Output = Result<QueryResult, CharybdisError>>;
}

impl<T: Model + ValueList> Delete for T {
    fn delete(&self, session: &CachingSession) -> impl Future<Output = Result<QueryResult, CharybdisError>> {
        async move {
            let primary_key_values = self
                .get_primary_key_values()
                .map_err(|e| CharybdisError::SerializeValuesError(e, Self::DB_MODEL_NAME.to_string()))?;

            session
                .execute(T::DELETE_QUERY, primary_key_values)
                .await
                .map_err(CharybdisError::QueryError)
        }
    }

    fn delete_by_partition_key(
        &self,
        session: &CachingSession,
    ) -> impl Future<Output = Result<QueryResult, CharybdisError>> {
        async move {
            let partition_key_values = self
                .get_partition_key_values()
                .map_err(|e| CharybdisError::SerializeValuesError(e, Self::DB_MODEL_NAME.to_string()))?;

            session
                .execute(T::DELETE_BY_PARTITION_KEY_QUERY, partition_key_values)
                .await
                .map_err(CharybdisError::QueryError)
        }
    }
}

pub trait DeleteWithCallbacks<Err> {
    fn delete_cb(&mut self, session: &CachingSession) -> impl Future<Output = Result<QueryResult, Err>>;
}

impl<Err, T> DeleteWithCallbacks<Err> for T
where
    Err: From<CharybdisError>,
    T: Model + ValueList + Delete + Callbacks<Err>,
{
    fn delete_cb(&mut self, session: &CachingSession) -> impl Future<Output = Result<QueryResult, Err>> {
        async move {
            self.before_delete(session).await?;
            let res = self.delete(session).await;
            self.after_delete(session).await?;

            Ok(res?)
        }
    }
}

pub trait DeleteWithExtCallbacks<E, Err> {
    fn delete_cb(&mut self, session: &CachingSession, extension: &E) -> impl Future<Output = Result<QueryResult, Err>>;
}

impl<T, Ext, Err> DeleteWithExtCallbacks<Ext, Err> for T
where
    Err: From<CharybdisError>,
    T: Model + ValueList + Delete + ExtCallbacks<Ext, Err>,
{
    fn delete_cb(
        &mut self,
        session: &CachingSession,
        extension: &Ext,
    ) -> impl Future<Output = Result<QueryResult, Err>> {
        async move {
            self.before_delete(session, extension).await?;
            let res = self.delete(session).await;
            self.after_delete(session, extension).await?;

            Ok(res?)
        }
    }
}
