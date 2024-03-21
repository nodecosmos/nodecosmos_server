mod callbacks;
pub mod create;
pub mod diagram;
mod update_initial_inputs;

use crate::api::WorkflowParams;
use crate::errors::NodecosmosError;
use crate::models::flow::Flow;
use crate::models::traits::Branchable;
use crate::models::workflow::diagram::WorkflowDiagram;
use charybdis::macros::charybdis_model;
use charybdis::stream::CharybdisModelStream;
use charybdis::types::{List, Text, Timestamp, Uuid};
use nodecosmos_macros::BranchableNodeId;
use scylla::CachingSession;
use serde::{Deserialize, Serialize};

/// Workflow model
///
/// Currently we only support one workflow per node.
///
/// Single `Workflow` can have multiple `Flows`.
/// `Flow` represents isolated process within `Workflow`.
/// Single `Flow` can have many `FlowSteps`.
/// `FlowSteps` contains inputs, nodes and outputs.
#[charybdis_model(
    table_name = workflows,
    partition_keys = [node_id],
    clustering_keys = [branch_id, id],
    global_secondary_indexes = []
)]
#[derive(BranchableNodeId, Serialize, Deserialize, Default, Clone)]
pub struct Workflow {
    #[serde(rename = "nodeId")]
    pub node_id: Uuid,

    #[serde(rename = "branchId")]
    pub branch_id: Uuid,

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

    #[charybdis(ignore)]
    #[serde(skip)]
    pub diagram: Option<WorkflowDiagram>,
}

impl Workflow {
    pub async fn branched(session: &CachingSession, params: &WorkflowParams) -> Result<Workflow, NodecosmosError> {
        if params.is_original() {
            let workflow = Workflow::find_first_by_node_id_and_branch_id(params.node_id, params.branch_id)
                .execute(session)
                .await?;

            Ok(workflow)
        } else {
            let maybe_branched = Workflow::maybe_find_first_by_node_id_and_branch_id(params.node_id, params.branch_id)
                .execute(session)
                .await?;

            if let Some(workflow) = maybe_branched {
                Ok(workflow)
            } else {
                let mut workflow = Workflow::find_first_by_node_id_and_branch_id(params.node_id, params.node_id)
                    .execute(session)
                    .await?;
                workflow.branch_id = params.branch_id;

                Ok(workflow)
            }
        }
    }

    pub async fn diagram(&mut self, session: &CachingSession) -> Result<&mut WorkflowDiagram, NodecosmosError> {
        if self.diagram.is_none() {
            let diagram = WorkflowDiagram::build(session, self).await?;
            self.diagram = Some(diagram);
        }

        Ok(self.diagram.as_mut().unwrap())
    }

    pub async fn flows(&self, session: &CachingSession) -> Result<CharybdisModelStream<Flow>, NodecosmosError> {
        let flows = Flow::find_by_node_id_and_branch_id_and_workflow_id(self.node_id, self.branch_id, self.id)
            .execute(session)
            .await?;

        Ok(flows)
    }
}

partial_workflow!(
    UpdateInitialInputsWorkflow,
    node_id,
    branch_id,
    id,
    initial_input_ids,
    updated_at
);

// used by node deletion
partial_workflow!(DeleteWorkflow, node_id, branch_id, id);

partial_workflow!(UpdateWorkflowTitle, node_id, branch_id, id, title, updated_at);
