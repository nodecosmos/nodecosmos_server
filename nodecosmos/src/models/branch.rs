use charybdis::macros::charybdis_model;
use charybdis::types::{Boolean, Map, Uuid};
use serde::{Deserialize, Serialize};

#[charybdis_model(
    table_name = branches,
    partition_keys = [id],
    clustering_keys = [],
)]
#[derive(Serialize, Deserialize, Default)]
pub struct Branch {
    id: Uuid,

    #[serde(rename = "nodeId")]
    node_id: Uuid,

    name: String,
    description: Option<String>,

    // nodes
    #[serde(default, rename = "createdNodesById")]
    created_nodes_by_id: Option<Map<Uuid, Boolean>>,

    #[serde(default, rename = "deletedNodesById")]
    deleted_nodes_by_id: Option<Map<Uuid, Boolean>>,

    #[serde(default, rename = "editedNodeTitlesById")]
    edited_titles_by_id: Option<Map<Uuid, Boolean>>,

    #[serde(default, rename = "editedNodeDescriptionsById")]
    edited_descriptions_by_id: Option<Map<Uuid, Boolean>>,

    #[serde(default, rename = "editedNodeTreePositionIdById")]
    edited_node_tree_position_id_by_id: Option<Map<Uuid, Boolean>>,

    // workflows
    #[serde(default, rename = "createdWorkflowsById")]
    created_workflows_by_id: Option<Map<Uuid, Boolean>>,

    #[serde(default, rename = "deletedWorkflowsById")]
    deleted_workflows_by_id: Option<Map<Uuid, Boolean>>,

    #[serde(default, rename = "editedWorkflowTitlesById")]
    edited_workflow_titles_by_id: Option<Map<Uuid, Boolean>>,

    // flows
    #[serde(default, rename = "createdFlowsById")]
    created_flows_by_id: Option<Map<Uuid, Boolean>>,

    #[serde(default, rename = "deletedFlowsById")]
    deleted_flows_by_id: Option<Map<Uuid, Boolean>>,

    #[serde(default, rename = "editedFlowTitlesById")]
    edited_flow_titles_by_id: Option<Map<Uuid, Boolean>>,

    #[serde(default, rename = "editedFlowDescriptionsById")]
    edited_flow_descriptions_by_id: Option<Map<Uuid, Boolean>>,

    // ios
    #[serde(default, rename = "createdIOsById")]
    created_ios_by_id: Option<Map<Uuid, Uuid>>,

    #[serde(default, rename = "deletedIOsById")]
    deleted_ios_by_id: Option<Map<Uuid, Uuid>>,

    #[serde(default, rename = "editedIOTitlesById")]
    edited_io_titles_by_id: Option<Map<Uuid, Boolean>>,

    #[serde(default, rename = "editedIODescriptionsById")]
    edited_io_descriptions_by_id: Option<Map<Uuid, Boolean>>,

    // flow steps
    #[serde(default, rename = "createdFlowStepsById")]
    created_flow_steps_by_id: Option<Map<Uuid, Boolean>>,

    #[serde(default, rename = "deletedFlowStepsById")]
    deleted_flow_steps_by_id: Option<Map<Uuid, Boolean>>,

    #[serde(default, rename = "editedFlowStepTitlesById")]
    created_flow_step_input_id_by_node_id: Option<Map<Uuid, Boolean>>,

    #[serde(default, rename = "editedFlowStepDescriptionsById")]
    deleted_flow_step_input_id_by_node_id: Option<Map<Uuid, Boolean>>,
}
