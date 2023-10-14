use charybdis::macros::charybdis_udt_model;
use charybdis::types::Text;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default)]
#[charybdis_udt_model(type_name = property)]
pub struct Property {
    pub title: Text,
    pub data_type: Text,
    pub value: Text,
}

#[derive(Debug, Deserialize)]
pub enum PropDataTypes {
    Node,
    Text,
    Number,
    Boolean,
    Date,
    Time,
    DateTime,
}
//
// impl PropDataTypes {
//     pub fn to_string(&self) -> String {
//         match self {
//             PropDataTypes::Node => "Node".to_string(),
//             PropDataTypes::Text => "Text".to_string(),
//             PropDataTypes::Number => "Number".to_string(),
//             PropDataTypes::Boolean => "Boolean".to_string(),
//             PropDataTypes::Date => "Date".to_string(),
//             PropDataTypes::Time => "Time".to_string(),
//             PropDataTypes::DateTime => "DateTime".to_string(),
//         }
//     }
//
//     pub fn from_string(s: &str) -> Self {
//         match s {
//             "Node" => PropDataTypes::Node,
//             "Text" => PropDataTypes::Text,
//             "Number" => PropDataTypes::Number,
//             "Boolean" => PropDataTypes::Boolean,
//             "Date" => PropDataTypes::Date,
//             "Time" => PropDataTypes::Time,
//             "DateTime" => PropDataTypes::DateTime,
//             _ => PropDataTypes::Text,
//         }
//     }
// }
