use crate::actions::client_session::{CurrentUser, OptCurrentUser};
use crate::errors::NodecosmosError;
use crate::models::materialized_views::auth_node_by_id::{
    find_auth_node_by_id_query, AuthNodeById,
};
use crate::models::node::*;
use charybdis::{AsNative, Find, New, Uuid};
use scylla::CachingSession;
use serde_json::json;

pub async fn auth_node_access(
    node: &BaseNode,
    opt_current_user: OptCurrentUser,
) -> Result<(), NodecosmosError> {
    if node.is_public.is_some_and(|is_public| is_public) {
        Ok(())
    } else if let Some(current_user) = opt_current_user.0 {
        if can_edit_node(&current_user, &node.as_native()) {
            Ok(())
        } else {
            Err(NodecosmosError::Unauthorized(json!({
                "error": "Unauthorized! Make sure you are logged in.",
                "message": "Not authorized to access node!"
            })))
        }
    } else {
        Err(NodecosmosError::Unauthorized(json!({
            "error": "Unauthorized! Make sure you are logged in.",
            "message": "Not authorized to access node!"
        })))
    }
}

pub async fn auth_node_creation(
    parent: &Option<Node>,
    current_user: &CurrentUser,
) -> Result<(), NodecosmosError> {
    if let Some(parent) = parent {
        if can_edit_node(current_user, parent) {
            Ok(())
        } else {
            Err(NodecosmosError::Unauthorized(json!({
                "error": "Unauthorized! Make sure you are logged in.",
                "message": "Not authorized to create node for provided parent!"
            })))
        }
    } else {
        Ok(())
    }
}

pub async fn auth_node_update_by_id(
    id: &Uuid,
    db_session: &CachingSession,
    current_user: &CurrentUser,
) -> Result<(), NodecosmosError> {
    println!("auth_node_update_by_id");
    let auth_node =
        AuthNodeById::find_one(db_session, find_auth_node_by_id_query!("id = ?"), (id,)).await?;

    println!("auth_node_update_by_id: {:?}", auth_node);

    let mut node = Node::new();
    node.id = auth_node.id;
    node.root_id = auth_node.root_id;
    node.editor_ids = auth_node.editor_ids;
    node.owner_id = auth_node.owner_id;

    auth_node_update(&node, current_user).await?;

    Ok(())
}

pub async fn auth_node_update(
    node: &Node,
    current_user: &CurrentUser,
) -> Result<(), NodecosmosError> {
    if can_edit_node(current_user, node) {
        Ok(())
    } else {
        Err(NodecosmosError::Unauthorized(json!({
            "error": "Unauthorized! Make sure you are logged in.",
            "message": "Not authorized to update node!"
        })))
    }
}

pub fn can_edit_node(current_user: &CurrentUser, node: &Node) -> bool {
    if node.owner_id == Some(current_user.id) {
        return true; // Owner can edit
    }

    let editor_ids = node.editor_ids.as_ref();
    if editor_ids.map_or(false, |ids| ids.contains(&current_user.id)) {
        return true; // Editor can edit
    }

    false
}
