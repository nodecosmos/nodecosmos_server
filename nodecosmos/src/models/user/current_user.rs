use crate::models::user::{partial_user, User};
use crate::utils::defaults::default_to_false;
use charybdis::types::{Boolean, Text, Uuid};
use serde::{Deserialize, Serialize};

partial_user!(
    CurrentUser,
    id,
    first_name,
    last_name,
    username,
    email,
    is_confirmed,
    is_blocked
);

impl CurrentUser {
    pub fn full_name(&self) -> String {
        format!("{} {}", self.first_name, self.last_name)
    }
}
