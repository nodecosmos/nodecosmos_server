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
use serde::Deserialize;
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

#[derive(Clone, Deserialize)]
pub struct ScyllaConfig {
    pub hosts: [String; 3],
    pub keyspace: String,
    pub username: String,
    pub password: String,
}

#[derive(Clone, Deserialize)]
pub struct ElasticConfig {
    pub host: String,
}

#[derive(Clone, Deserialize)]
pub struct AwsConfig {
    pub bucket: String,
}

#[derive(Clone, Deserialize)]
pub struct RedisConfig {
    pub host: String,
    pub url: String,
}

#[derive(Clone, Deserialize)]
pub struct Config {
    pub port: u16,
    pub allowed_origin: String,
    pub client_url: String,
    pub secret_key: String,
    pub session_expiration_in_days: i64,
    pub ssl: bool,
    pub scylla: ScyllaConfig,
    pub redis: RedisConfig,
    pub elasticsearch: ElasticConfig,
    pub aws: AwsConfig,
}

#[derive(Clone)]
pub struct App {
    pub config: Config,
    pub recaptcha_enabled: bool,
    pub recaptcha_secret: String,
    pub db_session: Arc<CachingSession>,
    pub elastic_client: Arc<Elasticsearch>,
    pub s3_client: Arc<aws_sdk_s3::Client>,
    pub resource_locker: Arc<ResourceLocker>,
    pub description_ws_pool: Arc<DescriptionWsPool>,
    pub sse_broadcast: Arc<SseBroadcast>,
    pub mailer: Arc<Mailer>,
}

impl App {
    pub async fn new() -> Self {
        dotenv::dotenv().ok();

        let env = env::var("ENV").expect("ENV must be set");
        let recaptcha_enabled = env::var("RECAPTCHA_ENABLED").expect("RECAPTCHA_ENABLED must be set");
        let recaptcha_secret = env::var("RECAPTCHA_SECRET").expect("RECAPTCHA_SECRET must be set");
        let config_file = format!("config.{}.toml", env);
        let contents = fs::read_to_string(config_file).expect("Unable to read file");
        let config_val = contents.parse::<Value>().expect("Unable to parse TOML");
        let config = config_val.try_into::<Config>().expect("Unable to parse config");

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
            recaptcha_enabled: recaptcha_enabled == "true",
            recaptcha_secret,
            db_session: Arc::new(db_session),
            elastic_client: Arc::new(elastic_client),
            mailer: Arc::new(mailer),
            s3_client: Arc::new(s3_client),
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
        Cors::default()
            .allowed_origin(self.config.allowed_origin.as_str())
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
        self.config.port
    }

    pub fn session_middleware(&self) -> SessionMiddleware<RedisActorSessionStore> {
        let secret_key = Key::from(self.config.secret_key.as_ref());
        let redis_actor_session_store = self.redis_actor_session_store();
        let expiration = self.config.session_expiration_in_days;
        let ttl = PersistentSession::default().session_ttl(cookie::time::Duration::days(expiration));

        SessionMiddleware::builder(redis_actor_session_store, secret_key)
            .session_lifecycle(ttl)
            .cookie_secure(self.config.ssl)
            .build()
    }

    fn redis_actor_session_store(&self) -> RedisActorSessionStore {
        RedisActorSessionStore::new(&self.config.redis.host)
    }
}
