use scylla::{CachingSession, Session, SessionBuilder};
use std::{env, fs, time::Duration};
use toml::Value;

pub async fn init_session () -> &'static CachingSession {
    // dotenv::dotenv().ok();
    //
    // let contents = fs::read_to_string("config.toml").expect("Unable to read file");
    // let value = contents.parse::<Value>().expect("Unable to parse TOML");
    //
    // // get environment variables
    // let env: String = env::var("ENV").expect("ENV must be set");
    //
    // let hosts = value["scylla"][env.clone()]["hosts"].as_array().expect("Missing hosts");
    // let keyspace = value["scylla"][env]["keyspace"].as_str().expect("Missing keyspace");
    //
    // println!("Hosts: {:?}", hosts);
    let mut known_nodes = ["172.22.0.4", "172.22.0.3", "172.22.0.2"];

    let session: Session = SessionBuilder::new()
        .known_nodes(&known_nodes)
        .connection_timeout(Duration::from_secs(3))
        .use_keyspace("nodecosmos", false)
        .build()
        .await
        .unwrap();

    let caching_session = CachingSession::from(session, 1000);

    let p = Box::into_raw(Box::new(caching_session));
    unsafe { &*p }
}
