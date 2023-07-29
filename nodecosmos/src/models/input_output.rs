use crate::models::flow_step::FlowStep;
use crate::models::helpers::{
    created_at_cb_fn, impl_updated_at_cb, sanitize_description_cb, updated_at_cb_fn,
};
use crate::models::udts::Property;
use crate::models::workflow::Workflow;
use charybdis::*;

#[derive(Clone)]
#[partial_model_generator]
#[charybdis_model(
    table_name = input_outputs,
    partition_keys = [node_id],
    clustering_keys = [workflow_id, id],
    secondary_indexes = []
)]
pub struct InputOutput {
    #[serde(rename = "nodeId")]
    pub node_id: Uuid,

    #[serde(rename = "workflowId")]
    pub workflow_id: Uuid,

    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,

    #[serde(rename = "flowStepId")]
    pub flow_step_id: Option<Uuid>,

    pub title: Option<Text>,
    pub unit: Option<Text>,

    #[serde(rename = "dataType")]
    pub data_type: Option<Text>,

    pub description: Option<Text>,

    #[serde(rename = "descriptionMarkdown")]
    pub description_markdown: Option<Text>,

    pub value: Option<Text>,
    pub properties: Option<Frozen<List<Frozen<Property>>>>,

    #[serde(rename = "createdAt")]
    pub created_at: Option<Timestamp>,

    #[serde(rename = "updatedAt")]
    pub updated_at: Option<Timestamp>,
}

impl InputOutput {
    async fn workflow(&self, session: &CachingSession) -> Result<Workflow, CharybdisError> {
        let mut workflow = Workflow::new();
        workflow.node_id = self.node_id;
        workflow.id = self.workflow_id;

        workflow.find_by_primary_key(session).await
    }

    async fn flow_step(
        &self,
        session: &CachingSession,
    ) -> Result<Option<FlowStep>, CharybdisError> {
        let flow_step_id = self.flow_step_id.unwrap_or_default();

        if flow_step_id.is_nil() {
            return Ok(None);
        }

        let mut flow_step = FlowStep::new();
        flow_step.node_id = self.node_id;
        flow_step.workflow_id = self.workflow_id;
        flow_step.id = self.flow_step_id.unwrap_or_default();

        let fs = flow_step.find_by_primary_key(session).await?;

        Ok(Some(fs))
    }

    async fn next_flow_step(
        &self,
        session: &CachingSession,
    ) -> Result<Option<FlowStep>, CharybdisError> {
        let flow_step = self.flow_step(session).await?;

        if let Some(fs) = flow_step {
            let flow = fs.flow(session).await?;
            let flow_step_ids = flow.step_ids.unwrap_or_default();
            let flow_step_index = flow_step_ids.iter().position(|&x| x == fs.id);

            if let Some(idx) = flow_step_index {
                let next_idx = idx + 1;
                let next_flow_step_id = flow_step_ids.get(next_idx);

                if let Some(id) = next_flow_step_id {
                    let mut next_flow_step = FlowStep::new();
                    next_flow_step.node_id = self.node_id;
                    next_flow_step.workflow_id = self.workflow_id;
                    next_flow_step.id = *id;

                    let next_fs = next_flow_step.find_by_primary_key(session).await?;

                    return Ok(Some(next_fs));
                }
            }
        }

        Ok(None)
    }
}

impl Callbacks for InputOutput {
    created_at_cb_fn!();

    updated_at_cb_fn!();

    async fn after_delete(&mut self, session: &CachingSession) -> Result<(), CharybdisError> {
        let mut workflow = self.workflow(session).await?;
        let initial_input_ids = workflow.initial_input_ids.clone().unwrap_or_default();

        if initial_input_ids.contains(&self.id) {
            workflow.pull_initial_input_id(session, self.id).await?;
        }

        let flow_step = self.flow_step(session).await?;

        if let Some(mut fs) = flow_step {
            fs.pull_output_id(session, self.id).await?;
        }

        let next_flow_step = self.next_flow_step(session).await?;

        if let Some(mut next_fs) = next_flow_step {
            next_fs.pull_input_id(session, self.id).await?;
        }

        Ok(())
    }
}

partial_input_output!(
    IoDescription,
    node_id,
    workflow_id,
    id,
    description,
    description_markdown,
    updated_at
);
sanitize_description_cb!(IoDescription);

partial_input_output!(IoTitle, node_id, workflow_id, id, title, updated_at);
impl_updated_at_cb!(IoTitle);

partial_input_output!(IoDelete, node_id, workflow_id, id);
