use charybdis::macros::charybdis_udt_model;
use charybdis::types::{Text, Uuid};
use serde::{Deserialize, Serialize};
use std::fmt;

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
pub enum OwnerTypes {
    User,
    Organization,
}

impl fmt::Display for OwnerTypes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OwnerTypes::User => write!(f, "User"),
            OwnerTypes::Organization => write!(f, "Organization"),
        }
    }
}

impl OwnerTypes {
    pub fn from_string(s: &str) -> Self {
        match s {
            "User" => OwnerTypes::User,
            "Organization" => OwnerTypes::Organization,
            _ => panic!("Invalid owner type"),
        }
    }
}

impl From<OwnerTypes> for Text {
    fn from(owner_type: OwnerTypes) -> Self {
        owner_type.to_string()
    }
}
