use crate::models::user::*;
use charybdis::prelude::CharybdisError;

use actix_session::Session;

pub fn set_current_user(
    client_session: &Session,
    user: User,
) -> Result<CurrentUser, CharybdisError> {
    let current_user = CurrentUser {
        id: user.id,
        username: user.username.clone(),
        email: user.email.clone(),
        email_verified: user.email_verified,
    };

    client_session
        .insert("current_user", &current_user)
        .map_err(|e| CharybdisError::SessionError(format!("Could not set current user. {}", e)))?;

    Ok(current_user)
}

pub fn get_current_user(client_session: &Session) -> Result<CurrentUser, CharybdisError> {
    if let Some(current_user) = client_session
        .get::<CurrentUser>("current_user")
        .map_err(|e| CharybdisError::SessionError(format!("Could not get current user. {}", e)))?
    {
        println!("current_user: {:?}", current_user);

        Ok(current_user)
    } else {
        Err(CharybdisError::SessionError(
            "Could not get current user.".to_string(),
        ))
    }
}
