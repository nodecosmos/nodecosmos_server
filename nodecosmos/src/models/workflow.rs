use crate::errors::NodecosmosError;
use crate::models::flow::{DeleteFlow, Flow};
use crate::models::flow_step::DeleteFlowStep;
use crate::models::helpers::{created_at_cb_fn, impl_updated_at_cb, updated_at_cb_fn};
use crate::models::input_output::DeleteInputOutput;
use charybdis::callbacks::Callbacks;
use charybdis::macros::charybdis_model;
use charybdis::stream::CharybdisModelStream;
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
    global_secondary_indexes = []
)]
#[derive(Serialize, Deserialize, Default)]
pub struct Workflow {
    #[serde(rename = "nodeId")]
    pub node_id: Uuid,

    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,

    #[serde(rename = "rootNodeId")]
    pub root_node_id: Uuid,

    pub title: Option<Text>,
    pub description: Option<Text>,
    pub description_markdown: Option<Text>,

    #[serde(rename = "createdAt")]
    pub created_at: Option<Timestamp>,

    #[serde(rename = "updatedAt")]
    pub updated_at: Option<Timestamp>,

    #[serde(rename = "initialInputIds")]
    pub initial_input_ids: Option<List<Uuid>>,
}

impl Workflow {
    pub async fn flows(&self, session: &CachingSession) -> Result<CharybdisModelStream<Flow>, NodecosmosError> {
        let flows = Flow::find_by_node_id_and_workflow_id(session, self.node_id, self.id).await?;

        Ok(flows)
    }

    pub async fn pull_initial_input_id(&mut self, session: &CachingSession, id: Uuid) -> Result<(), NodecosmosError> {
        self.pull_from_initial_input_ids(session, &vec![id]).await?;

        Ok(())
    }
}

impl Callbacks<NodecosmosError> for Workflow {
    created_at_cb_fn!();

    updated_at_cb_fn!();

    async fn after_delete(&self, session: &CachingSession) -> Result<(), NodecosmosError> {
        DeleteFlowStep::delete_by_node_id_and_workflow_id(session, self.node_id, self.id).await?;
        DeleteFlow::delete_by_node_id_and_workflow_id(session, self.node_id, self.id).await?;

        DeleteInputOutput::delete_by_root_node_id_and_node_id(session, self.root_node_id, self.node_id).await?;
        DeleteInputOutput::delete_by_root_node_id_and_node_id(session, self.root_node_id, self.node_id)
            .await?
            .try_collect()
            .await?;

        Ok(())
    }
}

partial_workflow!(UpdateInitialInputsWorkflow, node_id, id, initial_input_ids, updated_at);
impl_updated_at_cb!(UpdateInitialInputsWorkflow);

// used by node deletion
partial_workflow!(WorkDeleteFlow, node_id, id);

partial_workflow!(UpdateWorkflowTitle, node_id, id, title, updated_at);
impl_updated_at_cb!(UpdateWorkflowTitle);
