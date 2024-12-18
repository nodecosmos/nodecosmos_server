use crate::app::Config;
use crate::errors::NodecosmosError;
use crate::models::contact::Contact;
use crate::resources::email_client::EmailClient;
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
    pub client: EmailClient,
}

impl Mailer {
    pub fn new(client: EmailClient, config: &Config) -> Self {
        let mut templates = Handlebars::new();

        templates
            .register_template_string(CONFIRM_EMAIL, include_str!("mailer/confirm_email.html"))
            .expect("Template should be valid");

        templates
            .register_template_string(INVITATION_EMAIL, include_str!("mailer/invitation_email.html"))
            .expect("Template should be valid");

        templates
            .register_template_string(
                INVITATION_ACCEPTED_EMAIL,
                include_str!("mailer/invitation_accepted_email.html"),
            )
            .expect("Template should be valid");

        templates
            .register_template_string(RESET_PASSWORD_EMAIL, include_str!("mailer/reset_password_email.html"))
            .expect("Template should be valid");

        templates
            .register_template_string(
                PASSWORD_CHANGED_EMAIL,
                include_str!("mailer/password_changed_email.html"),
            )
            .expect("Template should be valid");

        templates
            .register_template_string(CONTACT_US_EMAIL, include_str!("mailer/contact_us_email.html"))
            .expect("Template should be valid");

        Self {
            templates,
            client_url: config.client_url.clone(),
            client,
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

        self.client
            .send_email(to, "Confirm your nodecosmos account", message)
            .await
    }

    pub async fn send_invitation_email(
        &self,
        to: String,
        username: &str,
        node_name: &str,
        token: String,
    ) -> Result<(), NodecosmosError> {
        let url = format!("{}/invitations?token={}", self.client_url, token);

        let mut ctx = HashMap::<&str, &str>::new();
        ctx.insert("username", username);
        ctx.insert("node_name", node_name);
        ctx.insert("invitation_url", &url);

        let message = self
            .templates
            .render(INVITATION_EMAIL, &ctx)
            .map_err(|e| NodecosmosError::TemplateError(e.to_string()))?;

        self.client
            .send_email(to, "You have been invited to collaborate on nodecosmos", message)
            .await
    }

    pub async fn send_invitation_accepted_email(
        &self,
        to: String,
        username: &str,
        branch_id: Uuid,
        node_id: Uuid,
        node_name: &str,
    ) -> Result<(), NodecosmosError> {
        let node_url = format!("{}/nodes/{}/{}", self.client_url, branch_id, node_id);

        let mut ctx = HashMap::<&str, &str>::new();
        ctx.insert("username", username);
        ctx.insert("node_name", node_name);
        ctx.insert("node_url", &node_url);

        let message = self
            .templates
            .render(INVITATION_ACCEPTED_EMAIL, &ctx)
            .map_err(|e| NodecosmosError::TemplateError(e.to_string()))?;

        self.client
            .send_email(to, "Your invitation has been accepted", message)
            .await
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

        self.client
            .send_email(to, "Reset your nodecosmos password", message)
            .await
    }

    pub async fn send_password_changed_email(&self, to: String, username: String) -> Result<(), NodecosmosError> {
        let mut ctx = HashMap::<&str, &str>::new();
        ctx.insert("username", &username);

        let message = self
            .templates
            .render(PASSWORD_CHANGED_EMAIL, &ctx)
            .map_err(|e| NodecosmosError::TemplateError(e.to_string()))?;

        self.client
            .send_email(to, "Your nodecosmos password has been changed", message)
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

        self.client
            .send_email(
                "goran@nodecosmos.com".to_string(),
                format!("New contact us request: ID {}", contact.id).as_str(),
                message,
            )
            .await?;

        Ok(())
    }
}
