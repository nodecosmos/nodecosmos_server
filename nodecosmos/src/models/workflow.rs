use crate::errors::NodecosmosError;
use crate::models::flow::FlowDelete;
use crate::models::flow_step::FlowStepDelete;
use crate::models::helpers::{created_at_cb_fn, impl_updated_at_cb, updated_at_cb_fn};
use crate::models::input_output::IoDelete;
use charybdis::batch::CharybdisModelBatch;
use charybdis::callbacks::Callbacks;
use charybdis::errors::CharybdisError;
use charybdis::macros::charybdis_model;
use charybdis::types::{List, Text, Timestamp, Uuid};
use scylla::CachingSession;
use serde::{Deserialize, Serialize};

///
/// Workflow model
///
/// Currently we only support one workflow per node,
/// in future we will support multiple workflows per node.
///
/// Single workflow can have multiple flows.
/// Flow represents isolated process within workflow.
/// Single flow can have many flow steps.
/// Flow step contains inputs, nodes and outputs.
///
/// In that sense Workflow is a collection of flows.
///
/// In future we will allow multiple workflows per node.
/// Reasoning is that we want to allow users to describe multiple processes.

#[charybdis_model(
    table_name = workflows,
    partition_keys = [node_id],
    clustering_keys = [id],
    secondary_indexes = []
)]
#[derive(Serialize, Deserialize, Default)]
pub struct Workflow {
    #[serde(rename = "nodeId")]
    pub node_id: Uuid,

    #[serde(rename = "rootNodeId")]
    pub root_node_id: Uuid,

    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,
    pub title: Option<Text>,
    pub description: Option<Text>,
    pub description_markdown: Option<Text>,

    #[serde(rename = "createdAt")]
    pub created_at: Option<Timestamp>,

    #[serde(rename = "updatedAt")]
    pub updated_at: Option<Timestamp>,

    #[serde(rename = "initialInputIds")]
    pub initial_input_ids: Option<List<Uuid>>,

    #[serde(rename = "flowIds")]
    pub flow_ids: Option<List<Uuid>>,
}

impl Workflow {
    pub async fn append_flow_id(&mut self, session: &CachingSession, flow_id: Uuid) -> Result<(), CharybdisError> {
        self.push_to_flow_ids(session, &vec![flow_id]).await?;

        Ok(())
    }

    pub async fn pull_flow_id(&mut self, session: &CachingSession, flow_id: Uuid) -> Result<(), CharybdisError> {
        self.pull_from_flow_ids(session, &vec![flow_id]).await?;

        Ok(())
    }

    pub async fn pull_initial_input_id(&mut self, session: &CachingSession, id: Uuid) -> Result<(), CharybdisError> {
        self.pull_from_initial_input_ids(session, &vec![id]).await?;

        Ok(())
    }
}

impl Callbacks<NodecosmosError> for Workflow {
    created_at_cb_fn!();

    updated_at_cb_fn!();

    async fn after_delete(&mut self, session: &CachingSession) -> Result<(), NodecosmosError> {
        if self.flow_ids.is_some() {
            let flow_steps = FlowStepDelete::find_by_node_id_and_workflow_id(session, self.node_id, self.id)
                .await?
                .try_collect()
                .await?;

            let flows = FlowDelete::find_by_node_id_and_workflow_id(session, self.node_id, self.id)
                .await?
                .try_collect()
                .await?;

            let input_outputs = IoDelete::find_by_node_id_and_workflow_id(session, self.node_id, self.id)
                .await?
                .try_collect()
                .await?;

            CharybdisModelBatch::chunked_delete(session, &flow_steps, 100).await?;
            CharybdisModelBatch::chunked_delete(session, &flows, 100).await?;
            CharybdisModelBatch::chunked_delete(session, &input_outputs, 100).await?;
        } else if self.initial_input_ids.is_some() {
            let input_outputs = IoDelete::find_by_node_id_and_workflow_id(session, self.node_id, self.id)
                .await?
                .try_collect()
                .await?;

            CharybdisModelBatch::chunked_delete(session, &input_outputs, 100).await?;
        }

        Ok(())
    }
}

partial_workflow!(UpdateInitialInputsWorkflow, node_id, id, initial_input_ids, updated_at);
impl_updated_at_cb!(UpdateInitialInputsWorkflow);

// used by node deletion
partial_workflow!(WorkflowDelete, node_id, id);

partial_workflow!(UpdateWorkflowTitle, node_id, id, title, updated_at);
impl_updated_at_cb!(UpdateWorkflowTitle);
