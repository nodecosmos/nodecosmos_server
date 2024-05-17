use base64::engine::general_purpose::URL_SAFE;
use base64::Engine;
use charybdis::macros::charybdis_model;
use charybdis::types::{Text, Timestamp, Uuid};
use chrono::Utc;
use rand::rngs::OsRng;
use rand::Rng;

#[derive(strum_macros::Display, strum_macros::EnumString)]
pub enum TokenType {
    UserConfirmation,
    PasswordReset,
    Invitation,
}

#[charybdis_model(
    table_name = tokens,
    partition_keys = [token],
    clustering_keys = [],
    global_secondary_indexes = [user_id, expires_at],
)]
pub struct Token {
    pub token: Text,
    pub user_id: Uuid,
    pub token_type: Text,
    pub created_at: Timestamp,
    pub expires_at: Timestamp,
}

impl Token {
    fn generate_token() -> String {
        let mut rng = OsRng::default();
        let random_bytes: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
        return URL_SAFE.encode(&random_bytes);
    }

    pub fn new_user_confirmation(user_id: Uuid) -> Self {
        Self {
            token: Self::generate_token(),
            user_id,
            token_type: TokenType::UserConfirmation.to_string(),
            created_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::days(1),
        }
    }
}
