use charybdis::macros::charybdis_udt_model;
use charybdis::types::Uuid;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default, PartialEq, Eq, Hash)]
#[charybdis_udt_model(type_name = BranchReorderData)]
pub struct BranchReorderData {
    pub id: Uuid,
    pub new_parent_id: Uuid,
    pub new_upper_sibling_id: Option<Uuid>,
    pub new_lower_sibling_id: Option<Uuid>,
}
