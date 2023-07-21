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
            .map_err(|e| CharybdisError::QueryError(e))
    }
}

pub trait UpdateWithCallbacks {
    async fn update_cb(&mut self, session: &CachingSession) -> Result<QueryResult, CharybdisError>;
}

impl<T: Model + ValueList + Update + Callbacks> UpdateWithCallbacks for T {
    async fn update_cb(&mut self, session: &CachingSession) -> Result<QueryResult, CharybdisError> {
        self.before_update(session).await?;
        let res = self.update(session).await;
        self.after_update(session).await?;

        res
    }
}

pub trait UpdateWithExtCallbacks<E> {
    async fn update_cb(
        &mut self,
        session: &CachingSession,
        extension: &E,
    ) -> Result<QueryResult, CharybdisError>;
}

impl<T: Model + ValueList + Update + ExtCallbacks<E>, E> UpdateWithExtCallbacks<E> for T {
    async fn update_cb(
        &mut self,
        session: &CachingSession,
        extension: &E,
    ) -> Result<QueryResult, CharybdisError> {
        self.before_update(session, extension).await?;
        let res = self.update(session).await;
        self.after_update(session, extension).await?;

        res
    }
}
