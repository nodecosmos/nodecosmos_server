use charybdis::macros::charybdis_udt_model;
use charybdis::types::Text;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, strum_macros::Display, strum_macros::EnumString)]
pub enum PropDataTypes {
    Node,
    Text,
    Number,
    Boolean,
    Date,
    Time,
    DateTime,
}

#[derive(Serialize, Deserialize, Default)]
#[charybdis_udt_model(type_name = property)]
#[serde(rename_all = "camelCase")]
pub struct Property {
    pub title: Text,
    pub data_type: Text,
    pub value: Text,
}
