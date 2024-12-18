use crate::constants::{MAX_PARALLEL_REQUESTS, MAX_WHERE_IN_CHUNK_SIZE};
use crate::stream::MergedModelStream;
use charybdis::errors::CharybdisError;
use charybdis::model::BaseModel;
use charybdis::query::{CharybdisQuery, ModelStream};
use charybdis::stream::CharybdisModelStream;
use charybdis::types::Uuid;
use futures::stream::SelectAll;
use futures::{stream, StreamExt};
use scylla::CachingSession;
use scylla::_macro_internal::SerializeRow;
use std::collections::HashSet;
use std::fmt::Debug;
use std::future::Future;

pub trait ParallelChunksExecutor<M: BaseModel + 'static> {
    async fn exec_chunks_in_parallel(self) -> MergedModelStream<M>;
}

impl<M, E, F, I> ParallelChunksExecutor<M> for I
where
    M: BaseModel + 'static + Send,
    E: From<CharybdisError> + Debug,
    F: Future<Output = Result<CharybdisModelStream<M>, E>> + Send,
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

pub trait IdsVec {
    fn ids(&self) -> Vec<Uuid>;
}

impl IdsVec for Vec<Uuid> {
    fn ids(&self) -> Vec<Uuid> {
        self.clone()
    }
}

impl IdsVec for HashSet<Uuid> {
    fn ids(&self) -> Vec<Uuid> {
        self.iter().copied().collect()
    }
}

impl IdsVec for &[Uuid] {
    fn ids(&self) -> Vec<Uuid> {
        self.iter().copied().collect()
    }
}

/// Execute a `WHERE IN ?` query in chunks of `MAX_WHERE_IN_CHUNK_SIZE` in parallel.
pub trait WhereInChunksExec<'a> {
    async fn where_in_chunked_query<M, F, Val>(&self, db: &CachingSession, f: F) -> MergedModelStream<M>
    where
        M: BaseModel + 'static + Send,
        Val: SerializeRow + 'a + Send,
        F: Send + Fn(Vec<Uuid>) -> CharybdisQuery<'a, Val, M, ModelStream>;
}

impl<'a, I: IdsVec> WhereInChunksExec<'a> for I {
    async fn where_in_chunked_query<M, F, Val>(&self, db: &CachingSession, f: F) -> MergedModelStream<M>
    where
        M: BaseModel + 'static + Send,
        Val: SerializeRow + 'a,
        F: Send + Fn(Vec<Uuid>) -> CharybdisQuery<'a, Val, M, ModelStream>,
    {
        let ids = self.ids();

        let queries = ids
            .chunks(MAX_WHERE_IN_CHUNK_SIZE)
            .map(|chunk| f(chunk.to_vec()).execute(db));
        let streams = stream::iter(queries).buffer_unordered(MAX_PARALLEL_REQUESTS);

        let mut select_all = SelectAll::new();

        streams
            .for_each(|result_stream| {
                match result_stream {
                    Ok(stream) => {
                        select_all.push(stream);
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
