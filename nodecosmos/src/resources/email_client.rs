use crate::app::SmtpConfig;
use crate::errors::NodecosmosError;
use lettre::message::Mailbox;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Address, Transport};

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

pub struct Smtp {
    pub relay: String,
    pub port: u16,
    pub from: Mailbox,
    pub creds: Credentials,
    pub starttls: bool,
}

impl Smtp {
    pub fn new(smtp_cfg: SmtpConfig) -> Self {
        Self {
            relay: smtp_cfg.host,
            port: smtp_cfg.port,
            from: Mailbox::new(
                smtp_cfg.from_name,
                smtp_cfg.from_email.parse::<Address>().expect("Invalid SMTP from email"),
            ),
            creds: Credentials::new(smtp_cfg.username, smtp_cfg.password),
            starttls: smtp_cfg.starttls,
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

        let transport_builder = if self.starttls {
            lettre::SmtpTransport::starttls_relay(&self.relay)
        } else {
            lettre::SmtpTransport::relay(&self.relay)
        };

        let mailer = transport_builder
            .map_err(|e| NodecosmosError::EmailError(format!("Relay Error {:?}", e)))?
            .port(self.port)
            .credentials(self.creds.clone())
            .build();

        mailer
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
