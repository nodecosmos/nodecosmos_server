use crate::errors::NodecosmosError;
use crate::models::user::{CurrentUser, User};
use serde_json::json;

pub async fn auth_user_update(user: &User, current_user: &CurrentUser) -> Result<(), NodecosmosError> {
    if user.id == current_user.id {
        return Ok(());
    }

    Err(NodecosmosError::Unauthorized(json!({
        "error": "Unauthorized",
        "message": "Not authorized to update user!"
    })))
}
