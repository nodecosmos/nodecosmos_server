use charybdis::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default)]
#[charybdis_udt_model(type_name = address)]
pub struct Address {
    pub street: Text,
    pub city: Text,
    pub state: Text,
    pub zip: Text,
    pub country: Text,
}
