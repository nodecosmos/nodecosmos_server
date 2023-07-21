use actix_cors::Cors;
use actix_session::config::PersistentSession;
use actix_session::storage::RedisActorSessionStore;
use actix_session::SessionMiddleware;
use actix_web::cookie::Key;
use actix_web::{cookie, http};
use elasticsearch::http::transport::Transport;
use elasticsearch::Elasticsearch;
use scylla::{CachingSession, Session, SessionBuilder};
use std::{env, fs, time::Duration};
use toml::Value;

#[derive(Clone)]
pub struct App {
    config: Value,
}

impl App {
    pub fn new() -> Self {
        dotenv::dotenv().ok();

        let env = env::var("ENV").expect("ENV must be set");
        let config_file = format!("config.{}.toml", env);

        let contents = fs::read_to_string(config_file).expect("Unable to read file");
        let value = contents.parse::<Value>().expect("Unable to parse TOML");

        Self { config: value }
    }
}

#[derive(Clone)]
pub struct CbExtension {
    pub elastic_client: Elasticsearch,
}

pub async fn get_db_session(app: &App) -> CachingSession {
    let hosts = app.config["scylla"]["hosts"]
        .as_array()
        .expect("Missing hosts");

    let keyspace = app.config["scylla"]["keyspace"]
        .as_str()
        .expect("Missing keyspace");

    let known_nodes: Vec<&str> = hosts.iter().map(|x| x.as_str().unwrap()).collect();

    let session: Session = SessionBuilder::new()
        .known_nodes(&known_nodes)
        .connection_timeout(Duration::from_secs(3))
        .use_keyspace(keyspace, false)
        .build()
        .await
        .unwrap_or_else(|e| {
            panic!(
                "Unable to connect to scylla hosts: {:?}. \nError: {}",
                known_nodes, e
            )
        });

    let session = CachingSession::from(session, 1000);

    session
}

pub async fn get_elastic_client(app: &App) -> Elasticsearch {
    let host = app.config["elasticsearch"]["host"]
        .as_str()
        .expect("Missing elastic host");

    let transport = Transport::single_node(host).unwrap_or_else(|e| {
        panic!(
            "Unable to connect to elastic host: {}. \nError: {}",
            app.config["elasticsearch"]["host"], e
        )
    });

    let client = Elasticsearch::new(transport);

    client
}

pub fn get_cors(app: &App) -> Cors {
    let allowed_origin = app.config["allowed_origin"]
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
        .expose_headers(vec![
            http::header::LOCATION,
            http::header::ACCESS_CONTROL_ALLOW_ORIGIN,
        ])
        .max_age(86400)
}

pub fn get_port(app: &App) -> u16 {
    app.config["port"].as_integer().expect("Missing port") as u16
}

pub fn get_secret_key(app: &App) -> Key {
    let secret_key = app.config["secret_key"]
        .as_str()
        .expect("Missing secret_key")
        .to_string();

    Key::from(secret_key.as_ref())
}

pub fn get_redis_actor_session_store(app: &App) -> RedisActorSessionStore {
    let redis_host = app.config["redis"]["host"]
        .as_str()
        .expect("Missing redis url");

    RedisActorSessionStore::new(redis_host)
}

pub fn get_session_middleware(app: &App) -> SessionMiddleware<RedisActorSessionStore> {
    let secret_key = get_secret_key(app);
    let redis_actor_session_store = get_redis_actor_session_store(app);
    let expiration = app.config["session_expiration_in_days"]
        .as_integer()
        .expect("Missing session_expiration");
    let ttl = PersistentSession::default().session_ttl(cookie::time::Duration::days(expiration));

    SessionMiddleware::builder(redis_actor_session_store, secret_key)
        .session_lifecycle(ttl)
        .build()
}
