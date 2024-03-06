mod callbacks;
pub mod diagram;

use crate::errors::NodecosmosError;
use crate::models::flow::Flow;
use crate::models::node::Node;
use crate::models::workflow::diagram::WorkflowDiagram;
use charybdis::macros::charybdis_model;
use charybdis::stream::CharybdisModelStream;
use charybdis::types::{List, Text, Timestamp, Uuid};
use scylla::CachingSession;
use serde::{Deserialize, Serialize};

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

    #[serde(rename = "descriptionMarkdown")]
    pub description_markdown: Option<Text>,

    #[serde(rename = "descriptionBase64")]
    pub description_base64: Option<Text>,

    #[serde(rename = "createdAt")]
    pub created_at: Option<Timestamp>,

    #[serde(rename = "updatedAt")]
    pub updated_at: Option<Timestamp>,

    #[serde(rename = "initialInputIds")]
    pub initial_input_ids: Option<List<Uuid>>,

    #[charybdis_model(ignore)]
    #[serde(skip)]
    pub diagram: Option<WorkflowDiagram>,

    #[charybdis_model(ignore)]
    #[serde(skip)]
    pub node: Option<Node>,
}

impl Workflow {
    pub async fn by_node_id_and_id(
        session: &CachingSession,
        node_id: Uuid,
        id: Uuid,
    ) -> Result<Workflow, NodecosmosError> {
        let workflow = find_first_workflow!("node_id = ? AND id = ?", (node_id, id))
            .execute(session)
            .await?;

        Ok(workflow)
    }

    pub async fn diagram(&mut self, session: &CachingSession) -> Result<&mut WorkflowDiagram, NodecosmosError> {
        if self.diagram.is_none() {
            let diagram = WorkflowDiagram::build(session, self).await?;
            self.diagram = Some(diagram);
        }

        Ok(self.diagram.as_mut().unwrap())
    }

    pub async fn init_node(&mut self, session: &CachingSession) -> Result<(), NodecosmosError> {
        // TODO: introduce branch_id to workflow
        let node = Node::find_by_id_and_branch_id(self.node_id, self.node_id)
            .execute(session)
            .await?;
        self.node = Some(node);

        Ok(())
    }

    pub async fn node(&mut self, session: &CachingSession) -> Result<&mut Node, NodecosmosError> {
        if self.node.is_none() {
            self.init_node(session).await?;
        }

        Ok(self.node.as_mut().unwrap())
    }

    pub async fn flows(&self, session: &CachingSession) -> Result<CharybdisModelStream<Flow>, NodecosmosError> {
        let flows = Flow::find_by_node_id_and_workflow_id(self.node_id, self.id)
            .execute(session)
            .await?;

        Ok(flows)
    }
}

partial_workflow!(UpdateInitialInputsWorkflow, node_id, id, initial_input_ids, updated_at);

// used by node deletion
partial_workflow!(DeleteWorkflow, node_id, id);

partial_workflow!(UpdateWorkflowTitle, node_id, id, title, updated_at);
