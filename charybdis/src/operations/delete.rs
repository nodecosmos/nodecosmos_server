use crate::callbacks::{Callbacks, ExtCallbacks};
use crate::errors::CharybdisError;
use crate::model::Model;
use scylla::frame::value::ValueList;
use scylla::{CachingSession, QueryResult};

pub trait Delete {
    async fn delete(&self, session: &CachingSession) -> Result<QueryResult, CharybdisError>;
}

impl<T: Model + ValueList> Delete for T {
    async fn delete(&self, session: &CachingSession) -> Result<QueryResult, CharybdisError> {
        let primary_key_values = self.get_primary_key_values().map_err(|e| {
            CharybdisError::SerializeValuesError(e, Self::DB_MODEL_NAME.to_string())
        })?;

        session
            .execute(T::DELETE_QUERY, primary_key_values)
            .await
            .map_err(CharybdisError::QueryError)
    }
}

pub trait DeleteWithCallbacks {
    async fn delete_cb(&mut self, session: &CachingSession) -> Result<QueryResult, CharybdisError>;
}

impl<T: Model + ValueList + Delete + Callbacks> DeleteWithCallbacks for T {
    async fn delete_cb(&mut self, session: &CachingSession) -> Result<QueryResult, CharybdisError> {
        self.before_delete(session).await?;
        let res = self.delete(session).await;
        self.after_delete(session).await?;

        res
    }
}

pub trait DeleteWithExtCallbacks<E> {
    async fn delete_cb(
        &mut self,
        session: &CachingSession,
        extension: &E,
    ) -> Result<QueryResult, CharybdisError>;
}

impl<T: Model + ValueList + Delete + ExtCallbacks<E>, E> DeleteWithExtCallbacks<E> for T {
    async fn delete_cb(
        &mut self,
        session: &CachingSession,
        extension: &E,
    ) -> Result<QueryResult, CharybdisError> {
        self.before_delete(session, extension).await?;
        let res = self.delete(session).await;
        self.after_delete(session, extension).await?;

        res
    }
}
