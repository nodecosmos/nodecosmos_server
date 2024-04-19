use charybdis::macros::charybdis_udt_model;
use charybdis::types::{Frozen, Set, Uuid};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default, Eq, PartialEq, Clone)]
#[charybdis_udt_model(type_name = conflict)]
#[serde(rename_all = "camelCase")]
pub struct Conflict {
    pub deleted_ancestors: Option<Frozen<Set<Uuid>>>,
    pub deleted_edited_nodes: Option<Frozen<Set<Uuid>>>,
    pub deleted_edited_flows: Option<Frozen<Set<Uuid>>>,
    pub deleted_edited_flow_steps: Option<Frozen<Set<Uuid>>>,
    pub deleted_edited_ios: Option<Frozen<Set<Uuid>>>,
    pub conflicting_flow_steps: Option<Frozen<Set<Uuid>>>,
}
