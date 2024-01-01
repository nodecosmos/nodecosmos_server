use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::Branch;
use crate::models::node::find_pk_node;
use crate::models::node::PkNode;
use crate::utils::cloned_ref::ClonedRef;
use charybdis::macros::charybdis_udt_model;
use charybdis::operations::Update;
use charybdis::types::{Text, Uuid};
use serde::{Deserialize, Serialize};
use std::fmt::Display;

pub enum ConflictType {
    AncestorsDeleted,
    WorkflowDeleted,
    FlowDeleted,
    FlowStepDeleted,
    PrevFlowStepDeleted,
    NextFlowStepDeleted,
    PrevStepOutputDeleted,
    FlowStepNodeDeleted,
    NoPreviousFlow,
    NoInitialInput,
}

impl Display for ConflictType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConflictType::AncestorsDeleted => write!(f, "ANCESTORS_DELETED"),
            ConflictType::WorkflowDeleted => write!(f, "WORKFLOW_DELETED"),
            ConflictType::FlowDeleted => write!(f, "FLOW_DELETED"),
            ConflictType::FlowStepDeleted => write!(f, "FLOW_STEP_DELETED"),
            ConflictType::PrevFlowStepDeleted => write!(f, "PREV_FLOW_STEP_DELETED"),
            ConflictType::NextFlowStepDeleted => write!(f, "NEXT_FLOW_STEP_DELETED"),
            ConflictType::PrevStepOutputDeleted => write!(f, "PREV_STEP_OUTPUT_DELETED"),
            ConflictType::FlowStepNodeDeleted => write!(f, "FLOW_STEP_NODE_DELETED"),
            ConflictType::NoPreviousFlow => write!(f, "NO_PREVIOUS_FLOW"),
            ConflictType::NoInitialInput => write!(f, "NO_INITIAL_INPUT"),
        }
    }
}

pub enum ConflictStatus {
    Pending,
    Resolved,
}

impl Display for ConflictStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConflictStatus::Pending => write!(f, "PENDING"),
            ConflictStatus::Resolved => write!(f, "RESOLVED"),
        }
    }
}

#[derive(Serialize, Deserialize, Default, Eq, Hash, PartialEq)]
#[charybdis_udt_model(type_name = conflict)]
pub struct Conflict {
    pub object_id: Uuid,
    pub c_type: Text,
    pub status: Text,
}
