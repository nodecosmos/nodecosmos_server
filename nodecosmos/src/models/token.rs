use base64::engine::general_purpose::URL_SAFE;
use base64::Engine;
use charybdis::macros::charybdis_model;
use charybdis::types::{Frozen, Text, Timestamp, Tuple, Uuid};
use chrono::Utc;
use rand::rngs::OsRng;
use rand::TryRngCore;

#[derive(strum_macros::Display, strum_macros::EnumString, PartialEq)]
pub enum TokenType {
    UserConfirmation,
    PasswordReset,
    Invitation,
}

// Tokens can be created for user confirmation, password reset, or invitations.
// In case of invitations, we could use either the email or the username as the user identifier.
#[charybdis_model(
    table_name = tokens,
    partition_keys = [id],
    clustering_keys = [],
)]
pub struct Token {
    pub id: Text,
    pub email: Text,
    pub username: Option<Text>,
    pub token_type: Text,
    pub created_at: Timestamp,
    pub expires_at: Timestamp,
    pub node_pk: Option<Frozen<Tuple<Uuid, Uuid>>>,
}

impl Token {
    fn generate_token() -> String {
        let mut rng = OsRng;
        let mut random_bytes = [0u8; 32];
        rng.try_fill_bytes(&mut random_bytes)
            .expect("Failed to generate random bytes with OS RNG");
        URL_SAFE.encode(random_bytes)
    }

    pub fn new_user_confirmation(email: Text) -> Self {
        Self {
            id: Self::generate_token(),
            email,
            username: None,
            token_type: TokenType::UserConfirmation.to_string(),
            created_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::days(1),
            node_pk: None,
        }
    }

    pub fn new_password_reset(email: Text) -> Self {
        Self {
            id: Self::generate_token(),
            email,
            username: None,
            token_type: TokenType::PasswordReset.to_string(),
            created_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::hours(1),
            node_pk: None,
        }
    }

    pub fn new_invitation(email: Text, username: Option<Text>, node_pk: (Uuid, Uuid)) -> Self {
        Self {
            id: Self::generate_token(),
            email,
            username,
            token_type: TokenType::Invitation.to_string(),
            created_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::weeks(1),
            node_pk: Some(node_pk),
        }
    }
}
