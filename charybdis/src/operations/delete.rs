use crate::callbacks::Callbacks;
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
            .map_err(|e| CharybdisError::QueryError(e))
    }
}

pub trait DeleteWithCallbacks {
    async fn delete_cb(&self, session: &CachingSession) -> Result<QueryResult, CharybdisError>;
}

impl<T: Model + ValueList + Delete + Callbacks> DeleteWithCallbacks for T {
    async fn delete_cb(&self, session: &CachingSession) -> Result<QueryResult, CharybdisError> {
        self.before_delete(session).await?;
        let res = self.delete(session).await;
        self.after_delete(session).await?;

        res
    }
}
