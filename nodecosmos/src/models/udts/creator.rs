use charybdis::prelude::*;

#[charybdis_udt_model(type_name = "creator")]
pub struct Creator {
    pub id: Uuid,
    pub username: Text,
}
