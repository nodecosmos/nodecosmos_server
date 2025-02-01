use crate::app::Config;
use crate::resources::description_ws_pool::DescriptionWsPool;
use crate::resources::email_client::{EmailClient, SesMailer, Smtp};
use crate::resources::mailer::Mailer;
use crate::resources::resource_locker::ResourceLocker;
use crate::resources::sse_broadcast::SseBroadcast;
use ammonia::Url;
use aws_config::BehaviorVersion;
use deadpool_redis::Pool;
use elasticsearch::auth::{ClientCertificate, Credentials};
use elasticsearch::http::transport::{MultiNodeConnectionPool, TransportBuilder};
use elasticsearch::Elasticsearch;
use openssl::ssl::{SslContextBuilder, SslMethod, SslVerifyMode};
use redis::{ClientTlsConfig, TlsCertificates};
use scylla::{CachingSession, SessionBuilder};
use std::fs::File;
use std::io::{BufReader, Read};
use std::time::Duration;

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
        let urls: Vec<Url> = config
            .elasticsearch
            .hosts
            .iter()
            .map(|url| Url::parse(url))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                panic!(
                    "Invalid Elasticsearch URL: {}. \nError: {}",
                    config.elasticsearch.hosts.join(", "),
                    e
                )
            })
            .expect("Failed to parse Elasticsearch URL");

        let conn_pool = MultiNodeConnectionPool::round_robin(urls, None);
        let mut builder = TransportBuilder::new(conn_pool);

        if let Some(cert) = &config.elasticsearch.pem {
            builder = builder.auth(Credentials::Certificate(ClientCertificate::Pem(
                std::fs::read(cert).expect("Failed to read certificate"),
            )))
        }

        let transport = builder
            .build()
            .map_err(|e| {
                panic!(
                    "Unable to connect to elastic hosts: {}. \nError: {}",
                    config.elasticsearch.hosts.join(", "),
                    e
                )
            })
            .unwrap();

        Elasticsearch::new(transport)
    }
}

impl<'a> Resource<'a> for Pool {
    type Cfg = &'a Config;

    async fn init_resource(config: Self::Cfg) -> Self {
        let redis_client = redis::Client::init_resource(config).await;
        let connection_info = redis_client.get_connection_info();
        let cfg = deadpool_redis::Config::from_connection_info(connection_info.clone());

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

impl<'a> Resource<'a> for redis::Client {
    type Cfg = &'a Config;

    async fn init_resource(config: Self::Cfg) -> Self {
        if let Some(ca) = &config.redis.ca {
            let root_cert_file = File::open(ca).expect("cannot open private cert file");
            let mut root_cert_vec = Vec::new();
            BufReader::new(root_cert_file)
                .read_to_end(&mut root_cert_vec)
                .expect("Unable to read ROOT cert file");

            let cert_file = File::open(config.redis.cert.clone().expect("redis must have cert defined"))
                .expect("cannot open private cert file");
            let mut client_cert_vec = Vec::new();
            BufReader::new(cert_file)
                .read_to_end(&mut client_cert_vec)
                .expect("Unable to read client cert file");

            let key_file = File::open(&config.redis.key.clone().expect("redis must have key defined"))
                .expect("cannot open private key file");
            let mut client_key_vec = Vec::new();
            BufReader::new(key_file)
                .read_to_end(&mut client_key_vec)
                .expect("Unable to read client key file");

            redis::Client::build_with_tls(
                config.redis.url.clone(),
                TlsCertificates {
                    client_tls: Some(ClientTlsConfig {
                        client_cert: client_cert_vec,
                        client_key: client_key_vec,
                    }),
                    root_cert: Some(root_cert_vec),
                },
            )
            .expect("Unable to build client")
        } else {
            redis::Client::open(config.redis.url.clone()).expect("Failed to create Redis client")
        }
    }
}
