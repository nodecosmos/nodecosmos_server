use base64::engine::general_purpose::URL_SAFE;
use base64::Engine;
use charybdis::macros::charybdis_model;
use charybdis::types::{Text, Timestamp, Tuple, Uuid};
use chrono::Utc;
use rand::rngs::OsRng;
use rand::Rng;

#[derive(strum_macros::Display, strum_macros::EnumString, PartialEq)]
pub enum TokenType {
    UserConfirmation,
    PasswordReset,
    Invitation,
}

#[charybdis_model(
    table_name = tokens,
    partition_keys = [id],
    clustering_keys = [],
)]
pub struct Token {
    pub id: Text,
    pub email: Text,
    pub token_type: Text,
    pub created_at: Timestamp,
    pub expires_at: Timestamp,
    pub node_pk: Option<Tuple<Uuid, Uuid>>,
}

impl Token {
    fn generate_token() -> String {
        let mut rng = OsRng::default();
        let random_bytes: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
        return URL_SAFE.encode(&random_bytes);
    }

    pub fn new_user_confirmation(email: Text) -> Self {
        Self {
            id: Self::generate_token(),
            email,
            token_type: TokenType::UserConfirmation.to_string(),
            created_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::days(1),
            node_pk: None,
        }
    }

    pub fn new_invitation(email: Text, node_pk: (Uuid, Uuid)) -> Self {
        Self {
            id: Self::generate_token(),
            email,
            token_type: TokenType::Invitation.to_string(),
            created_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::weeks(1),
            node_pk: Some(node_pk),
        }
    }
}
