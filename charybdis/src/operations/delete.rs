use crate::batch::CharybdisModelBatch;
use crate::callbacks::{Callbacks, ExtCallbacks};
use crate::errors::CharybdisError;
use crate::model::Model;
use scylla::frame::value::ValueList;
use scylla::{CachingSession, QueryResult};

pub trait Delete {
    async fn delete(&self, session: &CachingSession) -> Result<QueryResult, CharybdisError>;
    async fn delete_by_partition_key(&self, session: &CachingSession) -> Result<QueryResult, CharybdisError>;
}

impl<T: Model + ValueList> Delete for T {
    async fn delete(&self, session: &CachingSession) -> Result<QueryResult, CharybdisError> {
        let primary_key_values = self
            .get_primary_key_values()
            .map_err(|e| CharybdisError::SerializeValuesError(e, Self::DB_MODEL_NAME.to_string()))?;

        session
            .execute(T::DELETE_QUERY, primary_key_values)
            .await
            .map_err(CharybdisError::QueryError)
    }

    async fn delete_by_partition_key(&self, session: &CachingSession) -> Result<QueryResult, CharybdisError> {
        let partition_key_values = self
            .get_partition_key_values()
            .map_err(|e| CharybdisError::SerializeValuesError(e, Self::DB_MODEL_NAME.to_string()))?;

        session
            .execute(T::DELETE_BY_PARTITION_KEY_QUERY, partition_key_values)
            .await
            .map_err(CharybdisError::QueryError)
    }
}

pub trait DeleteAll<T, I>: Iterator<Item = T>
where
    T: Model + ValueList,
{
    async fn delete_all(&mut self, session: &CachingSession) -> Result<QueryResult, CharybdisError> {
        let mut batch = CharybdisModelBatch::unlogged();

        batch.append_deletes(self)?;

        let res = batch.execute(session).await?;

        Ok(res)
    }
}

pub trait DeleteWithCallbacks<Err> {
    async fn delete_cb(&mut self, session: &CachingSession) -> Result<QueryResult, Err>;
}

impl<Err, T> DeleteWithCallbacks<Err> for T
where
    Err: From<CharybdisError>,
    T: Model + ValueList + Delete + Callbacks<Err>,
{
    async fn delete_cb(&mut self, session: &CachingSession) -> Result<QueryResult, Err> {
        self.before_delete(session).await?;
        let res = self.delete(session).await;
        self.after_delete(session).await?;

        Ok(res?)
    }
}

pub trait DeleteWithExtCallbacks<E, Err> {
    async fn delete_cb(&mut self, session: &CachingSession, extension: &E) -> Result<QueryResult, Err>;
}

impl<T, Ext, Err> DeleteWithExtCallbacks<Ext, Err> for T
where
    Err: From<CharybdisError>,
    T: Model + ValueList + Delete + ExtCallbacks<Ext, Err>,
{
    async fn delete_cb(&mut self, session: &CachingSession, extension: &Ext) -> Result<QueryResult, Err> {
        self.before_delete(session, extension).await?;
        let res = self.delete(session).await;
        self.after_delete(session, extension).await?;

        Ok(res?)
    }
}
