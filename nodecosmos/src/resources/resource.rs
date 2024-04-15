use std::time::Duration;

use aws_config::BehaviorVersion;
use deadpool_redis::Pool;
use elasticsearch::http::transport::Transport;
use elasticsearch::Elasticsearch;
use scylla::{CachingSession, Session, SessionBuilder};
use toml::Value;

use crate::resources::description_ws_pool::DescriptionWsPool;
use crate::resources::resource_locker::ResourceLocker;
use crate::resources::sse_broadcast::SseBroadcast;

/// Resource's should be alive during application runtime.
/// It's usually related to external services like db clients,
/// redis, elastic, etc.
pub trait Resource<'a> {
    type Cfg;

    #[allow(opaque_hidden_inferred_bound)]
    async fn init_resource(config: Self::Cfg) -> Self;
}

impl<'a> Resource<'a> for CachingSession {
    type Cfg = &'a Value;

    async fn init_resource(config: Self::Cfg) -> Self {
        let hosts = config["scylla"]["hosts"].as_array().expect("Missing hosts");

        let keyspace = config["scylla"]["keyspace"].as_str().expect("Missing keyspace");

        let known_nodes: Vec<&str> = hosts.iter().map(|x| x.as_str().unwrap()).collect();

        let db_session: Session = SessionBuilder::new()
            .known_nodes(&known_nodes)
            .connection_timeout(Duration::from_secs(3))
            .use_keyspace(keyspace, false)
            .build()
            .await
            .unwrap_or_else(|e| panic!("Unable to connect to scylla hosts: {:?}. \nError: {}", known_nodes, e));

        CachingSession::from(db_session, 1000)
    }
}

impl<'a> Resource<'a> for Elasticsearch {
    type Cfg = &'a Value;

    async fn init_resource(config: Self::Cfg) -> Self {
        let host = config["elasticsearch"]["host"].as_str().expect("Missing elastic host");

        let transport = Transport::single_node(host).unwrap_or_else(|e| {
            panic!(
                "Unable to connect to elastic host: {}. \nError: {}",
                config["elasticsearch"]["host"], e
            )
        });

        Elasticsearch::new(transport)
    }
}

impl<'a> Resource<'a> for Pool {
    type Cfg = &'a Value;

    async fn init_resource(config: Self::Cfg) -> Self {
        let redis_url = config["redis"]["url"].as_str().expect("Missing redis url");

        let cfg = deadpool_redis::Config::from_url(redis_url);

        cfg.create_pool(Some(deadpool_redis::Runtime::Tokio1))
            .expect("Failed to create pool.")
    }
}

impl<'a> Resource<'a> for aws_sdk_s3::Client {
    type Cfg = ();

    async fn init_resource(_config: ()) -> Self {
        let config = aws_config::defaults(BehaviorVersion::latest()).load().await;

        let client = aws_sdk_s3::Client::new(&config);

        client
    }
}

impl<'a> Resource<'a> for ammonia::Builder<'a> {
    type Cfg = ();

    async fn init_resource(_cfg: ()) -> Self {
        let mut sanitizer = ammonia::Builder::default();
        sanitizer
            .add_tag_attributes("img", &["resizable"])
            .add_tag_attributes("pre", &["spellcheck"])
            .add_tag_attributes("code", &["spellcheck", "data-code-block-language", "spellcheck"]);

        sanitizer
    }
}

impl<'a> Resource<'a> for ResourceLocker {
    type Cfg = &'a Pool;

    async fn init_resource(pool: &'a Pool) -> Self {
        ResourceLocker::new(pool)
    }
}

impl<'a> Resource<'a> for DescriptionWsPool {
    type Cfg = ();

    async fn init_resource(_config: ()) -> Self {
        DescriptionWsPool::default()
    }
}

impl<'a> Resource<'a> for SseBroadcast {
    type Cfg = ();

    async fn init_resource(_config: ()) -> Self {
        SseBroadcast::new()
    }
}
