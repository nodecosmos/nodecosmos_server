use crate::{BaseModel, CharybdisError};
use futures::{Stream, StreamExt, TryStreamExt};
use scylla::transport::iterator::{NextRowError, TypedRowIterator};
use std::pin::Pin;
use std::task::{Context, Poll};

pub struct CharybdisModelIterator<T: BaseModel> {
    inner: TypedRowIterator<T>,
}

impl<T: BaseModel> From<TypedRowIterator<T>> for CharybdisModelIterator<T> {
    fn from(iter: TypedRowIterator<T>) -> Self {
        CharybdisModelIterator { inner: iter }
    }
}

impl<T: BaseModel> Stream for CharybdisModelIterator<T> {
    type Item = Result<T, NextRowError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.inner.poll_next_unpin(cx)
    }
}

impl<T: BaseModel> CharybdisModelIterator<T> {
    // Looks like whole point of this function is to convert NextRowError to CharybdisError
    pub async fn try_collect(self) -> Result<Vec<T>, CharybdisError> {
        let results: Result<Vec<T>, NextRowError> = self.inner.try_collect().await;

        results.map_err(|e| CharybdisError::from(e))
    }
}
