use crate::models::flow::{find_flow_delete_query, FlowDelete};
use crate::models::flow_step::{find_flow_step_delete_query, FlowStepDelete};
use crate::models::helpers::{created_at_cb_fn, impl_updated_at_cb, updated_at_cb_fn};
use crate::models::input_output::{find_io_delete_query, IoDelete};
use charybdis::*;

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

    pub async fn pull_initial_input_id(
        &mut self,
        session: &CachingSession,
        initial_input_id: Uuid,
    ) -> Result<(), CharybdisError> {
        execute(
            session,
            Workflow::PULL_FROM_INITIAL_INPUT_IDS_QUERY,
            (initial_input_id, self.node_id, self.id),
        )
        .await?;

        Ok(())
    }
}

impl Callbacks for Workflow {
    created_at_cb_fn!();

    updated_at_cb_fn!();

    async fn after_delete(&mut self, session: &CachingSession) -> Result<(), CharybdisError> {
        if self.flow_ids.is_some() {
            let mut batch = CharybdisModelBatch::new();

            let flow_steps = FlowStepDelete::find(
                session,
                find_flow_step_delete_query!("node_id = ? AND workflow_id = ?"),
                (self.node_id, self.id),
            )
            .await?;

            let flows = FlowDelete::find(
                session,
                find_flow_delete_query!("node_id = ? AND workflow_id = ?"),
                (self.node_id, self.id),
            )
            .await?;

            let input_outputs = IoDelete::find(
                session,
                find_io_delete_query!("node_id = ? AND workflow_id = ?"),
                (self.node_id, self.id),
            )
            .await?;

            batch.append_deletes(input_outputs)?;
            batch.append_deletes(flow_steps)?;
            batch.append_deletes(flows)?;

            batch.execute(session).await?;
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

partial_workflow!(WorkflowDelete, node_id, id);
