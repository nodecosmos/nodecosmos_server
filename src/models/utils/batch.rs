use crate::models::branch::Branch;
use charybdis::batch::CharybdisModelBatch;
use charybdis::types::Uuid;
use log::error;

pub fn append_statement_or_log_fatal(
    batch: &mut CharybdisModelBatch<(Vec<Uuid>, Uuid), Branch>,
    query: &str,
    params: (Vec<Uuid>, Uuid),
) -> Result<(), ()> {
    batch
        .append_statement(query, params)
        .map_err(|err| error!("Failed to append statement to batch: {}", err))
}
