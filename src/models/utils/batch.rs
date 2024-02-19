use crate::models::branch::Branch;
use crate::utils::logger::log_fatal;
use charybdis::batch::CharybdisModelBatch;
use charybdis::model::Model;
use charybdis::types::Uuid;
use charybdis::SerializeRow;

pub fn append_statement_or_log_fatal(
    batch: &mut CharybdisModelBatch<(Vec<Uuid>, Uuid), Branch>,
    query: &str,
    params: (Vec<Uuid>, Uuid),
) -> Result<(), ()> {
    batch.append_statement(query, params).map_err(|err| {
        log_fatal(format!("Failed to append statement to batch: {}", err));
        ()
    })
}
