use crate::errors::NodecosmosError;
use handlebars::Handlebars;
use std::collections::HashMap;
use toml::Value;

const CONFIRM_EMAIL: &str = "confirm_email";

pub struct Mailer {
    pub templates: Handlebars<'static>,
    pub client_url: String,
    pub ses_client: aws_sdk_ses::Client,
}

impl Mailer {
    pub fn new(ses_client: aws_sdk_ses::Client, config: &Value) -> Self {
        let mut templates = Handlebars::new();
        let client_url = config["client_url"].as_str().expect("Missing client url").to_string();

        templates
            .register_template_string(CONFIRM_EMAIL, include_str!("./mailer/confirm_email.html"))
            .expect("Template should be valid");

        Self {
            templates,
            client_url,
            ses_client,
        }
    }

    pub async fn send_confirm_user_email(
        &self,
        to: String,
        username: String,
        token: String,
    ) -> Result<(), NodecosmosError> {
        let url = format!("{}/{}?token={}", self.client_url, username, token);
        let logo_url = format!("{}/logo.svg", self.client_url);

        let mut ctx = HashMap::<&str, &str>::new();
        ctx.insert("confirmation_link", &url);
        ctx.insert("logo_url", &logo_url);

        let message = self
            .templates
            .render(CONFIRM_EMAIL, &ctx)
            .map_err(|e| NodecosmosError::TemplateError(e.to_string()))?;

        self.send_email(to, "Confirm your email", message).await
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
        let body = aws_sdk_ses::types::Body::builder().text(body_content).build();

        let msg = aws_sdk_ses::types::Message::builder()
            .subject(subject_content)
            .body(body)
            .build();

        self.ses_client
            .send_email()
            .destination(dest)
            .source("support@nodecosmos.com")
            .message(msg)
            .send()
            .await
            .map_err(|e| NodecosmosError::AwsSdkError(e.to_string()))?;

        Ok(())
    }
}
