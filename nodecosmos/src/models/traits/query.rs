use crate::constants::MAX_PARALLEL_REQUESTS;
use crate::errors::NodecosmosError;
use crate::stream::MergedModelStream;
use charybdis::model::BaseModel;
use charybdis::stream::CharybdisModelStream;
use futures::stream::SelectAll;
use futures::{stream, StreamExt};
use std::future::Future;

pub trait ParallelChunksExecutor<M: BaseModel + 'static> {
    async fn exec_chunks_in_parallel(self) -> MergedModelStream<M>;
}

impl<I, M, F> ParallelChunksExecutor<M> for I
where
    M: BaseModel + 'static + Send,
    F: Future<Output = Result<CharybdisModelStream<M>, NodecosmosError>> + Send,
    I: IntoIterator<Item = F> + Send,
{
    async fn exec_chunks_in_parallel(self) -> MergedModelStream<M> {
        let streams = stream::iter(self).buffer_unordered(MAX_PARALLEL_REQUESTS);

        let mut select_all = SelectAll::new();

        streams
            .for_each(|result_stream| {
                match result_stream {
                    Ok(stream) => {
                        select_all.push(stream);
                        log::info!("Successfully executed a batch query and merged its stream.");
                    }
                    Err(e) => {
                        log::error!("Error executing batch query: {:?}", e);
                    }
                }
                futures::future::ready(())
            })
            .await;

        MergedModelStream {
            inner: select_all.boxed(),
        }
    }
}
