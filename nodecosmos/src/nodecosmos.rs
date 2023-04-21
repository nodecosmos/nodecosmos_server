use actix_cors::Cors;
use actix_session::storage::RedisActorSessionStore;
use actix_session::SessionMiddleware;
use actix_web::cookie::Key;
use actix_web::{http, web};
use scylla::{CachingSession, Session, SessionBuilder};
use std::{env, fs, time::Duration};
use toml::Value;

#[derive(Clone)]
pub struct Nodecosmos {
    config: Value,
    env: String,
}

impl Nodecosmos {
    pub fn new() -> Self {
        dotenv::dotenv().ok();

        let env = env::var("ENV").expect("ENV must be set");
        let config_file = format!("config.{}.toml", env);

        let contents = fs::read_to_string(config_file).expect("Unable to read file");
        let value = contents.parse::<Value>().expect("Unable to parse TOML");

        Self { config: value, env }
    }
}

pub async fn get_db_session(app: &Nodecosmos) -> web::Data<CachingSession> {
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
        .unwrap();

    let session = CachingSession::from(session, 1000);

    web::Data::new(session)
}

pub fn get_cors(app: &Nodecosmos) -> Cors {
    let allowed_origin = app.config["allowed_origin"]
        .as_str()
        .expect("Missing allowed_origin")
        .to_string();

    Cors::default()
        .allowed_origin(allowed_origin.as_str())
        .allowed_methods(vec!["GET", "POST", "PUT", "DELETE"])
        .allowed_headers(vec![http::header::AUTHORIZATION, http::header::ACCEPT])
        .allowed_header(http::header::CONTENT_TYPE)
        .max_age(3600)
}

pub fn get_port(app: &Nodecosmos) -> u16 {
    app.config["port"].as_integer().expect("Missing port") as u16
}

pub fn get_secret_key(app: &Nodecosmos) -> Key {
    let secret_key = app.config["secret_key"]
        .as_str()
        .expect("Missing secret_key")
        .to_string();

    Key::from(secret_key.as_ref())
}

pub fn get_redis_actor_session_store(app: &Nodecosmos) -> RedisActorSessionStore {
    let redis_host = app.config["redis"]["host"]
        .as_str()
        .expect("Missing redis url");

    RedisActorSessionStore::new(redis_host)
}

pub fn get_session_middleware(app: &Nodecosmos) -> SessionMiddleware<RedisActorSessionStore> {
    let secret_key = get_secret_key(app);
    let redis_actor_session_store = get_redis_actor_session_store(app);
    let store = SessionMiddleware::new(redis_actor_session_store, secret_key);
    store
}
