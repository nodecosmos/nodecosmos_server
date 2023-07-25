use charybdis::*;
use std::fmt;

#[charybdis_udt_model(type_name = owner)]
pub struct Owner {
    pub id: Uuid,
    pub owner_type: Text, // user or organization
    pub name: Text,
}

#[derive(Debug, Deserialize)]
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
