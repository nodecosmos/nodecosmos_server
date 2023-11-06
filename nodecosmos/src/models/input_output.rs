mod callbacks;

use crate::errors::NodecosmosError;
use crate::models::flow_step::FlowStep;
use crate::models::udts::Property;
use crate::models::workflow::Workflow;
use crate::utils::deserializer::required;
use charybdis::batch::CharybdisModelBatch;
use charybdis::macros::charybdis_model;
use charybdis::operations::New;
use charybdis::types::{Frozen, List, Text, Timestamp, Uuid};
use scylla::CachingSession;
use serde::{Deserialize, Serialize};

/// we group input outputs by root node id
/// so they are accessible to all workflows within a same root node
#[charybdis_model(
    table_name = input_outputs,
    partition_keys = [root_node_id],
    clustering_keys = [node_id, workflow_id, id],
    local_secondary_indexes = [
        ([root_node_id], [id]),
        ([root_node_id], [original_id]),
    ]
)]
#[derive(Serialize, Deserialize, Default)]
pub struct Io {
    #[serde(rename = "rootNodeId")]
    pub root_node_id: Uuid,

    #[serde(rename = "nodeId")]
    pub node_id: Uuid,

    #[serde(rename = "workflowId")]
    pub workflow_id: Uuid,

    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,

    #[serde(rename = "originalId", deserialize_with = "required")]
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

    #[charybdis(ignore)]
    #[serde(skip)]
    pub flow_step: Option<FlowStep>,

    #[charybdis(ignore)]
    #[serde(skip)]
    pub workflow: Option<Workflow>,
}

impl Io {
    pub async fn ios_by_original_id(
        session: &CachingSession,
        root_node_id: Uuid,
        original_id: Uuid,
    ) -> Result<Vec<Io>, NodecosmosError> {
        let ios = find_io!(
            session,
            "root_node_id = ? AND original_id = ?",
            (root_node_id, original_id)
        )
        .await?
        .try_collect()
        .await?;

        Ok(ios)
    }

    pub async fn delete_by_ids(
        session: &CachingSession,
        ids: Vec<Uuid>,
        workflow: &Workflow,
        flow_step_id: Option<Uuid>,
    ) -> Result<(), NodecosmosError> {
        let mut batch = CharybdisModelBatch::new();

        for output_id in ids.iter() {
            let mut output = Io::new();

            output.root_node_id = workflow.root_node_id;
            output.node_id = workflow.node_id;
            output.workflow_id = workflow.id;
            output.flow_step_id = flow_step_id;
            output.id = *output_id;

            batch.append_delete(&output)?;

            output.pull_from_next_workflow_step(session).await?;
        }

        batch.execute(session).await?;

        Ok(())
    }

    pub async fn workflow(&mut self, session: &CachingSession) -> Result<&mut Option<Workflow>, NodecosmosError> {
        if self.workflow.is_none() {
            let workflow = Workflow::by_node_id_and_id(session, self.node_id, self.workflow_id).await?;
            self.workflow = Some(workflow);
        }

        Ok(&mut self.workflow)
    }

    pub async fn flow_step(&mut self, session: &CachingSession) -> Result<&mut Option<FlowStep>, NodecosmosError> {
        if let Some(flow_step_id) = self.flow_step_id {
            if self.flow_step.is_none() {
                let flow_step = FlowStep::find_by_node_id_and_id(session, self.node_id, flow_step_id).await?;
                self.flow_step = Some(flow_step);
            }
        }

        Ok(&mut self.flow_step)
    }

    pub async fn original_io(&self, session: &CachingSession) -> Result<Option<Self>, NodecosmosError> {
        if let Some(original_id) = self.original_id {
            let original_io =
                find_one_io!(session, "root_node_id = ? AND id = ?", (self.root_node_id, original_id)).await?;
            Ok(Some(original_io))
        } else {
            Ok(None)
        }
    }

    /// We use copy instead of reference, as in future we may add more features
    /// that will require each node within a flow step to have it's own IO.
    pub async fn copy_vals_from_original(&mut self, session: &CachingSession) -> Result<(), NodecosmosError> {
        let original_io = self.original_io(session).await?;

        if let Some(original_io) = original_io {
            self.title = original_io.title;
            self.unit = original_io.unit;
            self.data_type = original_io.data_type;
            self.description = original_io.description;
            self.description_markdown = original_io.description_markdown;
            self.original_id = original_io.original_id;
        } else {
            self.original_id = Some(self.id);
        }

        Ok(())
    }

    pub async fn pull_from_initial_input_ids(&mut self, session: &CachingSession) -> Result<(), NodecosmosError> {
        let id = self.id;
        let workflow = self.workflow(session).await?;

        if let Some(workflow) = &mut workflow.as_mut() {
            let initial_input_ids = workflow.initial_input_ids.as_ref().unwrap();

            if initial_input_ids.contains(&id) {
                workflow.pull_initial_input_id(session, id).await?;
            }
        }

        Ok(())
    }

    pub async fn pull_form_flow_step_outputs(&mut self, session: &CachingSession) -> Result<(), NodecosmosError> {
        let id = self.id;
        let flow_step = self.flow_step(session).await?;

        if let Some(flow_step) = &mut flow_step.as_mut() {
            flow_step.pull_output_id(session, id).await?;
        }

        Ok(())
    }

    // remove output as input from next workflow step
    pub async fn pull_from_next_workflow_step(&mut self, session: &CachingSession) -> Result<(), NodecosmosError> {
        let current_step_wf_index;
        let flow_step_id = self.flow_step_id;
        let id = self.id;
        let workflow = self.workflow(session).await?;

        if let Some(workflow) = workflow.as_mut() {
            let diagram = workflow.diagram(session).await?;

            // get current workflow index of flow step
            if let Some(flow_step_id) = flow_step_id {
                let wf_index = diagram.workflow_index(flow_step_id);

                if let Some(wf_index) = wf_index {
                    current_step_wf_index = wf_index;
                } else {
                    return Err(NodecosmosError::InternalServerError("MissingFlowStepIndex".to_string()));
                }
            } else {
                current_step_wf_index = 0;
            }

            // get next flow steps within diagram
            let next_wf_index = current_step_wf_index + 1;
            let next_flow_steps = diagram.flow_steps_by_wf_index(next_wf_index)?;

            if let Some(mut next_flow_steps) = next_flow_steps {
                for flow_step in next_flow_steps.iter_mut() {
                    let mut flow_step = flow_step.borrow_mut();
                    flow_step.pull_input_id(session, id).await?;
                }
            }
        }

        Ok(())
    }
}

partial_io!(
    UpdateDescriptionIo,
    root_node_id,
    node_id,
    workflow_id,
    id,
    original_id,
    description,
    description_markdown,
    updated_at
);

partial_io!(
    UpdateTitleIo,
    root_node_id,
    node_id,
    workflow_id,
    id,
    original_id,
    title,
    updated_at
);

partial_io!(UpdateWorkflowIndexIo, root_node_id, node_id, workflow_id, id);

partial_io!(DeleteIo, root_node_id, node_id, workflow_id, id, flow_step_id);
