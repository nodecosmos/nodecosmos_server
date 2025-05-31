use std::future::{ready, Ready};
use std::sync::Arc;

use crate::api::current_user::get_current_user;
use crate::app::{App, StripeCfg};
use crate::errors::NodecosmosError;
use crate::models::user::CurrentUser;
use crate::resources::resource_locker::ResourceLocker;
use crate::resources::sse_broadcast::SseBroadcast;
use crate::resources::ws_broadcast::WsBroadcast;
use actix_session::SessionExt;
use actix_web::dev::Payload;
use actix_web::{web, FromRequest, HttpRequest};
use elasticsearch::Elasticsearch;
use scylla::client::caching_session::CachingSession;

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

    pub fn s3_client_arc(&self) -> Arc<aws_sdk_s3::Client> {
        self.app.s3_client.clone()
    }

    pub fn s3_bucket(&self) -> &String {
        &self.app.config.aws.bucket
    }

    pub fn mailer(&self) -> &crate::resources::mailer::Mailer {
        &self.app.mailer
    }

    pub fn resource_locker(&self) -> &ResourceLocker {
        &self.app.resource_locker
    }

    pub fn ws_broadcast(&self) -> Arc<WsBroadcast> {
        self.app.ws_broadcast.clone()
    }

    pub fn sse_broadcast(&self) -> Arc<SseBroadcast> {
        self.app.sse_broadcast.clone()
    }

    pub async fn redis_connection(
        &self,
    ) -> Result<deadpool::managed::Object<crate::resources::resource::RedisClusterManager>, NodecosmosError> {
        let conn = self.app.redis_pool.get().await?;

        Ok(conn)
    }

    pub fn stripe_cfg(&self) -> &Option<StripeCfg> {
        &self.app.stripe_cfg
    }
}

impl FromRequest for RequestData {
    type Error = NodecosmosError;
    type Future = Ready<Result<RequestData, NodecosmosError>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        let client_session = req.get_session();

        match get_current_user(&client_session) {
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
                let error_response = NodecosmosError::Unauthorized("You must be logged in to perform this action!");
                ready(Err(error_response))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use charybdis::types::Uuid;

    impl RequestData {
        pub async fn new(current_user: Option<CurrentUser>) -> Self {
            dotenv::dotenv().ok();

            let crate_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            let project_root = crate_root.parent().expect("Could not find project root");

            std::env::set_current_dir(project_root).expect("Could not set current directory to project root");
            let current_user = current_user.unwrap_or(CurrentUser {
                id: Uuid::new_v4(),
                email: "test@email.com".to_string(),
                profile_image_filename: None,
                profile_image_url: None,
                is_confirmed: true,
                first_name: "Test".to_string(),
                last_name: "User".to_string(),
                username: "testuser".to_string(),
                is_blocked: false,
            });

            let app = App::new().await.expect("Could not create app");
            RequestData {
                app: web::Data::new(app),
                current_user,
            }
        }
    }
}
