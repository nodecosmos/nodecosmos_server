use charybdis::macros::charybdis_model;
use charybdis::types::{Text, Timestamp, Uuid};
use chrono::Utc;

#[derive(strum_macros::Display, strum_macros::EnumString)]
pub enum TokenType {
    UserConfirmation,
    PasswordReset,
    Invitation,
}

#[charybdis_model(
    table_name = tokens,
    partition_keys = [id],
    clustering_keys = [],
    global_secondary_indexes = [user_id, expires_at],
)]
pub struct Token {
    pub id: Uuid,
    pub user_id: Uuid,
    pub token_type: Text,
    pub created_at: Timestamp,
    pub expires_at: Timestamp,
}

impl Token {
    pub fn new_user_confirmation(user_id: Uuid) -> Self {
        Self {
            id: Uuid::new_v4(),
            user_id,
            token_type: TokenType::UserConfirmation.to_string(),
            created_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::days(1),
        }
    }
}
