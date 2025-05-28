use charybdis::macros::charybdis_udt_model;
use charybdis::types::{Text, Uuid};
use serde::{Deserialize, Serialize};

use crate::models::user::{CurrentUser, FullName, User};

#[derive(Deserialize, strum_macros::Display, strum_macros::EnumString)]
pub enum ProfileType {
    User,
    Organization,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
#[charybdis_udt_model(type_name = profile)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: Uuid,
    pub profile_type: Text,
    // user or organization
    pub name: Text,
    pub username: Option<Text>,
    pub profile_image_url: Option<Text>,
}

impl From<&User> for Profile {
    fn from(user: &User) -> Self {
        Self {
            id: user.id,
            name: user.full_name(),
            username: Some(user.username.clone()),
            profile_type: ProfileType::User.to_string(),
            profile_image_url: user.profile_image_url.clone(),
        }
    }
}

impl From<&CurrentUser> for Profile {
    fn from(current_user: &CurrentUser) -> Self {
        Self {
            id: current_user.id,
            name: current_user.full_name(),
            username: Some(current_user.username.clone()),
            profile_type: ProfileType::User.to_string(),
            profile_image_url: current_user.profile_image_url.clone(),
        }
    }
}
