use charybdis::macros::charybdis_udt_model;
use charybdis::types::{Int, Text};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default)]
#[charybdis_udt_model(type_name = comment_data)]
pub struct CommentData {
    line_number: Option<Int>,
}
