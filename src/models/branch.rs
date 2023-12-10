pub mod branchable;

use crate::models::udts::Owner;
use charybdis::macros::charybdis_model;
use charybdis::types::{Boolean, Map, Set, Text, Uuid};
use serde::{Deserialize, Serialize};

#[charybdis_model(
    table_name = branches,
    partition_keys = [id],
    clustering_keys = [],
)]
#[derive(Serialize, Deserialize, Default, Clone)]
pub struct Branch {
    pub id: Uuid,

    pub title: Option<Text>,
    pub description: Option<Text>,

    #[serde(rename = "ownerId")]
    pub owner_id: Uuid,

    pub owner: Option<Owner>,

    #[serde(rename = "editorIds")]
    pub editor_ids: Option<Set<Uuid>>,

    #[serde(rename = "isPublic")]
    pub is_public: Boolean,

    #[serde(rename = "isContributionRequest")]
    pub is_contribution_request: Option<Boolean>,

    // nodes
    #[serde(default, rename = "createdNodesById")]
    pub created_nodes_by_id: Option<Map<Uuid, Boolean>>,

    #[serde(default, rename = "deletedNodesById")]
    pub deleted_nodes_by_id: Option<Map<Uuid, Boolean>>,

    #[serde(default, rename = "editedNodeTitlesById")]
    pub edited_node_titles_by_id: Option<Map<Uuid, Boolean>>,

    #[serde(default, rename = "editedNodeDescriptionsById")]
    pub edited_node_descriptions_by_id: Option<Map<Uuid, Boolean>>,

    #[serde(default, rename = "editedNodeTreePositionsById")]
    pub edited_node_tree_positions_by_id: Option<Map<Uuid, Boolean>>,

    // workflows
    #[serde(default, rename = "createdWorkflowsById")]
    pub created_workflows_by_id: Option<Map<Uuid, Boolean>>,

    #[serde(default, rename = "deletedWorkflowsById")]
    pub deleted_workflows_by_id: Option<Map<Uuid, Boolean>>,

    #[serde(default, rename = "editedWorkflowTitlesById")]
    pub edited_workflow_titles_by_id: Option<Map<Uuid, Boolean>>,

    // flows
    #[serde(default, rename = "createdFlowsById")]
    pub created_flows_by_id: Option<Map<Uuid, Boolean>>,

    #[serde(default, rename = "deletedFlowsById")]
    pub deleted_flows_by_id: Option<Map<Uuid, Boolean>>,

    #[serde(default, rename = "editedFlowTitlesById")]
    pub edited_flow_titles_by_id: Option<Map<Uuid, Boolean>>,

    #[serde(default, rename = "editedFlowDescriptionsById")]
    pub edited_flow_descriptions_by_id: Option<Map<Uuid, Boolean>>,

    // ios
    #[serde(default, rename = "createdIOsById")]
    pub created_ios_by_id: Option<Map<Uuid, Uuid>>,

    #[serde(default, rename = "deletedIOsById")]
    pub deleted_ios_by_id: Option<Map<Uuid, Uuid>>,

    #[serde(default, rename = "editedIOTitlesById")]
    pub edited_io_titles_by_id: Option<Map<Uuid, Boolean>>,

    #[serde(default, rename = "editedIODescriptionsById")]
    pub edited_io_descriptions_by_id: Option<Map<Uuid, Boolean>>,

    // flow steps
    #[serde(default, rename = "createdFlowStepsById")]
    pub created_flow_steps_by_id: Option<Map<Uuid, Boolean>>,

    #[serde(default, rename = "deletedFlowStepsById")]
    pub deleted_flow_steps_by_id: Option<Map<Uuid, Boolean>>,

    #[serde(default, rename = "editedFlowStepTitlesById")]
    pub created_flow_step_input_id_by_node_id: Option<Map<Uuid, Boolean>>,

    #[serde(default, rename = "editedFlowStepDescriptionsById")]
    pub deleted_flow_step_input_id_by_node_id: Option<Map<Uuid, Boolean>>,
}

partial_branch!(AuthBranch, id, owner_id, editor_ids, is_public);

partial_branch!(UpdateCreatedNodesBranch, id, created_nodes_by_id);

partial_branch!(UpdateDeletedNodesBranch, id, deleted_nodes_by_id);

partial_branch!(UpdateEditedNodeTitlesBranch, id, edited_node_titles_by_id);

partial_branch!(UpdateEditedNodeDescriptionsBranch, id, edited_node_descriptions_by_id);

partial_branch!(
    UpdateEditedNodeTreePositionIdBranch,
    id,
    edited_node_tree_positions_by_id
);

partial_branch!(UpdateCreatedWorkflowsBranch, id, created_workflows_by_id);

partial_branch!(UpdateDeletedWorkflowsBranch, id, deleted_workflows_by_id);

partial_branch!(UpdateEditedWorkflowTitlesBranch, id, edited_workflow_titles_by_id);

partial_branch!(UpdateCreatedFlowsBranch, id, created_flows_by_id);

partial_branch!(UpdateDeletedFlowsBranch, id, deleted_flows_by_id);

partial_branch!(UpdateEditedFlowTitlesBranch, id, edited_flow_titles_by_id);

partial_branch!(UpdateEditedFlowDescriptionsBranch, id, edited_flow_descriptions_by_id);

partial_branch!(UpdateCreatedIOsBranch, id, created_ios_by_id);

partial_branch!(UpdateDeletedIOsBranch, id, deleted_ios_by_id);

partial_branch!(UpdateEditedIOTitlesBranch, id, edited_io_titles_by_id);

partial_branch!(UpdateEditedIODescriptionsBranch, id, edited_io_descriptions_by_id);

partial_branch!(UpdateCreatedFlowStepsBranch, id, created_flow_steps_by_id);

partial_branch!(UpdateDeletedFlowStepsBranch, id, deleted_flow_steps_by_id);
