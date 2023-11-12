mod callbacks;
pub mod diagram;
mod update;

use crate::errors::NodecosmosError;
use crate::models::flow::Flow;
use crate::models::workflow::diagram::WorkflowDiagram;
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
#[derive(Serialize, Deserialize, Default, Clone)]
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

    #[charybdis(ignore)]
    #[serde(skip)]
    pub diagram: Option<WorkflowDiagram>,
}

impl Workflow {
    pub async fn by_node_id_and_id(
        session: &CachingSession,
        node_id: Uuid,
        id: Uuid,
    ) -> Result<Workflow, NodecosmosError> {
        let workflow = find_one_workflow!(session, "node_id = ? AND id = ?", (node_id, id)).await?;

        Ok(workflow)
    }

    pub async fn diagram(&mut self, session: &CachingSession) -> Result<&mut WorkflowDiagram, NodecosmosError> {
        if self.diagram.is_none() {
            let diagram = WorkflowDiagram::build(session, self).await?;
            self.diagram = Some(diagram);
        }

        Ok(self.diagram.as_mut().unwrap())
    }

    pub async fn flows(&self, session: &CachingSession) -> Result<CharybdisModelStream<Flow>, NodecosmosError> {
        let flows = Flow::find_by_node_id_and_workflow_id(session, self.node_id, self.id).await?;

        Ok(flows)
    }
}

partial_workflow!(UpdateInitialInputsWorkflow, node_id, id, initial_input_ids, updated_at);

// used by node deletion
partial_workflow!(WorkDeleteFlow, node_id, id);

partial_workflow!(UpdateWorkflowTitle, node_id, id, title, updated_at);
