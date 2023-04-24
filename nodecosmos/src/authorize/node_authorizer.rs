use crate::errors::NodecosmosError;
use crate::models::node::Node;
use crate::models::user::CurrentUser;

use actix_session::Session;
use scylla::CachingSession;

pub async fn auth_node_creation(
    node: &Node,
    parent: &DescendableNode,
    db_session: &CachingSession,
    current_user: &CurrentUser,
) -> Result<(), NodecosmosError> {
    Ok(())
}

pub async fn auth_node_update(
    node: &Node,
    db_session: &CachingSession,
    current_user: &CurrentUser,
) -> Result<(), NodecosmosError> {
    Ok(())
}
