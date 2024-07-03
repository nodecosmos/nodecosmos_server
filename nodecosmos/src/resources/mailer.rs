use crate::app::Config;
use crate::errors::NodecosmosError;
use crate::models::contact::Contact;
use charybdis::types::Uuid;
use handlebars::Handlebars;
use std::collections::HashMap;

const CONFIRM_EMAIL: &str = "confirm_email";
const INVITATION_EMAIL: &str = "invitation_email";
const INVITATION_ACCEPTED_EMAIL: &str = "invitation_accepted_email";
const RESET_PASSWORD_EMAIL: &str = "reset_password_email";
const PASSWORD_CHANGED_EMAIL: &str = "password_changed_email";
const CONTACT_US_EMAIL: &str = "contact_us_email";

pub struct Mailer {
    pub templates: Handlebars<'static>,
    pub client_url: String,
    pub ses_client: aws_sdk_ses::Client,
}

impl Mailer {
    pub fn new(ses_client: aws_sdk_ses::Client, config: &Config) -> Self {
        let mut templates = Handlebars::new();

        templates
            .register_template_string(CONFIRM_EMAIL, include_str!("./mailer/confirm_email.html"))
            .expect("Template should be valid");

        templates
            .register_template_string(INVITATION_EMAIL, include_str!("./mailer/invitation_email.html"))
            .expect("Template should be valid");

        templates
            .register_template_string(
                INVITATION_ACCEPTED_EMAIL,
                include_str!("./mailer/invitation_accepted_email.html"),
            )
            .expect("Template should be valid");

        templates
            .register_template_string(RESET_PASSWORD_EMAIL, include_str!("./mailer/reset_password_email.html"))
            .expect("Template should be valid");

        templates
            .register_template_string(
                PASSWORD_CHANGED_EMAIL,
                include_str!("./mailer/password_changed_email.html"),
            )
            .expect("Template should be valid");

        templates
            .register_template_string(CONTACT_US_EMAIL, include_str!("./mailer/contact_us_email.html"))
            .expect("Template should be valid");

        Self {
            templates,
            client_url: config.client_url.clone(),
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

        let mut ctx = HashMap::<&str, &str>::new();
        ctx.insert("confirmation_url", &url);

        let message = self
            .templates
            .render(CONFIRM_EMAIL, &ctx)
            .map_err(|e| NodecosmosError::TemplateError(e.to_string()))?;

        self.send_email(to, "Confirm your nodecosmos account", message).await
    }

    pub async fn send_invitation_email(
        &self,
        to: String,
        username: &String,
        node_name: &String,
        token: String,
    ) -> Result<(), NodecosmosError> {
        let url = format!("{}/invitations?token={}", self.client_url, token);

        let mut ctx = HashMap::<&str, &str>::new();
        ctx.insert("username", &username);
        ctx.insert("node_name", &node_name);
        ctx.insert("invitation_url", &url);

        let message = self
            .templates
            .render(INVITATION_EMAIL, &ctx)
            .map_err(|e| NodecosmosError::TemplateError(e.to_string()))?;

        self.send_email(to, "You have been invited to collaborate on nodecosmos", message)
            .await
    }

    pub async fn send_invitation_accepted_email(
        &self,
        to: String,
        username: &String,
        branch_id: Uuid,
        node_id: Uuid,
        node_name: &String,
    ) -> Result<(), NodecosmosError> {
        let node_url = format!("{}/nodes/{}/{}", self.client_url, branch_id, node_id);

        let mut ctx = HashMap::<&str, &str>::new();
        ctx.insert("username", &username);
        ctx.insert("node_name", &node_name);
        ctx.insert("node_url", &node_url);

        let message = self
            .templates
            .render(INVITATION_ACCEPTED_EMAIL, &ctx)
            .map_err(|e| NodecosmosError::TemplateError(e.to_string()))?;

        self.send_email(to, "Your invitation has been accepted", message).await
    }

    pub async fn send_reset_password_email(
        &self,
        to: String,
        username: String,
        token: String,
    ) -> Result<(), NodecosmosError> {
        let url = format!("{}/reset_password?token={}", self.client_url, token);

        let mut ctx = HashMap::<&str, &str>::new();
        ctx.insert("username", &username);
        ctx.insert("reset_password_url", &url);

        let message = self
            .templates
            .render(RESET_PASSWORD_EMAIL, &ctx)
            .map_err(|e| NodecosmosError::TemplateError(e.to_string()))?;

        self.send_email(to, "Reset your nodecosmos password", message).await
    }

    pub async fn send_password_changed_email(&self, to: String, username: String) -> Result<(), NodecosmosError> {
        let mut ctx = HashMap::<&str, &str>::new();
        ctx.insert("username", &username);

        let message = self
            .templates
            .render(PASSWORD_CHANGED_EMAIL, &ctx)
            .map_err(|e| NodecosmosError::TemplateError(e.to_string()))?;

        self.send_email(to, "Your nodecosmos password has been changed", message)
            .await
    }

    pub async fn send_contact_us_email(&self, contact: &Contact) -> Result<(), NodecosmosError> {
        let mut ctx = HashMap::<&str, &str>::new();
        ctx.insert("first_name", &contact.first_name);
        ctx.insert("last_name", &contact.last_name);
        ctx.insert("email", &contact.email);
        ctx.insert("company_name", &contact.company_name);
        ctx.insert("phone", &contact.phone);
        ctx.insert("message", &contact.message);

        let message = self
            .templates
            .render(CONTACT_US_EMAIL, &ctx)
            .map_err(|e| NodecosmosError::TemplateError(e.to_string()))?;

        self.send_email("goran@nodecosmos.com".to_string(), "New contact us request", message)
            .await?;

        Ok(())
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

        self.ses_client
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
