use charybdis::macros::charybdis_udt_model;
use charybdis::types::{Double, Uuid};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default, PartialEq, Clone)]
#[charybdis_udt_model(type_name = BranchReorderData)]
pub struct BranchReorderData {
    pub id: Uuid,
    #[serde(rename = "newParentId")]
    pub new_parent_id: Uuid,

    #[serde(rename = "newOrderIndex")]
    pub new_upper_sibling_id: Option<Uuid>,

    #[serde(rename = "newLowerSiblingId")]
    pub new_lower_sibling_id: Option<Uuid>,

    #[serde(rename = "oldParentId")]
    pub old_parent_id: Uuid,

    #[serde(rename = "oldOrderIndex")]
    pub old_order_index: Double,
}
