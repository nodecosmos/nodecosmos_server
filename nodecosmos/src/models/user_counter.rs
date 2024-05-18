use charybdis::macros::charybdis_model;
use charybdis::types::{Counter, Uuid};

#[charybdis_model(
    table_name = user_counters,
    partition_keys = [id],
    clustering_keys = []
)]
#[derive(Default)]
pub struct UserCounter {
    pub id: Uuid,
    pub resend_token_count: Counter,
}
