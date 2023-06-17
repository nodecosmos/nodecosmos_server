use crate::models::flow::Flow;
use crate::models::helpers::{created_at_cb_fn, impl_updated_at_cb, updated_at_cb_fn};
use crate::models::input_output::InputOutput;
use charybdis::*;
use chrono::Utc;

///
/// Workflow model
///
/// Currently we only support one workflow per node,
/// in future we will support multiple workflows per node.
///
/// Single workflow can have multiple flows that can be executed in parallel.
/// Flow should be isolated in a way that represents a single process.
/// Each flow can have multiple flow steps that can be executed in parallel.
/// Each flow step is made of input node and output.
///
/// In that sense Workflow is a collection of flows.
///
/// In future we will allow multiple workflows per node.
/// Reasoning is that we want to allow users to describe multiple processes.
#[partial_model_generator]
#[charybdis_model(
    table_name = workflows,
    partition_keys = [node_id],
    clustering_keys = [id],
    secondary_indexes = []
)]
pub struct Workflow {
    #[serde(rename = "nodeId")]
    pub node_id: Uuid,

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
    pub async fn append_flow_id(
        &mut self,
        session: &CachingSession,
        flow_id: Uuid,
    ) -> Result<(), CharybdisError> {
        execute(
            session,
            Workflow::PUSH_TO_FLOW_IDS_QUERY,
            (flow_id, self.node_id, self.id),
        )
        .await?;

        Ok(())
    }

    pub async fn pull_flow_id(
        &mut self,
        session: &CachingSession,
        flow_id: Uuid,
    ) -> Result<(), CharybdisError> {
        execute(
            session,
            Workflow::PULL_FROM_FLOW_IDS_QUERY,
            (flow_id, self.node_id, self.id),
        )
        .await?;

        Ok(())
    }
}

impl Callbacks for Workflow {
    created_at_cb_fn!();

    updated_at_cb_fn!();

    async fn after_delete(&mut self, session: &CachingSession) -> Result<(), CharybdisError> {
        let mut flow = Flow::new();
        flow.node_id = self.node_id;
        flow.workflow_id = self.id;

        let flows = flow.find_by_partition_key(session).await?;

        for flow in flows {
            flow?.delete_cb(session).await?;
        }

        // delete ios
        let mut input_output = InputOutput::new();
        input_output.workflow_id = self.id;

        let input_outputs = input_output.find_by_partition_key(session).await?;

        for input_output in input_outputs {
            input_output?.delete_cb(session).await?;
        }

        Ok(())
    }
}

partial_workflow!(
    UpdateInitialInputsWorkflow,
    node_id,
    id,
    initial_input_ids,
    updated_at
);
impl_updated_at_cb!(UpdateInitialInputsWorkflow);
