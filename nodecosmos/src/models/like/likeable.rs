use charybdis::types::Uuid;
use scylla::client::caching_session::CachingSession;

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::like::Like;

pub trait Likeable {
    async fn increment_like(data: &RequestData, like: &Like) -> Result<(), NodecosmosError>;
    async fn decrement_like(data: &RequestData, like: &Like) -> Result<(), NodecosmosError>;
    async fn like_count(db_session: &CachingSession, id: Uuid, branch_id: Uuid) -> Result<i64, NodecosmosError>;
}
