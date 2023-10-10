use crate::callbacks::Callbacks;
use crate::errors::CharybdisError;
use crate::model::Model;
use crate::ExtCallbacks;
use scylla::frame::value::ValueList;
use scylla::{CachingSession, QueryResult};

pub trait Update {
    async fn update(&self, session: &CachingSession) -> Result<QueryResult, CharybdisError>;
}

impl<T: Model + ValueList> Update for T {
    async fn update(&self, session: &CachingSession) -> Result<QueryResult, CharybdisError> {
        let update_values = self.get_update_values().map_err(|e| {
            CharybdisError::SerializeValuesError(e, Self::DB_MODEL_NAME.to_string())
        })?;

        session
            .execute(Self::UPDATE_QUERY, update_values)
            .await
            .map_err(CharybdisError::QueryError)
    }
}

pub trait UpdateWithCallbacks<Err> {
    async fn update_cb(&mut self, session: &CachingSession) -> Result<QueryResult, Err>;
}

impl<T, Err> UpdateWithCallbacks<Err> for T
where
    Err: From<CharybdisError>,
    T: Model + ValueList + Update + Callbacks<Err>,
{
    async fn update_cb(&mut self, session: &CachingSession) -> Result<QueryResult, Err> {
        self.before_update(session).await?;
        let res = self.update(session).await;
        self.after_update(session).await?;

        Ok(res?)
    }
}

pub trait UpdateWithExtCallbacks<Ext, Err> {
    async fn update_cb(
        &mut self,
        session: &CachingSession,
        extension: &Ext,
    ) -> Result<QueryResult, Err>;
}

impl<T, Ext, Err> UpdateWithExtCallbacks<Ext, Err> for T
where
    Err: From<CharybdisError>,
    T: Model + ValueList + Update + ExtCallbacks<Ext, Err>,
{
    async fn update_cb(
        &mut self,
        session: &CachingSession,
        extension: &Ext,
    ) -> Result<QueryResult, Err> {
        self.before_update(session, extension).await?;
        let res = self.update(session).await;
        self.after_update(session, extension).await?;

        Ok(res?)
    }
}
