use charybdis::prelude::*;

#[charybdis_udt_model(type_name = "owner")]
pub struct Owner {
    pub id: Uuid,
    pub username: Text,
}
