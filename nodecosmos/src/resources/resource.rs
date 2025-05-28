use redis::{AsyncCommands, ProtocolVersion};
use scylla::client::session_builder::SessionBuilder;
use std::io::Read;

const PASSPHRASE: &str = "KEEP_CALM_AND_LET_IT_BE";

fn create_pkcs12_bundle(tls_crt: &str, tls_key: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // Read the PEM files
    let cert_pem = std::fs::read(tls_crt)?; // Read the certificate from tls_crt
    let key_pem = std::fs::read(tls_key)?; // Read the private key from tls_key

    // Parse the certificate and private key
    let cert = openssl::x509::X509::from_pem(&cert_pem)?;
    let pkey = openssl::pkey::PKey::private_key_from_pem(&key_pem)?;

    // Build the PKCS#12 bundle; an empty password ("") is used here, but you can change that if needed.
    let pkcs12 = openssl::pkcs12::Pkcs12::builder()
        .cert(&cert)
        .pkey(&pkey)
        .build2(PASSPHRASE)?;

    Ok(pkcs12.to_der().expect("Failed to convert pkcs12 to der"))
}

/// Resource's should be alive during application runtime.
/// It's usually related to external services like db clients,
/// redis, elastic, etc.
pub trait Resource<'a> {
    type Cfg;

    #[allow(opaque_hidden_inferred_bound)]
    async fn init_resource(config: Self::Cfg) -> Self;
}

impl<'a> Resource<'a> for scylla::client::caching_session::CachingSession {
    type Cfg = &'a crate::app::Config;

    async fn init_resource(config: Self::Cfg) -> Self {
        let known_nodes: Vec<&str> = config.scylla.hosts.iter().map(|x| x.as_str()).collect();

        let mut builder = SessionBuilder::new()
            .known_nodes(&known_nodes)
            .connection_timeout(std::time::Duration::from_secs(3))
            .use_keyspace(&config.scylla.keyspace, false);

        if let Some(ca) = &config.scylla.ca {
            let mut context_builder = openssl::ssl::SslContextBuilder::new(openssl::ssl::SslMethod::tls())
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

            context_builder.set_verify(openssl::ssl::SslVerifyMode::PEER);

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

            builder = builder.tls_context(Some(context_builder.build()));
        }

        let db_session = builder
            .build()
            .await
            .unwrap_or_else(|e| panic!("Unable to connect to scylla hosts: {:?}. \nError: {}", known_nodes, e));

        scylla::client::caching_session::CachingSession::from(db_session, 1000)
    }
}

impl<'a> Resource<'a> for elasticsearch::Elasticsearch {
    type Cfg = &'a crate::app::Config;

    async fn init_resource(config: Self::Cfg) -> Self {
        let urls: Vec<elasticsearch::http::Url> = config
            .elasticsearch
            .hosts
            .iter()
            .map(|url| elasticsearch::http::Url::parse(url))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                panic!(
                    "Invalid Elasticsearch URL: {}. \nError: {}",
                    config.elasticsearch.hosts.join(", "),
                    e
                )
            })
            .expect("Failed to parse Elasticsearch URL");

        let conn_pool = elasticsearch::http::transport::MultiNodeConnectionPool::round_robin(urls, None);
        let mut builder = elasticsearch::http::transport::TransportBuilder::new(conn_pool);

        log::info!("Connecting to Elasticsearch: {:?}", config.elasticsearch.hosts);

        if let (Some(ca), Some(cert), Some(key)) = (
            &config.elasticsearch.ca,
            &config.elasticsearch.cert,
            &config.elasticsearch.key,
        ) {
            log::info!("Using client certificate for Elasticsearch");

            let pkcs12 = create_pkcs12_bundle(cert, key)
                .map_err(|e| {
                    panic!(
                        "Failed to create pkcs12 bundle from ca, cert and key files. \nError: {:?}",
                        e
                    )
                })
                .unwrap();

            let cert_validation = elasticsearch::cert::CertificateValidation::Full(
                elasticsearch::cert::Certificate::from_pem(&std::fs::read(ca).expect("Failed to read ca file"))
                    .expect("Failed to create certificate from ca file"),
            );

            builder = builder
                .cert_validation(cert_validation)
                .auth(elasticsearch::auth::Credentials::Certificate(
                    elasticsearch::auth::ClientCertificate::Pkcs12(pkcs12, Some(PASSPHRASE.to_string())),
                ));
        }

        let transport = builder
            .build()
            .map_err(|e| {
                panic!(
                    "Unable to connect to elastic hosts: {}. \nError: {:?}",
                    config.elasticsearch.hosts.join(", "),
                    e
                )
            })
            .unwrap();

        log::info!("Connected to Elasticsearch");

        elasticsearch::Elasticsearch::new(transport)
    }
}

impl<'a> Resource<'a> for aws_sdk_s3::Client {
    type Cfg = ();

    async fn init_resource(_config: ()) -> Self {
        let config = aws_config::defaults(aws_config::BehaviorVersion::latest()).load().await;

        aws_sdk_s3::Client::new(&config)
    }
}

impl<'a> Resource<'a> for crate::resources::mailer::Mailer {
    type Cfg = &'a crate::app::Config;

    async fn init_resource(cfg: Self::Cfg) -> Self {
        // check if we use smtp or ses

        let client = if let Some(smtp_config) = cfg.smtp.clone() {
            crate::resources::email_client::EmailClient::Smtp(crate::resources::email_client::Smtp::new(smtp_config))
        } else {
            let ses_cfg = aws_config::defaults(aws_config::BehaviorVersion::latest()).load().await;

            let client = aws_sdk_ses::Client::new(&ses_cfg);

            crate::resources::email_client::EmailClient::Ses(crate::resources::email_client::SesMailer::new(client))
        };

        crate::resources::mailer::Mailer::new(client, cfg)
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

impl<'a> Resource<'a> for crate::resources::resource_locker::ResourceLocker {
    type Cfg = (&'a deadpool::managed::Pool<RedisClusterManager>, u8);

    async fn init_resource(cfg: Self::Cfg) -> Self {
        crate::resources::resource_locker::ResourceLocker::new(cfg.0, cfg.1)
    }
}

impl<'a> Resource<'a> for crate::resources::description_ws_pool::DescriptionWsPool {
    type Cfg = ();

    async fn init_resource(_config: ()) -> Self {
        crate::resources::description_ws_pool::DescriptionWsPool::default()
    }
}

impl<'a> Resource<'a> for crate::resources::sse_broadcast::SseBroadcast {
    type Cfg = ();

    async fn init_resource(_config: ()) -> Self {
        crate::resources::sse_broadcast::SseBroadcast::new()
    }
}

impl<'a> Resource<'a> for Vec<redis::Client> {
    type Cfg = &'a crate::app::Config;

    async fn init_resource(config: Self::Cfg) -> Self {
        let mut clients = Vec::new();
        use std::io::Read;

        for url in &config.redis.urls {
            let client;

            if let Some(ca) = &config.redis.ca {
                let root_cert_file = std::fs::File::open(ca).expect("cannot open private cert file");
                let mut root_cert_vec = Vec::new();
                std::io::BufReader::new(root_cert_file)
                    .read_to_end(&mut root_cert_vec)
                    .expect("Unable to read ROOT cert file");

                let cert_file = std::fs::File::open(config.redis.cert.clone().expect("redis must have cert defined"))
                    .expect("cannot open private cert file");
                let mut client_cert_vec = Vec::new();
                std::io::BufReader::new(cert_file)
                    .read_to_end(&mut client_cert_vec)
                    .expect("Unable to read client cert file");

                let key_file = std::fs::File::open(&config.redis.key.clone().expect("redis must have key defined"))
                    .expect("cannot open private key file");
                let mut client_key_vec = Vec::new();
                std::io::BufReader::new(key_file)
                    .read_to_end(&mut client_key_vec)
                    .expect("Unable to read client key file");

                log::info!("Connecting to Redis with TLS Vec<redis::Client>");

                let tls = redis::TlsCertificates {
                    client_tls: Some(redis::ClientTlsConfig {
                        client_cert: client_cert_vec,
                        client_key: client_key_vec,
                    }),
                    root_cert: Some(root_cert_vec),
                };

                client = redis::Client::build_with_tls(url.clone(), tls).expect("Unable to build client");

                let mut conn = client
                    .get_multiplexed_async_connection()
                    .await
                    .expect("Failed to connect to Redis");

                redis::cmd("SET")
                    .arg("test")
                    .arg("test")
                    .exec_async(&mut conn)
                    .await
                    .expect("Failed to set key from client");
            } else {
                client = redis::Client::open(url.clone()).expect("Failed to create Redis client");
            }

            clients.push(client);
        }

        clients
    }
}

impl<'a> Resource<'a> for redis::cluster::ClusterClient {
    type Cfg = &'a crate::app::Config;

    async fn init_resource(config: Self::Cfg) -> Self {
        let client;
        if let Some(ca) = &config.redis.ca {
            let root_cert_file = std::fs::File::open(ca).expect("cannot open private cert file");
            let mut root_cert_vec = Vec::new();
            std::io::BufReader::new(root_cert_file)
                .read_to_end(&mut root_cert_vec)
                .expect("Unable to read ROOT cert file");

            let cert_file = std::fs::File::open(config.redis.cert.clone().expect("redis must have cert defined"))
                .expect("cannot open private cert file");
            let mut client_cert_vec = Vec::new();
            std::io::BufReader::new(cert_file)
                .read_to_end(&mut client_cert_vec)
                .expect("Unable to read client cert file");

            let key_file = std::fs::File::open(&config.redis.key.clone().expect("redis must have key defined"))
                .expect("cannot open private key file");
            let mut client_key_vec = Vec::new();
            std::io::BufReader::new(key_file)
                .read_to_end(&mut client_key_vec)
                .expect("Unable to read client key file");

            log::info!("Connecting to Redis with TLS Cluster Client");

            let tls = redis::TlsCertificates {
                client_tls: Some(redis::ClientTlsConfig {
                    client_cert: client_cert_vec,
                    client_key: client_key_vec,
                }),
                root_cert: Some(root_cert_vec),
            };

            client = redis::cluster::ClusterClient::builder(config.redis.urls.clone())
                .certs(tls)
                .use_protocol(ProtocolVersion::RESP3)
                .build()
                .expect("Unable to build client");
        } else {
            client =
                redis::cluster::ClusterClient::new(config.redis.urls.clone()).expect("Failed to create Redis client");
        }

        let mut conn = client.get_async_connection().await.expect("Failed to connect to Redis");

        // test write
        let _: () = conn.set("test", "test").await.expect("Failed to set key from client");

        log::info!("Connected to Redis");

        client
    }
}
// Custom manager that holds your preconfigured ClusterClient.
pub struct RedisClusterManager {
    client: redis::cluster::ClusterClient,
}

impl RedisClusterManager {
    pub fn new(client: redis::cluster::ClusterClient) -> Self {
        Self { client }
    }
}

impl deadpool::managed::Manager for RedisClusterManager {
    type Type = redis::cluster_async::ClusterConnection;
    type Error = redis::RedisError;

    // Create a new connection from the preconfigured redis::cluster::ClusterClient.
    async fn create(&self) -> Result<Self::Type, redis::RedisError> {
        self.client.get_async_connection().await
    }

    // Optionally recycle the connection (e.g. by sending a PING).
    async fn recycle(
        &self,
        obj: &mut Self::Type,
        _metrics: &deadpool::managed::Metrics,
    ) -> Result<(), deadpool::managed::RecycleError<redis::RedisError>> {
        redis::cmd("PING")
            .query_async(obj)
            .await
            .map_err(deadpool::managed::RecycleError::Backend)
    }
}

impl<'a> Resource<'a> for deadpool::managed::Pool<RedisClusterManager> {
    type Cfg = &'a crate::app::Config;

    async fn init_resource(config: Self::Cfg) -> Self {
        let cluster_client = redis::cluster::ClusterClient::init_resource(config).await;

        let manager = RedisClusterManager::new(cluster_client);

        deadpool::managed::Pool::builder(manager)
            .build()
            .expect("Failed to build Redis pool")
    }
}
