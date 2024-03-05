use charybdis::macros::charybdis_udt_model;
use charybdis::types::{Frozen, Set, Text, Uuid};
use serde::{Deserialize, Serialize};
use std::fmt;

pub enum ConflictStatus {
    Pending,
    Resolved,
}

impl fmt::Display for ConflictStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConflictStatus::Pending => write!(f, "Pending"),
            ConflictStatus::Resolved => write!(f, "Resolved"),
        }
    }
}

#[derive(Serialize, Deserialize, Default, Eq, PartialEq)]
#[charybdis_udt_model(type_name = conflict)]
pub struct Conflict {
    pub status: Text,

    #[serde(rename = "deletedAncestors")]
    pub deleted_ancestors: Option<Frozen<Set<Uuid>>>,

    #[serde(rename = "deletedEditedNodes")]
    pub deleted_edited_nodes: Option<Frozen<Set<Uuid>>>,
}
