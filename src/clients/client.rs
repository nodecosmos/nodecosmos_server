use crate::clients::description_ws_pool::DescriptionWsPool;
use crate::clients::resource_locker::ResourceLocker;
use crate::clients::sse_pool::SsePool;
use deadpool_redis::Pool;
use elasticsearch::http::transport::Transport;
use elasticsearch::Elasticsearch;
use scylla::{CachingSession, Session, SessionBuilder};
use std::time::Duration;
use toml::Value;

/// Client's should be alive during application runtime. It's usually related to external services like db clients,
/// redis, elastic, etc.
pub trait Client<'a> {
    type Cfg;

    #[allow(opaque_hidden_inferred_bound)]
    async fn init_client(config: Self::Cfg) -> Self;
}

impl<'a> Client<'a> for CachingSession {
    type Cfg = &'a Value;

    async fn init_client(config: Self::Cfg) -> Self {
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
}

impl<'a> Client<'a> for Elasticsearch {
    type Cfg = &'a Value;

    async fn init_client(config: Self::Cfg) -> Self {
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

impl<'a> Client<'a> for Pool {
    type Cfg = &'a Value;

    async fn init_client(config: Self::Cfg) -> Self {
        let redis_url = config["redis"]["url"].as_str().expect("Missing redis url");

        let cfg = deadpool_redis::Config::from_url(redis_url);

        cfg.create_pool(Some(deadpool_redis::Runtime::Tokio1))
            .expect("Failed to create pool.")
    }
}

impl<'a> Client<'a> for aws_sdk_s3::Client {
    type Cfg = ();

    async fn init_client(_config: ()) -> Self {
        let config = aws_config::from_env().load().await;

        let client = aws_sdk_s3::Client::new(&config);

        client
    }
}

impl<'a> Client<'a> for ResourceLocker {
    type Cfg = &'a Pool;

    async fn init_client(pool: &'a Pool) -> Self {
        ResourceLocker::new(pool)
    }
}

impl<'a> Client<'a> for DescriptionWsPool {
    type Cfg = ();

    async fn init_client(_config: ()) -> Self {
        DescriptionWsPool::default()
    }
}

impl<'a> Client<'a> for SsePool {
    type Cfg = ();

    async fn init_client(_config: ()) -> Self {
        SsePool::new()
    }
}
