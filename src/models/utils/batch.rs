use crate::utils::logger::log_fatal;
use charybdis::batch::CharybdisModelBatch;
use charybdis::types::Uuid;

pub fn append_statement_or_log_fatal(
    batch: &mut CharybdisModelBatch,
    query: &str,
    params: (Vec<Uuid>, Uuid),
) -> Result<(), ()> {
    batch.append_statement(query, params).map_err(|err| {
        log_fatal(format!("Failed to append statement to batch: {}", err));
        ()
    })
}
