use scylla::{Session, SessionBuilder};
use std::time::Duration;
use dotenv::dotenv;
use std::env;

pub(crate) async fn initialize_session() -> Session {
    dotenv().ok();

    let scylla_host_1 = env::var("SCYLLA_HOST_1").expect("SCYLLA_HOST_1 must be set");
    let scylla_host_2 = env::var("SCYLLA_HOST_2").expect("SCYLLA_HOST_2 must be set");
    let scylla_host_3 = env::var("SCYLLA_HOST_3").expect("SCYLLA_HOST_3 must be set");

    SessionBuilder::new()
        .known_node(scylla_host_1)
        .known_node(scylla_host_2)
        .known_node(scylla_host_3)
        .connection_timeout(Duration::from_secs(3))
        .use_keyspace("nodecosmos", false)
        .build()
        .await
        .unwrap()
}
