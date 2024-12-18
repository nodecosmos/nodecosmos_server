use crate::constants::{MAX_PARALLEL_REQUESTS, MAX_WHERE_IN_CHUNK_SIZE};
use crate::stream::MergedModelStream;
use charybdis::model::BaseModel;
use charybdis::query::{CharybdisQuery, ModelStream};
use charybdis::types::Uuid;
use futures::stream::SelectAll;
use futures::{stream, StreamExt};
use scylla::CachingSession;
use scylla::_macro_internal::SerializeRow;
use std::collections::HashSet;

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
        self.to_vec()
    }
}

/// Execute a `WHERE IN ?` query in chunks of `MAX_WHERE_IN_CHUNK_SIZE` in parallel.
pub trait WhereInChunksExec<'a> {
    async fn where_in_chunked_query<M, F, Val>(&self, db: &CachingSession, f: F) -> MergedModelStream<M>
    where
        M: BaseModel + 'static + Send,
        Val: SerializeRow + 'a,
        F: Fn(Vec<Uuid>) -> CharybdisQuery<'a, Val, M, ModelStream>;
}

impl<'a, I: IdsVec> WhereInChunksExec<'a> for I {
    async fn where_in_chunked_query<M, F, Val>(&self, db: &CachingSession, f: F) -> MergedModelStream<M>
    where
        M: BaseModel + 'static + Send,
        Val: SerializeRow + 'a,
        F: Fn(Vec<Uuid>) -> CharybdisQuery<'a, Val, M, ModelStream>,
    {
        let ids = self.ids();
        let mut queries = vec![];

        for id_chunk in ids.chunks(MAX_WHERE_IN_CHUNK_SIZE) {
            queries.push(f(id_chunk.to_vec()).execute(db));
        }

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
