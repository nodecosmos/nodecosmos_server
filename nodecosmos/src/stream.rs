use crate::errors::NodecosmosError;
use charybdis::errors::CharybdisError;
use charybdis::model::BaseModel;
use futures::{stream, Stream, StreamExt, TryStreamExt};
use std::pin::Pin;
use std::task::{Context, Poll};

/// When we have a stream of streams, we can merge them into a single stream. This is useful in case of
/// `WHERE IN` queries, as scylla limits the cartesian product of the IN clause to 100 elements. So we can
/// split the IN clause into chunks of 100 elements and execute to get the results.
pub struct MergedModelStream<M: BaseModel + 'static> {
    pub inner: stream::BoxStream<'static, Result<M, CharybdisError>>,
}

impl<M: BaseModel> Stream for MergedModelStream<M> {
    type Item = Result<M, CharybdisError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.inner.poll_next_unpin(cx)
    }
}

impl<M: BaseModel> MergedModelStream<M> {
    pub async fn try_collect(self) -> Result<Vec<M>, NodecosmosError> {
        let results: Result<Vec<M>, NodecosmosError> = self.inner.try_collect().await.map_err(NodecosmosError::from);

        results
    }
}
