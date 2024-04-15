use charybdis::macros::charybdis_udt_model;
use charybdis::types::{Double, Uuid};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default, PartialEq, Clone)]
#[charybdis_udt_model(type_name = BranchReorderData)]
#[serde(rename_all = "camelCase")]
pub struct BranchReorderData {
    pub id: Uuid,
    pub new_parent_id: Uuid,
    pub new_upper_sibling_id: Option<Uuid>,
    pub new_lower_sibling_id: Option<Uuid>,
    pub old_parent_id: Uuid,
    pub old_order_index: Double,
}
