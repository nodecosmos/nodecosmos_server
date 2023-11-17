use crate::api::current_user::get_current_user;
use crate::app::App;
use crate::errors::NodecosmosError;
use crate::models::user::CurrentUser;
use actix_session::SessionExt;
use actix_web::dev::Payload;
use actix_web::{web, FromRequest, HttpRequest};
use serde_json::json;
use std::future::{ready, Ready};

/// It contains the data that is required by node API endpoints and node callbacks.
#[derive(Clone)]
pub struct RequestData {
    pub app: web::Data<App>,
    pub current_user: CurrentUser,
}

impl FromRequest for RequestData {
    type Error = NodecosmosError;
    type Future = Ready<Result<RequestData, NodecosmosError>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        let client_session = req.get_session();

        return match get_current_user(&client_session) {
            Some(current_user) => {
                let app = req.app_data::<web::Data<App>>();

                match app {
                    Some(app) => {
                        let ext = RequestData {
                            app: web::Data::clone(app),
                            current_user,
                        };

                        ready(Ok(ext))
                    }
                    None => {
                        let err = NodecosmosError::InternalServerError("Could not get app data".to_string());

                        ready(Err(err))
                    }
                }
            }
            None => {
                let error_response = NodecosmosError::Unauthorized(json!({
                    "error": "Unauthorized! You must be logged in to perform this action",
                    "message": "You must be logged in to perform this action!"
                }));
                ready(Err(error_response))
            }
        };
    }
}
