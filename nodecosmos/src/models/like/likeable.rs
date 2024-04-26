use charybdis::types::Uuid;
use scylla::CachingSession;

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;

pub trait Likeable {
    async fn increment_like(data: &RequestData, id: Uuid, branch_id: Uuid) -> Result<i64, NodecosmosError>;
    async fn decrement_like(data: &RequestData, id: Uuid, branch_id: Uuid) -> Result<i64, NodecosmosError>;
    async fn like_count(db_session: &CachingSession, id: Uuid, branch_id: Uuid) -> Result<i64, NodecosmosError>;
}
