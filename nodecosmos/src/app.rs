use std::sync::Arc;
use std::{env, fs};

use actix_cors::Cors;
use actix_session::config::PersistentSession;
use actix_session::storage::RedisActorSessionStore;
use actix_session::SessionMiddleware;
use actix_web::cookie::Key;
use actix_web::{cookie, http, web};
use deadpool_redis::Pool;
use elasticsearch::Elasticsearch;
use scylla::CachingSession;
use toml::Value;

use crate::api::data::RequestData;
use crate::models::node::Node;
use crate::models::traits::BuildIndex;
use crate::models::user::User;
use crate::resources::description_ws_pool::DescriptionWsPool;
use crate::resources::mailer::Mailer;
use crate::resources::resource::Resource;
use crate::resources::resource_locker::ResourceLocker;
use crate::resources::sse_broadcast::SseBroadcast;
use crate::tasks;

#[derive(Clone)]
pub struct App {
    pub config: Value,
    pub db_session: Arc<CachingSession>,
    pub elastic_client: Arc<Elasticsearch>,
    pub redis_pool: Arc<Pool>,
    pub s3_bucket: String,
    pub s3_client: Arc<aws_sdk_s3::Client>,
    pub ses_client: Arc<aws_sdk_ses::Client>,
    pub resource_locker: Arc<ResourceLocker>,
    pub description_ws_pool: Arc<DescriptionWsPool>,
    pub sse_broadcast: Arc<SseBroadcast>,
    pub mailer: Arc<Mailer>,
}

impl App {
    pub async fn new() -> Self {
        dotenv::dotenv().ok();

        let env = env::var("ENV").expect("ENV must be set");
        let config_file = format!("config.{}.toml", env);
        let contents = fs::read_to_string(config_file).expect("Unable to read file");
        let config = contents.parse::<Value>().expect("Unable to parse TOML");
        let s3_bucket = config["aws"]["bucket"].as_str().expect("Missing bucket").to_string();

        let s3_client = aws_sdk_s3::Client::init_resource(()).await;
        let ses_client = aws_sdk_ses::Client::init_resource(()).await;
        let db_session = CachingSession::init_resource(&config).await;
        let elastic_client = Elasticsearch::init_resource(&config).await;
        let redis_pool = Pool::init_resource(&config).await;
        let mailer = Mailer::init_resource((ses_client.clone(), &config)).await;

        // app data
        let resource_locker = ResourceLocker::init_resource(&redis_pool).await;
        let description_ws_pool = Arc::new(DescriptionWsPool::init_resource(()).await);
        let sse_broadcast = Arc::new(SseBroadcast::init_resource(()).await);

        Self {
            config,
            db_session: Arc::new(db_session),
            elastic_client: Arc::new(elastic_client),
            redis_pool: Arc::new(redis_pool),
            mailer: Arc::new(mailer),
            s3_bucket,
            s3_client: Arc::new(s3_client),
            ses_client: Arc::new(ses_client),
            resource_locker: Arc::new(resource_locker),
            description_ws_pool,
            sse_broadcast,
        }
    }

    /// Init processes that need to be run on startup
    pub async fn init(&self) {
        // init logger
        env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

        // init elastic
        Node::build_index(&self.elastic_client).await;
        User::build_index(&self.elastic_client).await;

        let data = RequestData {
            app: web::Data::new(self.clone()),
            current_user: Default::default(),
        };

        tasks::recovery_task(data).await;
        tasks::cleanup_rooms_task(self.sse_broadcast.clone()).await;
    }

    pub fn cors(&self) -> Cors {
        let allowed_origin = self.config["allowed_origin"]
            .as_str()
            .expect("Missing allowed_origin")
            .to_string();

        Cors::default()
            .allowed_origin(allowed_origin.as_str())
            .supports_credentials()
            .allowed_methods(vec!["GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"])
            .allowed_headers(vec![
                http::header::AUTHORIZATION,
                http::header::ACCEPT,
                http::header::ORIGIN,
                http::header::USER_AGENT,
                http::header::DNT,
                http::header::CONTENT_TYPE,
                http::header::ACCESS_CONTROL_ALLOW_CREDENTIALS,
            ])
            .expose_headers(vec![http::header::LOCATION, http::header::ACCESS_CONTROL_ALLOW_ORIGIN])
            .max_age(86400)
    }

    pub fn port(&self) -> u16 {
        self.config["port"].as_integer().expect("Missing port") as u16
    }

    pub fn session_middleware(&self) -> SessionMiddleware<RedisActorSessionStore> {
        let secret_key = self.secret_key();
        let redis_actor_session_store = self.redis_actor_session_store();
        let expiration = self.config["session_expiration_in_days"]
            .as_integer()
            .expect("Missing session_expiration");
        let ttl = PersistentSession::default().session_ttl(cookie::time::Duration::days(expiration));

        SessionMiddleware::builder(redis_actor_session_store, secret_key)
            .session_lifecycle(ttl)
            .cookie_secure(self.config["ssl"].as_bool().expect("Missing ssl"))
            .build()
    }

    fn secret_key(&self) -> Key {
        let secret_key = self.config["secret_key"]
            .as_str()
            .expect("Missing secret_key")
            .to_string();

        Key::from(secret_key.as_ref())
    }

    fn redis_actor_session_store(&self) -> RedisActorSessionStore {
        let redis_host = self.config["redis"]["host"].as_str().expect("Missing redis url");

        RedisActorSessionStore::new(redis_host)
    }
}
