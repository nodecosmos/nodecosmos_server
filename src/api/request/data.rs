use crate::api::current_user::get_current_user;
use crate::app::App;
use crate::errors::NodecosmosError;
use crate::models::user::CurrentUser;
use crate::services::resource_locker::ResourceLocker;
use actix_session::SessionExt;
use actix_web::dev::Payload;
use actix_web::{web, FromRequest, HttpRequest};
use charybdis::types::Uuid;
use elasticsearch::Elasticsearch;
use scylla::CachingSession;
use serde_json::json;
use std::future::{ready, Ready};

/// It contains the data that is required by node API endpoints and node callbacks.
#[derive(Clone)]
pub struct RequestData {
    pub app: web::Data<App>,
    pub current_user: CurrentUser,
}

impl RequestData {
    pub fn db_session(&self) -> &CachingSession {
        &self.app.db_session
    }

    pub fn elastic_client(&self) -> &Elasticsearch {
        &self.app.elastic_client
    }

    pub fn s3_client(&self) -> &aws_sdk_s3::Client {
        &self.app.s3_client
    }

    pub fn s3_bucket(&self) -> &String {
        &self.app.s3_bucket
    }

    pub fn resource_locker(&self) -> &ResourceLocker {
        &self.app.resource_locker
    }

    pub fn current_user_id(&self) -> Uuid {
        self.current_user.id
    }
}

impl RequestData {
    pub fn new(app: web::Data<App>, current_user: CurrentUser) -> Self {
        Self { app, current_user }
    }
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
                        let req_data = RequestData {
                            app: web::Data::clone(app),
                            current_user,
                        };

                        ready(Ok(req_data))
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
