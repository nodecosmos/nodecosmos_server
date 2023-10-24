use crate::errors::NodecosmosError;
use crate::models::flow_step::flow_steps_by_index::FlowStepsByIndex;
use crate::models::flow_step::{find_one_flow_step, FlowStep};
use crate::models::helpers::{sanitize_description_cb_fn, updated_at_cb_fn};
use crate::models::udts::Property;
use crate::models::workflow::Workflow;
use charybdis::batch::CharybdisModelBatch;
use charybdis::callbacks::Callbacks;
use charybdis::macros::charybdis_model;
use charybdis::operations::{Find, New};
use charybdis::types::{Frozen, List, Text, Timestamp, Uuid};
use scylla::CachingSession;
use serde::{Deserialize, Serialize};

/// we group input outputs by root node id
/// so they are accessible to all workflows within a same root node
#[derive(Clone)]
#[charybdis_model(
    table_name = input_outputs,
    partition_keys = [root_node_id],
    clustering_keys = [node_id, workflow_id, id],
    local_secondary_indexes = [
        ([root_node_id] [id])
    ]
)]
#[derive(Serialize, Deserialize, Default)]
pub struct InputOutput {
    #[serde(rename = "rootNodeId")]
    pub root_node_id: Uuid,

    #[serde(rename = "nodeId")]
    pub node_id: Uuid,

    #[serde(rename = "workflowId")]
    pub workflow_id: Uuid,

    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,

    #[serde(rename = "originalId")]
    pub original_id: Option<Uuid>,

    /// outputted by flow step
    #[serde(rename = "flowStepId")]
    pub flow_step_id: Option<Uuid>,

    pub title: Option<Text>,
    pub unit: Option<Text>,

    #[serde(rename = "dataType")]
    pub data_type: Option<Text>,
    pub value: Option<Text>,

    pub description: Option<Text>,

    #[serde(rename = "descriptionMarkdown")]
    pub description_markdown: Option<Text>,

    pub properties: Option<Frozen<List<Frozen<Property>>>>,

    #[serde(rename = "createdAt")]
    pub created_at: Option<Timestamp>,

    #[serde(rename = "updatedAt")]
    pub updated_at: Option<Timestamp>,
}

impl InputOutput {
    pub async fn ios_by_original_id(
        session: &CachingSession,
        root_node_id: Uuid,
        original_id: Uuid,
    ) -> Result<Vec<InputOutput>, NodecosmosError> {
        let ios = find_input_output!(
            session,
            "root_node_id = ? AND original_id = ?",
            (root_node_id, original_id)
        )
        .await?
        .try_collect()
        .await?;

        Ok(ios)
    }

    async fn workflow(&self, session: &CachingSession) -> Result<Workflow, NodecosmosError> {
        let mut workflow = Workflow::new();
        workflow.node_id = self.node_id;
        workflow.id = self.workflow_id;

        let res = workflow.find_by_primary_key(session).await?;

        Ok(res)
    }

    async fn flow_step(&self, session: &CachingSession) -> Result<Option<FlowStep>, NodecosmosError> {
        let flow_step_id = self.flow_step_id.unwrap_or_default();

        if flow_step_id.is_nil() {
            return Ok(None);
        }

        let flow_step = find_one_flow_step!(session, "node_id = ? AND id = ?", (self.node_id, flow_step_id)).await?;

        Ok(Some(flow_step))
    }
}

impl Callbacks<NodecosmosError> for InputOutput {
    async fn before_insert(&mut self, session: &CachingSession) -> Result<(), NodecosmosError> {
        let now = chrono::Utc::now();

        self.id = Uuid::new_v4();
        self.created_at = Some(now);
        self.updated_at = Some(now);

        if let Some(original_id) = self.original_id {
            let original_input =
                find_one_input_output!(session, "root_node_id = ? AND id = ?", (self.root_node_id, original_id))
                    .await?;

            self.title = original_input.title;
            self.unit = original_input.unit;
            self.data_type = original_input.data_type;
            self.description = original_input.description;
            self.description_markdown = original_input.description_markdown;
            self.original_id = original_input.original_id;
        } else {
            self.original_id = Some(self.id);
        }

        Ok(())
    }

    updated_at_cb_fn!();

    async fn before_delete(&mut self, session: &CachingSession) -> Result<(), NodecosmosError> {
        let mut workflow = self.workflow(session).await?;
        let initial_input_ids = workflow.initial_input_ids.clone().unwrap_or_default();

        if initial_input_ids.contains(&self.id) {
            workflow.pull_initial_input_id(session, self.id).await?;
        }

        let flow_step = self.flow_step(session).await?;

        if let Some(mut flow_step) = flow_step {
            flow_step.pull_output_id(session, self.id).await?;

            // remove input from next flow steps
            let mut flow_steps_by_index = FlowStepsByIndex::build(session, &workflow).await?;
            let current_step_wf_index = flow_steps_by_index.flow_step_index(flow_step.id);

            if let Some(current_step_wf_index) = current_step_wf_index {
                let next_wf_index = current_step_wf_index + 1;
                let next_flow_steps = flow_steps_by_index.flow_steps_by_wf_index(next_wf_index)?;

                if let Some(mut next_flow_steps) = next_flow_steps {
                    for flow_step in next_flow_steps.iter_mut() {
                        let mut flow_step = flow_step.borrow_mut();
                        flow_step.pull_input_id(session, self.id).await?;
                    }
                }
            }
        }

        Ok(())
    }
}

partial_input_output!(
    UpdateDescriptionInputOutput,
    root_node_id,
    node_id,
    workflow_id,
    id,
    original_id,
    description,
    description_markdown,
    updated_at
);

impl Callbacks<NodecosmosError> for UpdateDescriptionInputOutput {
    sanitize_description_cb_fn!();

    /// This may seem cumbersome, but end-goal with IOs is to reflect title, description and unit changes,
    /// while allowing IO to have it's own properties and value.
    async fn after_update(&self, session: &CachingSession) -> Result<(), NodecosmosError> {
        if let Some(original_id) = self.original_id {
            let ios = InputOutput::ios_by_original_id(session, self.root_node_id, original_id).await?;

            for chunk in ios.chunks(25) {
                let mut batch = CharybdisModelBatch::new();

                for io in chunk {
                    if io.id == self.id {
                        continue;
                    }
                    let mut updated_input = io.clone();
                    updated_input.description = self.description.clone();
                    updated_input.description_markdown = self.description_markdown.clone();
                    updated_input.updated_at = self.updated_at;

                    batch.append_update(&updated_input)?;
                }

                // Execute the batch update
                batch.execute(session).await?;
            }
        }

        Ok(())
    }
}

partial_input_output!(
    UpdateTitleInputOutput,
    root_node_id,
    node_id,
    workflow_id,
    id,
    original_id,
    title,
    updated_at
);

impl Callbacks<NodecosmosError> for UpdateTitleInputOutput {
    updated_at_cb_fn!();

    /// See UpdateDescriptionInputOutput::after_update for explanation
    async fn after_update(&self, session: &CachingSession) -> Result<(), NodecosmosError> {
        if let Some(original_id) = self.original_id {
            let ios = InputOutput::ios_by_original_id(session, self.root_node_id, original_id).await?;

            for chunk in ios.chunks(25) {
                let mut batch = CharybdisModelBatch::new();

                for io in chunk {
                    if io.id == self.id {
                        continue;
                    }
                    let mut updated_input = io.clone();
                    updated_input.title = self.title.clone();
                    updated_input.updated_at = self.updated_at;

                    batch.append_update(&updated_input)?;
                }

                batch.execute(session).await?;
            }
        }

        Ok(())
    }
}

partial_input_output!(UpdateWorkflowIndexInputOutput, root_node_id, node_id, workflow_id, id);

partial_input_output!(DeleteInputOutput, root_node_id, node_id, workflow_id, id);
