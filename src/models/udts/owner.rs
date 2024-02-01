use crate::models::user::{CurrentUser, FullName, User};
use charybdis::macros::charybdis_udt_model;
use charybdis::types::{Text, Uuid};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Deserialize)]
pub enum OwnerType {
    User,
    Organization,
}

impl fmt::Display for OwnerType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OwnerType::User => write!(f, "user"),
            OwnerType::Organization => write!(f, "organization"),
        }
    }
}

impl OwnerType {
    pub fn from_string(s: &str) -> Self {
        match s {
            "User" => OwnerType::User,
            "Organization" => OwnerType::Organization,
            _ => panic!("Invalid owner type"),
        }
    }
}

impl From<OwnerType> for Text {
    fn from(owner_type: OwnerType) -> Self {
        owner_type.to_string()
    }
}

#[derive(Serialize, Deserialize, Default)]
#[charybdis_udt_model(type_name = owner)]
pub struct Owner {
    pub id: Uuid,

    #[serde(rename = "ownerType")]
    pub owner_type: Text, // user or organization

    pub name: Text,

    pub username: Option<Text>,

    #[serde(rename = "profileImageURL")]
    pub profile_image_url: Option<Text>,
}

impl Owner {
    pub fn init(user: &User) -> Self {
        Self {
            id: user.id,
            owner_type: OwnerType::User.into(),
            name: user.full_name(),
            username: Some(user.username.clone()),
            profile_image_url: user.profile_image_url.clone(),
        }
    }

    pub fn init_from_current_user(current_user: &CurrentUser) -> Owner {
        Owner {
            id: current_user.id,
            name: current_user.full_name(),
            username: Some(current_user.username.clone()),
            owner_type: OwnerType::User.into(),
            profile_image_url: current_user.profile_image_url.clone(),
        }
    }
}
