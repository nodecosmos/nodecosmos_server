use crate::actions::client_session::CurrentUser;
use crate::errors::NodecosmosError;
use crate::models::node::*;
use serde_json::json;

pub async fn auth_node_creation(
    parent: &Option<Node>,
    current_user: &CurrentUser,
) -> Result<(), NodecosmosError> {
    match parent {
        Some(parent) => {
            if can_edit_node(current_user, &parent) {
                Ok(())
            } else {
                Err(NodecosmosError::Unauthorized(json!({
                    "error": "Unauthorized",
                    "message": "Not authorized to create node for provided parent!"
                })))
            }
        }
        None => Ok(()),
    }
}

pub async fn auth_node_update(
    node: &Node,
    current_user: &CurrentUser,
) -> Result<(), NodecosmosError> {
    if can_edit_node(current_user, &node) {
        Ok(())
    } else {
        Err(NodecosmosError::Unauthorized(json!({
            "error": "Unauthorized",
            "message": "Not authorized to create node for provided parent!"
        })))
    }
}

fn can_edit_node(current_user: &CurrentUser, node: &Node) -> bool {
    let owner_id = node.owner_id.unwrap_or_default();
    if owner_id == current_user.id {
        return true; // Owner can edit
    }

    let editor_ids = node.editor_ids.as_ref();
    if editor_ids.is_some() && editor_ids.unwrap().contains(&current_user.id) {
        return true; // Editor can edit
    }

    false
}
