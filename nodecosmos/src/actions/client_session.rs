use crate::models::user::*;

use crate::errors::NodecosmosError;
use actix_session::Session;

pub fn set_current_user(
    client_session: &Session,
    user: User,
) -> Result<CurrentUser, NodecosmosError> {
    let current_user = CurrentUser {
        id: user.id,
        username: user.username.clone(),
        email: user.email.clone(),
        email_verified: user.email_verified,
    };

    client_session
        .insert("current_user", &current_user)
        .map_err(|e| {
            NodecosmosError::ClientSessionError(format!("Could not set current user. {}", e))
        })?;

    Ok(current_user)
}

pub fn get_current_user(client_session: &Session) -> Result<CurrentUser, NodecosmosError> {
    if let Some(current_user) = client_session
        .get::<CurrentUser>("current_user")
        .map_err(|e| {
            NodecosmosError::ClientSessionError(format!("Could not get current user. {}", e))
        })?
    {
        Ok(current_user)
    } else {
        Err(NodecosmosError::ClientSessionError(
            "Could not get current user.".to_string(),
        ))
    }
}
