use crate::models::user::CurrentUser;
use charybdis::macros::charybdis_udt_model;
use charybdis::types::{Text, Uuid};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Serialize, Deserialize, Default, Debug)]
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
    pub fn init(current_user: &CurrentUser) -> Owner {
        Owner {
            id: current_user.id,
            name: current_user.full_name(),
            username: Some(current_user.username.clone()),
            owner_type: OwnerType::User.into(),
            profile_image_url: None,
        }
    }

    pub fn init_from(owner: &Self) -> Self {
        Self {
            id: owner.id,
            owner_type: owner.owner_type.clone(),
            name: owner.name.clone(),
            username: owner.username.clone(),
            profile_image_url: owner.profile_image_url.clone(),
        }
    }
}

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
