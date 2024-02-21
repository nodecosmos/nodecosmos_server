use crate::models::user::{CurrentUser, FullName, User};
use charybdis::macros::charybdis_udt_model;
use charybdis::types::{Text, Uuid};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Deserialize)]
pub enum ProfileType {
    User,
    Organization,
}

impl fmt::Display for ProfileType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProfileType::User => write!(f, "user"),
            ProfileType::Organization => write!(f, "organization"),
        }
    }
}

impl ProfileType {
    pub fn from_string(s: &str) -> Self {
        match s {
            "User" => ProfileType::User,
            "Organization" => ProfileType::Organization,
            _ => panic!("Invalid owner type"),
        }
    }
}

impl From<ProfileType> for Text {
    fn from(profile_type: ProfileType) -> Self {
        profile_type.to_string()
    }
}

#[derive(Serialize, Deserialize, Default)]
#[charybdis_udt_model(type_name = profile)]
pub struct Profile {
    pub id: Uuid,

    #[serde(rename = "profileType")]
    pub profile_type: Text, // user or organization

    pub name: Text,

    pub username: Option<Text>,

    #[serde(rename = "profileImageURL")]
    pub profile_image_url: Option<Text>,
}

impl Profile {
    pub fn init(user: &User) -> Self {
        Self {
            id: user.id,
            profile_type: ProfileType::User.into(),
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
            profile_type: ProfileType::User.into(),
            profile_image_url: current_user.profile_image_url.clone(),
        }
    }
}
