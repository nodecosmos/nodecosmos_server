use crate::actions::client_session::{CurrentUser, OptCurrentUser};
use crate::errors::NodecosmosError;
use crate::models::node::*;
use charybdis::AsNative;
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
    if editor_ids.is_some() && editor_ids.unwrap().contains(&current_user.id) {
        return true; // Editor can edit
    }

    false
}
