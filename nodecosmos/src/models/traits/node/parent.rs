use scylla::CachingSession;

use crate::errors::NodecosmosError;
use crate::models::node::BaseNode;

/// Implemented by `NodeParent` derive  `#[derive(NodeParent)]` with exception of `BaseNode` to avoid infinite recursion
pub trait Parent {
    async fn parent(&mut self, db_session: &CachingSession) -> Result<Option<&mut BaseNode>, NodecosmosError>;
    async fn branch_parent(&mut self, db_session: &CachingSession) -> Result<Option<&mut BaseNode>, NodecosmosError>;
}

impl Parent for BaseNode {
    async fn parent(&mut self, _db_session: &CachingSession) -> Result<Option<&mut BaseNode>, NodecosmosError> {
        return Err(NodecosmosError::Forbidden("Not implemented".to_string()));
    }

    async fn branch_parent(&mut self, _db_session: &CachingSession) -> Result<Option<&mut BaseNode>, NodecosmosError> {
        return Err(NodecosmosError::Forbidden("Not implemented".to_string()));
    }
}
