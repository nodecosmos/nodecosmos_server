use crate::app::App;
use crate::errors::NodecosmosError;
use reqwest::Client;

pub async fn validate_recaptcha(app: &App, token: &Option<String>) -> Result<(), NodecosmosError> {
    if app.recaptcha_enabled {
        if let Some(r_token) = token {
            // https://www.google.com/recaptcha/api/siteverify
            let recaptcha_response = Client::new()
                .post("https://www.google.com/recaptcha/api/siteverify")
                .form(&[("secret", &app.recaptcha_secret), ("response", &r_token)])
                .send()
                .await
                .map_err(|e| {
                    log::error!("Error sending recaptcha request: {:?}", e);
                    NodecosmosError::BadRequest("Error sending recaptcha request".to_string())
                })?;

            // parse the response
            let recaptcha_response = recaptcha_response.json::<serde_json::Value>().await.map_err(|e| {
                log::error!("Error parsing recaptcha response: {:?}", e);
                NodecosmosError::BadRequest("Error parsing recaptcha response".to_string())
            })?;

            // check if the response is success
            if !recaptcha_response["success"].as_bool().unwrap_or(false) {
                return Err(NodecosmosError::BadRequest("Recaptcha failed".to_string()));
            }

            if recaptcha_response["score"].as_f64().unwrap_or(0.0) < 0.5 {
                return Err(NodecosmosError::BadRequest("Recaptcha score failed".to_string()));
            }
        } else {
            return Err(NodecosmosError::BadRequest("Missing rToken".to_string()));
        }
    }

    Ok(())
}
