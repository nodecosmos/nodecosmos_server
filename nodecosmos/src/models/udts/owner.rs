use charybdis::*;

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

impl OwnerTypes {
    pub fn to_string(&self) -> String {
        match self {
            OwnerTypes::User => "User".to_string(),
            OwnerTypes::Organization => "Organization".to_string(),
        }
    }

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
