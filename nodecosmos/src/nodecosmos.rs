use scylla::{CachingSession, Session, SessionBuilder};
use std::{env, fs, time::Duration};
use toml::Value;

pub(crate) struct Nodecosmos {
    config: Value,
    env: String,
}

impl Nodecosmos {
    pub(crate) fn new() -> Self {
        dotenv::dotenv().ok();

        let contents = fs::read_to_string("config.toml").expect("Unable to read file");
        let value = contents.parse::<Value>().expect("Unable to parse TOML");

        Self {
            config: value,
            env: env::var("ENV").expect("ENV must be set"),
        }
    }

    pub(crate) async fn get_db_session(&self) -> CachingSession {
        let hosts = self.config["scylla"][&self.env]["hosts"]
            .as_array()
            .expect("Missing hosts");

        let keyspace = self.config["scylla"][&self.env]["keyspace"]
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

        CachingSession::from(session, 1000)
    }

    pub(crate) fn get_allowed_origin(&self) -> String {
        self.config["web"][&self.env]["allowed_origin"]
            .as_str()
            .expect("Missing allowed_origin")
            .to_string()
    }
}
