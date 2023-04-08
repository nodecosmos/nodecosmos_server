use charybdis::prelude::*;

#[charybdis_udt_model]
pub struct Address {
    pub street: Text,
    pub city: Text,
    pub state: Text,
    pub zip: Text,
}
