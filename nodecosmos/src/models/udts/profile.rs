use crate::models::user::{CurrentUser, FullName, User};
use charybdis::macros::charybdis_udt_model;
use charybdis::types::{Text, Uuid};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, strum_macros::Display, strum_macros::EnumString)]
pub enum ProfileType {
    User,
    Organization,
}

#[derive(Serialize, Deserialize, Clone)]
#[charybdis_udt_model(type_name = profile)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: Uuid,
    pub profile_type: Text, // user or organization
    pub name: Text,
    pub username: Option<Text>,
    pub profile_image_url: Option<Text>,
}

impl Profile {
    pub fn init(user: &User) -> Self {
        Self {
            id: user.id,
            profile_type: ProfileType::User.to_string(),
            name: user.full_name(),
            username: Some(user.username.clone()),
            profile_image_url: user.profile_image_url.clone(),
        }
    }

    pub fn init_from_current_user(current_user: &CurrentUser) -> Profile {
        Profile {
            id: current_user.id,
            name: current_user.full_name(),
            username: Some(current_user.username.clone()),
            profile_type: ProfileType::User.to_string(),
            profile_image_url: current_user.profile_image_url.clone(),
        }
    }
}
