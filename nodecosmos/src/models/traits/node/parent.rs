use scylla::CachingSession;

use crate::errors::NodecosmosError;
use crate::models::node::Node;

/// Implemented by `NodeParent` derive  `#[derive(NodeParent)]` with exception of `BaseNode` to avoid infinite recursion
pub trait Parent {
    async fn parent(&mut self, db_session: &CachingSession) -> Result<Option<&mut Box<Node>>, NodecosmosError>;
    async fn branch_parent(&mut self, db_session: &CachingSession) -> Result<Option<&mut Box<Node>>, NodecosmosError>;
}
