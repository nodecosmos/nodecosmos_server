use crate::errors::NodecosmosError;
use crate::models::user::{CurrentUser, User};
use actix_session::{Session, SessionExt};
use actix_web::dev::Payload;
use actix_web::{FromRequest, HttpRequest};
use futures::future::{ready, Ready};
use serde_json::json;

pub fn set_current_user(client_session: &Session, user: &User) -> Result<CurrentUser, NodecosmosError> {
    let current_user = CurrentUser {
        id: user.id,
        first_name: user.first_name.clone(),
        last_name: user.last_name.clone(),
        username: user.username.clone(),
        email: user.email.clone(),
        is_confirmed: user.is_confirmed,
        is_blocked: user.is_blocked,
    };

    client_session
        .insert("current_user", &current_user)
        .map_err(|e| NodecosmosError::ClientSessionError(format!("Could not set current user. {}", e)))?;

    Ok(current_user)
}

pub fn get_current_user(client_session: &Session) -> Option<CurrentUser> {
    let current_user = client_session
        .get::<CurrentUser>("current_user")
        .map_err(|e| NodecosmosError::ClientSessionError(format!("Could not get current user. {}", e)));

    match current_user {
        Ok(Some(user)) => {
            if user.is_blocked {
                None
            } else {
                Some(user)
            }
        }
        _ => None,
    }
}

impl FromRequest for CurrentUser {
    type Error = NodecosmosError;
    type Future = Ready<Result<CurrentUser, NodecosmosError>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        let client_session = req.get_session();
        match get_current_user(&client_session) {
            Some(user) => ready(Ok(user)),
            None => {
                let error_response = NodecosmosError::Unauthorized(json!({
                    "error": "Unauthorized! You must be logged in to perform this action",
                    "message": "You must be logged in to perform this action!"
                }));
                ready(Err(error_response))
            }
        }
    }
}

pub struct OptCurrentUser(pub Option<CurrentUser>);

impl FromRequest for OptCurrentUser {
    type Error = NodecosmosError;
    type Future = Ready<Result<Self, NodecosmosError>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        let client_session = req.get_session();
        match get_current_user(&client_session) {
            Some(user) => ready(Ok(OptCurrentUser(Some(user)))),
            None => ready(Ok(OptCurrentUser(None))),
        }
    }
}
