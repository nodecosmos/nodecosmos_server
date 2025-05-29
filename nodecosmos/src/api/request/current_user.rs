use std::future::{ready, Ready};

use actix_session::{Session, SessionExt};
use actix_web::dev::Payload;
use actix_web::{FromRequest, HttpRequest};
use log::error;
use scylla::client::caching_session::CachingSession;

use crate::errors::NodecosmosError;
use crate::models::user::CurrentUser;

impl FromRequest for CurrentUser {
    type Error = NodecosmosError;
    type Future = Ready<Result<CurrentUser, NodecosmosError>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        let client_session = req.get_session();
        match get_current_user(&client_session) {
            Some(user) => ready(Ok(user)),
            None => {
                let error_response = NodecosmosError::Unauthorized("You must be logged in to perform this action!");
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

pub fn set_current_user(client_session: &Session, current_user: &CurrentUser) -> Result<(), NodecosmosError> {
    client_session.insert("current_user", current_user).map_err(|e| {
        error!("Could not set current user. {}", e);

        NodecosmosError::ClientSessionError("Could not set current user.".to_string())
    })?;

    Ok(())
}

pub fn remove_current_user(client_session: &Session) {
    client_session.remove("current_user");
}

pub fn get_current_user(client_session: &Session) -> Option<CurrentUser> {
    let current_user = client_session
        .get::<CurrentUser>("current_user")
        .map_err(|e| error!("Could not get current user. {}", e));

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

pub async fn refresh_current_user(
    client_session: &Session,
    db_session: &CachingSession,
) -> Result<(), NodecosmosError> {
    let current_user = get_current_user(client_session);

    match current_user {
        Some(user) => {
            let user = CurrentUser::find_by_id(user.id).execute(db_session).await?;
            set_current_user(client_session, &user)
        }
        None => Err(NodecosmosError::InternalServerError("Failed to find user".to_string())),
    }
}
