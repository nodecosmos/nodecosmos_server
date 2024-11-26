use std::time::Duration;

use crate::app::Config;
use aws_config::BehaviorVersion;
use deadpool_redis::Pool;
use elasticsearch::http::transport::Transport;
use elasticsearch::Elasticsearch;
use openssl::ssl::{SslContextBuilder, SslMethod, SslVerifyMode};
use scylla::{CachingSession, SessionBuilder};

use crate::resources::description_ws_pool::DescriptionWsPool;
use crate::resources::email_client::{EmailClient, SesMailer, Smtp};
use crate::resources::mailer::Mailer;
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
    type Cfg = &'a Config;

    async fn init_resource(config: Self::Cfg) -> Self {
        let known_nodes: Vec<&str> = config.scylla.hosts.iter().map(|x| x.as_str()).collect();

        let mut builder = SessionBuilder::new()
            .known_nodes(&known_nodes)
            .connection_timeout(Duration::from_secs(3))
            .use_keyspace(&config.scylla.keyspace, false);

        if let Some(ca) = &config.scylla.ca {
            let mut context_builder = SslContextBuilder::new(SslMethod::tls())
                .map_err(|e| {
                    eprintln!("Failed to create SSL context: {}", e);
                    std::process::exit(1);
                })
                .unwrap();

            context_builder
                .set_ca_file(ca)
                .map_err(|e| {
                    eprintln!("Failed to set CA file: {}", e);
                    std::process::exit(1);
                })
                .unwrap();

            context_builder.set_verify(SslVerifyMode::PEER);

            if let Some(key) = &config.scylla.cert {
                context_builder
                    .set_certificate_file(key, openssl::ssl::SslFiletype::PEM)
                    .map_err(|e| {
                        eprintln!("Failed to set certificate file: {}", e);
                        std::process::exit(1);
                    })
                    .unwrap();

                let key = config
                    .scylla
                    .key
                    .as_ref()
                    .expect("Private key file is required when certificate is provided");
                context_builder
                    .set_private_key_file(key, openssl::ssl::SslFiletype::PEM)
                    .map_err(|e| {
                        eprintln!("Failed to set private key file: {}", e);
                        std::process::exit(1);
                    })
                    .unwrap();
            }

            builder = builder.ssl_context(Some(context_builder.build()));
        }

        let db_session = builder
            .build()
            .await
            .unwrap_or_else(|e| panic!("Unable to connect to scylla hosts: {:?}. \nError: {}", known_nodes, e));

        CachingSession::from(db_session, 1000)
    }
}

impl<'a> Resource<'a> for Elasticsearch {
    type Cfg = &'a Config;

    async fn init_resource(config: Self::Cfg) -> Self {
        let transport = Transport::single_node(&config.elasticsearch.host).unwrap_or_else(|e| {
            panic!(
                "Unable to connect to elastic host: {}. \nError: {}",
                &config.elasticsearch.host, e
            )
        });

        Elasticsearch::new(transport)
    }
}

impl<'a> Resource<'a> for Pool {
    type Cfg = &'a Config;

    async fn init_resource(config: Self::Cfg) -> Self {
        let cfg = deadpool_redis::Config::from_url(&config.redis.url);

        cfg.create_pool(Some(deadpool_redis::Runtime::Tokio1))
            .expect("Failed to create pool.")
    }
}

impl<'a> Resource<'a> for aws_sdk_s3::Client {
    type Cfg = ();

    async fn init_resource(_config: ()) -> Self {
        let config = aws_config::defaults(BehaviorVersion::latest()).load().await;

        aws_sdk_s3::Client::new(&config)
    }
}

impl<'a> Resource<'a> for Mailer {
    type Cfg = &'a Config;

    async fn init_resource(cfg: Self::Cfg) -> Self {
        // check if we use smtp or ses

        let client = if let Some(smtp_config) = cfg.smtp.clone() {
            EmailClient::Smtp(Smtp::new(smtp_config))
        } else {
            let ses_cfg = aws_config::defaults(BehaviorVersion::latest()).load().await;

            let client = aws_sdk_ses::Client::new(&ses_cfg);

            EmailClient::Ses(SesMailer::new(client))
        };

        Mailer::new(client, cfg)
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
    type Cfg = (&'a Pool, u8);

    async fn init_resource(cfg: Self::Cfg) -> Self {
        ResourceLocker::new(cfg.0, cfg.1)
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
