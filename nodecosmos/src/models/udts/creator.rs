use charybdis::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default)]
#[charybdis_udt_model(type_name = creator)]
pub struct Creator {
    pub id: Uuid,
    pub username: Text,
}
