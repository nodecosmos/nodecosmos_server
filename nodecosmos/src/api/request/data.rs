use std::future::{ready, Ready};
use std::sync::Arc;

use actix_session::SessionExt;
use actix_web::dev::Payload;
use actix_web::{web, FromRequest, HttpRequest};
use charybdis::types::Uuid;
use elasticsearch::Elasticsearch;
use scylla::CachingSession;
use serde_json::json;

use crate::api::current_user::get_current_user;
use crate::app::App;
use crate::errors::NodecosmosError;
use crate::models::user::CurrentUser;
use crate::resources::description_ws_pool::DescriptionWsPool;
use crate::resources::resource_locker::ResourceLocker;
use crate::resources::sse_broadcast::SseBroadcast;

/// It contains the data that is required by node API endpoints and node callbacks.
#[derive(Clone)]
pub struct RequestData {
    pub app: web::Data<App>,
    pub current_user: CurrentUser,
}

impl RequestData {
    pub fn new(app: web::Data<App>, current_user: CurrentUser) -> Self {
        Self { app, current_user }
    }

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

    pub fn description_ws_pool(&self) -> Arc<DescriptionWsPool> {
        self.app.description_ws_pool.clone()
    }

    pub fn sse_broadcast(&self) -> Arc<SseBroadcast> {
        self.app.sse_broadcast.clone()
    }

    pub fn current_user_id(&self) -> Uuid {
        self.current_user.id
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
                        let data = RequestData {
                            app: web::Data::clone(app),
                            current_user,
                        };

                        ready(Ok(data))
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
