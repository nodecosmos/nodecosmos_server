use actix_cors::Cors;
use actix_session::config::PersistentSession;
use actix_session::SessionMiddleware;
use actix_web::cookie::Key;
use actix_web::{http, web};
use elasticsearch::Elasticsearch;
use scylla::client::caching_session::CachingSession;
use serde::Deserialize;
use std::sync::Arc;
use std::{env, fs};
use toml::Value;

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::node::Node;
use crate::models::traits::ElasticIndex;
use crate::models::user::User;
use crate::resources::email_client::TlsMode;
use crate::resources::mailer::Mailer;
use crate::resources::resource::{RedisClusterManager, Resource};
use crate::resources::resource_locker::ResourceLocker;
use crate::resources::sse_broadcast::SseBroadcast;
use crate::resources::ws_broadcast::WsBroadcast;
use crate::session_store::RedisClusterSessionStore;
use crate::tasks;

#[derive(Clone, Deserialize)]
pub struct ScyllaConfig {
    pub hosts: Vec<String>,
    pub keyspace: String,
    pub ca: Option<String>,
    pub cert: Option<String>,
    pub key: Option<String>,
}

#[derive(Clone, Deserialize)]
pub struct ElasticConfig {
    pub hosts: Vec<String>,
    pub ca: Option<String>,
    pub cert: Option<String>,
    pub key: Option<String>,
}

#[derive(Clone, Deserialize)]
pub struct AwsConfig {
    pub bucket: String,
}

#[derive(Clone, Deserialize)]
pub struct RedisConfig {
    pub urls: Vec<String>,

    #[serde(default)]
    pub replicas: u8,

    pub ca: Option<String>,
    pub cert: Option<String>,
    pub key: Option<String>,
}

#[derive(Clone, Deserialize)]
pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub tls_mode: TlsMode,
    pub username: String,
    pub password: String,
    pub from_name: Option<String>,
    pub from_email: String,
}

#[derive(Clone, Deserialize)]
pub struct StripeCfg {
    pub secret_key: String,
    pub price_id: String,
    pub webhook_secret_key: String,
}

#[derive(Clone, Deserialize)]
pub struct GoogleCfg {
    pub client_id: String,
    pub client_secret: String,
}

#[allow(unused)]
#[derive(Clone, Deserialize)]
pub struct Config {
    pub port: u16,
    pub allowed_origin_1: String,
    pub allowed_origin_2: Option<String>,
    pub client_url: String,
    pub session_expiration_in_days: i64,
    pub ssl: bool,
    pub scylla: ScyllaConfig,
    pub redis: RedisConfig,
    pub elasticsearch: ElasticConfig,
    pub aws: AwsConfig,
    pub smtp: Option<SmtpConfig>,
    pub ca: Option<String>,
    pub cert: Option<String>,
    pub key: Option<String>,
    pub region: Option<String>,
}

type RedisClients = Vec<redis::Client>;
pub type RedisClusterManagerPool = deadpool::managed::Pool<RedisClusterManager>;

#[derive(Clone)]
pub struct App {
    pub config: Config,
    pub stripe_cfg: Option<StripeCfg>,
    pub google_cfg: Option<GoogleCfg>,
    pub recaptcha_enabled: bool,
    pub recaptcha_secret: String,
    pub db_session: Arc<CachingSession>,
    pub elastic_client: Arc<Elasticsearch>,
    pub s3_client: Arc<aws_sdk_s3::Client>,
    pub resource_locker: Arc<ResourceLocker>,
    pub ws_broadcast: Arc<WsBroadcast>,
    pub sse_broadcast: Arc<SseBroadcast>,
    pub redis_pool: RedisClusterManagerPool,
    pub redis_clients: RedisClients,
    pub mailer: Arc<Mailer>,
    pub secret_key: String,
}

impl App {
    pub async fn new() -> Result<Self, NodecosmosError> {
        let secret_key = env::var("SECRET_KEY").unwrap_or_default();
        let config_file = env::var("CONFIG_FILE").map_err(|e| NodecosmosError::ConfigError(e.to_string()))?;
        let contents = fs::read_to_string(config_file).map_err(|e| NodecosmosError::ConfigError(e.to_string()))?;
        let config_val = contents
            .parse::<Value>()
            .map_err(|e| NodecosmosError::ConfigError(e.to_string()))?;
        let config = config_val
            .try_into::<Config>()
            .map_err(|e| NodecosmosError::ConfigError(e.to_string()))?;
        let recaptcha_enabled = env::var("RECAPTCHA_ENABLED").unwrap_or_default() == "true";
        let recaptcha_secret = env::var("RECAPTCHA_SECRET").unwrap_or_default();
        let s3_client = aws_sdk_s3::Client::init_resource(()).await;
        let db_session = CachingSession::init_resource(&config).await;
        let elastic_client = Elasticsearch::init_resource(&config).await;
        let redis_pool = RedisClusterManagerPool::init_resource(&config).await;
        let redis_clients: RedisClients = RedisClients::init_resource(&config).await;

        // app
        let resource_locker = ResourceLocker::init_resource((&redis_pool, config.redis.replicas)).await;
        let ws_broadcast = WsBroadcast::init_resource(()).await;
        let sse_broadcast = SseBroadcast::init_resource(()).await;
        let mailer = Mailer::init_resource(&config).await;

        let stripe_secret_key = env::var("STRIPE_SECRET_KEY").ok();
        let stripe_price_id = env::var("STRIPE_PRICE_ID").ok();
        let webhook_secret_key = env::var("STRIPE_WEBHOOK_SECRET").ok();
        let stripe_cfg = if let (Some(secret_key), Some(price_id), Some(webhook_secret_key)) =
            (stripe_secret_key, stripe_price_id, webhook_secret_key)
        {
            Some(StripeCfg {
                secret_key,
                price_id,
                webhook_secret_key,
            })
        } else {
            None
        };

        let google_client_id = env::var("GOOGLE_CLIENT_ID").ok();
        let google_client_secret = env::var("GOOGLE_CLIENT_SECRET").ok();

        let google_cfg = if let (Some(client_id), Some(client_secret)) = (google_client_id, google_client_secret) {
            Some(GoogleCfg {
                client_id,
                client_secret,
            })
        } else {
            None
        };

        Ok(Self {
            config,
            stripe_cfg,
            google_cfg,
            recaptcha_enabled,
            recaptcha_secret,
            db_session: Arc::new(db_session),
            elastic_client: Arc::new(elastic_client),
            s3_client: Arc::new(s3_client),
            redis_pool,
            redis_clients,
            // app
            resource_locker: Arc::new(resource_locker),
            ws_broadcast: Arc::new(ws_broadcast),
            sse_broadcast: Arc::new(sse_broadcast),
            mailer: Arc::new(mailer),
            secret_key,
        })
    }

    /// Init processes that need to be run on startup
    pub async fn init(&self) {
        // init elastic
        Node::build_index(&self.elastic_client).await;
        User::build_index(&self.elastic_client).await;

        let data = RequestData {
            app: web::Data::new(self.clone()),
            current_user: Default::default(),
        };

        tasks::recovery_task(data).await;
        tasks::cleanup_rooms_task(self.sse_broadcast.clone()).await;
        tasks::listen_redis_events(self).await;
    }

    pub fn cors(&self) -> Cors {
        let mut cors = Cors::default().allowed_origin(self.config.allowed_origin_1.as_str());

        if let Some(allowed_origin_2) = self.config.allowed_origin_2.as_ref() {
            cors = cors.allowed_origin(allowed_origin_2);
        }

        cors.supports_credentials()
            .allowed_methods(vec!["GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"])
            .allowed_headers(vec![
                http::header::AUTHORIZATION,
                http::header::ACCEPT,
                http::header::ORIGIN,
                http::header::USER_AGENT,
                http::header::DNT,
                http::header::CONTENT_TYPE,
                http::header::X_FORWARDED_FOR,
                http::header::X_FORWARDED_PROTO,
                http::header::X_FORWARDED_HOST,
            ])
            .expose_headers(vec![http::header::LOCATION, http::header::ACCESS_CONTROL_ALLOW_ORIGIN])
            .max_age(86400)
    }

    pub fn port(&self) -> u16 {
        self.config.port
    }

    pub fn session_middleware(&self, rss: RedisClusterSessionStore) -> SessionMiddleware<RedisClusterSessionStore> {
        let secret_key = Key::from(self.secret_key.as_ref());

        let expiration = self.config.session_expiration_in_days;
        let ttl = PersistentSession::default().session_ttl(time::Duration::days(expiration));

        SessionMiddleware::builder(rss, secret_key)
            .session_lifecycle(ttl)
            .cookie_secure(self.config.ssl)
            .build()
    }
}
