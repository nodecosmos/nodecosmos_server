use crate::constants::MAX_PARALLEL_REQUESTS;
use crate::errors::NodecosmosError;
use futures::future::try_join_all;
use std::future::Future;
use std::iter::Iterator;

pub async fn process_in_chunks<I, T, F, Fut, Res>(items: I, mut process_fn: F) -> Result<(), NodecosmosError>
where
    I: IntoIterator<Item = T>,
    F: FnMut(T) -> Fut,
    Fut: Future<Output = Result<Res, NodecosmosError>>,
{
    let mut item_iter = items.into_iter();

    loop {
        let chunk: Vec<_> = item_iter.by_ref().take(MAX_PARALLEL_REQUESTS).collect();

        if chunk.is_empty() {
            break;
        }

        let futures = chunk.into_iter().map(&mut process_fn);
        try_join_all(futures).await?;
    }

    Ok(())
}
