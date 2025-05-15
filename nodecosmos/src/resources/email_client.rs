use crate::app::SmtpConfig;
use crate::errors::NodecosmosError;
use lettre::message::Mailbox;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Address, SmtpTransport, Transport};
use serde::Deserialize;

pub struct SesMailer {
    pub client: aws_sdk_ses::Client,
}

impl SesMailer {
    pub fn new(client: aws_sdk_ses::Client) -> Self {
        Self { client }
    }

    async fn send_email(&self, to: String, subject: &str, message: String) -> Result<(), NodecosmosError> {
        let mut dest = aws_sdk_ses::types::Destination::builder().build();

        dest.to_addresses = Some(vec![to]);

        let subject_content = aws_sdk_ses::types::Content::builder()
            .data(subject)
            .charset("UTF-8")
            .build()
            .expect("building Content");
        let body_content = aws_sdk_ses::types::Content::builder()
            .data(message)
            .charset("UTF-8")
            .build()
            .expect("building Content");
        let body = aws_sdk_ses::types::Body::builder().html(body_content).build();

        let msg = aws_sdk_ses::types::Message::builder()
            .subject(subject_content)
            .body(body)
            .build();

        self.client
            .send_email()
            .destination(dest)
            .source("support@nodecosmos.com")
            .message(msg)
            .send()
            .await
            .map_err(|e| NodecosmosError::AwsSdkError(format!("{:?}", e)))?;

        Ok(())
    }
}

#[derive(Clone, Deserialize, strum_macros::Display, strum_macros::EnumString)]
#[serde(rename_all = "lowercase")]
pub enum TlsMode {
    None,
    Tls,
    StartTls,
}

pub struct Smtp {
    pub from: Mailbox,
    pub client: SmtpTransport,
}

impl Smtp {
    pub fn new(smtp_cfg: SmtpConfig) -> Self {
        let transport_builder = match smtp_cfg.tls_mode {
            TlsMode::None => Ok(SmtpTransport::builder_dangerous(&smtp_cfg.host)),
            TlsMode::Tls => SmtpTransport::relay(&smtp_cfg.host),
            TlsMode::StartTls => SmtpTransport::starttls_relay(&smtp_cfg.host),
        };

        let mut client = transport_builder
            .expect("SMTP transport builder failed")
            .port(smtp_cfg.port);

        if !smtp_cfg.username.is_empty() {
            client = client.credentials(Credentials::new(smtp_cfg.username, smtp_cfg.password));
        }

        let client = client.build();

        Self {
            from: Mailbox::new(
                smtp_cfg.from_name,
                smtp_cfg.from_email.parse::<Address>().expect("Invalid SMTP from email"),
            ),
            client,
        }
    }

    pub async fn send_email(&self, to: String, subject: &str, message: String) -> Result<(), NodecosmosError> {
        let email = lettre::Message::builder()
            .from(self.from.clone())
            .to(to.parse().expect("parsing email"))
            .subject(subject)
            .header(lettre::message::header::ContentType::TEXT_HTML)
            .body(message)
            .map_err(|e| NodecosmosError::EmailError(format!("{:?}", e)))?;

        self.client
            .send(&email)
            .map_err(|e| NodecosmosError::EmailError(format!("{:?}", e)))?;

        Ok(())
    }
}

pub enum EmailClient {
    Ses(SesMailer),
    Smtp(Smtp),
}

impl EmailClient {
    pub async fn send_email(&self, to: String, subject: &str, message: String) -> Result<(), NodecosmosError> {
        match self {
            Self::Ses(mailer) => mailer.send_email(to, subject, message).await,
            Self::Smtp(mailer) => mailer.send_email(to, subject, message).await,
        }
    }
}
