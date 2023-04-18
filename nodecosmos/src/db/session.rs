use scylla::{CachingSession, Session, SessionBuilder};
use std::{env, fs, time::Duration};
use toml::Value;

pub async fn init_session() -> CachingSession {
    dotenv::dotenv().ok();

    let contents = fs::read_to_string("config.toml").expect("Unable to read file");
    let value = contents.parse::<Value>().expect("Unable to parse TOML");

    // get environment variables
    let env: String = env::var("ENV").expect("ENV must be set");

    let hosts = value["scylla"][env.clone()]["hosts"]
        .as_array()
        .expect("Missing hosts");

    let keyspace = value["scylla"][env]["keyspace"]
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

    session
}
