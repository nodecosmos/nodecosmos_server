use charybdis::macros::charybdis_udt_model;
use charybdis::types::{Frozen, Set, Text, Uuid};
use serde::{Deserialize, Serialize};

#[derive(Eq, PartialEq, Default, strum_macros::Display, strum_macros::EnumString)]
pub enum ConflictStatus {
    #[default]
    Pending,

    Resolved,
}

#[derive(Serialize, Deserialize, Default, Eq, PartialEq, Clone)]
#[charybdis_udt_model(type_name = conflict)]
pub struct Conflict {
    pub status: Text,

    #[serde(rename = "deletedAncestors")]
    pub deleted_ancestors: Option<Frozen<Set<Uuid>>>,

    #[serde(rename = "deletedEditedNodes")]
    pub deleted_edited_nodes: Option<Frozen<Set<Uuid>>>,

    #[serde(rename = "deletedEditedFlows")]
    pub deleted_edited_flows: Option<Frozen<Set<Uuid>>>,

    #[serde(rename = "deletedEditedFlowSteps")]
    pub deleted_edited_flow_steps: Option<Frozen<Set<Uuid>>>,

    #[serde(rename = "deletedEditedFlowStepItems")]
    pub deleted_edited_ios: Option<Frozen<Set<Uuid>>>,

    #[serde(rename = "divergedFlows")]
    pub diverged_flows: Option<Frozen<Set<Uuid>>>,
}
