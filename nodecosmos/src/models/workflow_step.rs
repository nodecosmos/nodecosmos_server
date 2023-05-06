use crate::models::helpers::{impl_default_callbacks, set_updated_at_cb};
use charybdis::*;
use chrono::Utc;

#[partial_model_generator]
#[charybdis_model(
    table_name = "workflow_steps",
    partition_keys = ["workflow_id"],
    clustering_keys = ["id"],
    secondary_indexes = []
)]
pub struct WorkflowStep {
    #[serde(rename = "workflowId")]
    pub workflow_id: Uuid,
    pub id: Uuid,
    pub title: Text,
    pub description: Text,

    #[serde(rename = "nodeId")]
    pub node_id: Uuid,

    #[serde(rename = "inputIds")]
    pub input_ids: Option<Set<Uuid>>,

    #[serde(rename = "outputIds")]
    pub output_ids: Option<Set<Uuid>>,

    #[serde(rename = "createdAt")]
    pub created_at: Option<Timestamp>,

    #[serde(rename = "updatedAt")]
    pub updated_at: Option<Timestamp>,

    #[serde(rename = "nextWorkflowStepByOutputId")]
    pub next_workflow_step: Option<Map<Uuid, Uuid>>,
}

impl_default_callbacks!(WorkflowStep);

partial_workflow_step!(
    UpdateWorkflowStepTitle,
    workflow_id,
    id,
    node_id,
    title,
    updated_at
);
set_updated_at_cb!(UpdateWorkflowStepTitle);

partial_workflow_step!(
    UpdateWorkflowStepDescription,
    workflow_id,
    id,
    node_id,
    description,
    updated_at
);
set_updated_at_cb!(UpdateWorkflowStepDescription);

partial_workflow_step!(
    UpdateWorkflowStepInputIds,
    workflow_id,
    id,
    node_id,
    input_ids,
    updated_at
);
set_updated_at_cb!(UpdateWorkflowStepInputIds);

partial_workflow_step!(
    UpdateWorkflowStepOutputIds,
    workflow_id,
    id,
    node_id,
    output_ids,
    updated_at
);
set_updated_at_cb!(UpdateWorkflowStepOutputIds);
