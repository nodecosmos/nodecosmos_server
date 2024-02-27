use crate::api::data::RequestData;
use crate::models::branch::merge::BranchMerge;
use crate::models::node::reorder::Recovery;
use crate::services::elastic::ElasticIndexBuilder;
use crate::services::resource_locker::ResourceLocker;
use actix_cors::Cors;
use actix_session::config::PersistentSession;
use actix_session::storage::RedisActorSessionStore;
use actix_session::SessionMiddleware;
use actix_web::cookie::Key;
use actix_web::{cookie, http, web};
use deadpool_redis::{Config, Pool, Runtime};
use elasticsearch::http::transport::Transport;
use elasticsearch::Elasticsearch;
use scylla::{CachingSession, Session, SessionBuilder};
use std::sync::Arc;
use std::{env, fs, time::Duration};
use toml::Value;

#[derive(Clone)]
pub struct App {
    pub config: Value,
    pub db_session: Arc<CachingSession>,
    pub elastic_client: Arc<Elasticsearch>,
    pub redis_pool: Arc<Pool>,
    pub resource_locker: Arc<ResourceLocker>,
    pub s3_bucket: String,
    pub s3_client: Arc<aws_sdk_s3::Client>,
}

impl App {
    async fn create_caching_session(config: &Value) -> CachingSession {
        let hosts = config["scylla"]["hosts"].as_array().expect("Missing hosts");

        let keyspace = config["scylla"]["keyspace"].as_str().expect("Missing keyspace");

        let known_nodes: Vec<&str> = hosts.iter().map(|x| x.as_str().unwrap()).collect();

        let session: Session = SessionBuilder::new()
            .known_nodes(&known_nodes)
            .connection_timeout(Duration::from_secs(3))
            .use_keyspace(keyspace, false)
            .build()
            .await
            .unwrap_or_else(|e| panic!("Unable to connect to scylla hosts: {:?}. \nError: {}", known_nodes, e));

        CachingSession::from(session, 1000)
    }

    async fn create_elastic_client(config: &Value) -> Elasticsearch {
        let host = config["elasticsearch"]["host"].as_str().expect("Missing elastic host");

        let transport = Transport::single_node(host).unwrap_or_else(|e| {
            panic!(
                "Unable to connect to elastic host: {}. \nError: {}",
                config["elasticsearch"]["host"], e
            )
        });

        Elasticsearch::new(transport)
    }

    async fn create_redis_pool(config: &Value) -> Pool {
        let redis_url = config["redis"]["url"].as_str().expect("Missing redis url");

        let cfg = Config::from_url(redis_url);

        cfg.create_pool(Some(Runtime::Tokio1)).expect("Failed to create pool.")
    }

    async fn create_s3_client() -> aws_sdk_s3::Client {
        let config = aws_config::from_env().load().await;

        let client = aws_sdk_s3::Client::new(&config);

        client
    }

    pub async fn new() -> Self {
        dotenv::dotenv().ok();

        let env = env::var("ENV").expect("ENV must be set");
        let config_file = format!("config.{}.toml", env);
        let contents = fs::read_to_string(config_file).expect("Unable to read file");
        let config = contents.parse::<Value>().expect("Unable to parse TOML");
        let s3_bucket = config["aws"]["bucket"].as_str().expect("Missing bucket").to_string();

        let s3_client = Self::create_s3_client().await;
        let db_session = Self::create_caching_session(&config).await;
        let elastic_client = Self::create_elastic_client(&config).await;
        let redis_pool = Self::create_redis_pool(&config).await;
        let resource_locker = ResourceLocker::new(&redis_pool);

        Self {
            config,
            db_session: Arc::new(db_session),
            elastic_client: Arc::new(elastic_client),
            resource_locker: Arc::new(resource_locker),
            redis_pool: Arc::new(redis_pool),
            s3_bucket,
            s3_client: Arc::new(s3_client),
        }
    }

    /// Init processes that need to be run on startup
    pub async fn init(&self) {
        // init logger
        env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
        // init elastic
        ElasticIndexBuilder::new(&self.elastic_client).build().await;
        // init recovery in case reordering was interrupted or failed
        Recovery::recover_from_stored_data(&self.db_session, &self.resource_locker).await;
        BranchMerge::recover_from_stored_data(&RequestData {
            app: web::Data::new(self.clone()),
            current_user: Default::default(),
        })
        .await;
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
            .cookie_secure(false)
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
