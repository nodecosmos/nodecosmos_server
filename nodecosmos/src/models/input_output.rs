use crate::models::flow_step::FlowStep;
use crate::models::helpers::{sanitize_description_cb_fn, updated_at_cb_fn};
use crate::models::udts::Property;
use crate::models::workflow::Workflow;
use charybdis::*;

#[derive(Clone)]
#[partial_model_generator]
#[charybdis_model(
    table_name = input_outputs,
    partition_keys = [node_id],
    clustering_keys = [workflow_id, id],
    secondary_indexes = [original_id, id]
)]
pub struct InputOutput {
    #[serde(rename = "nodeId")]
    pub node_id: Uuid,

    #[serde(rename = "workflowId")]
    pub workflow_id: Uuid,

    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,

    #[serde(rename = "rootNodeId")]
    pub root_node_id: Uuid,

    #[serde(rename = "originalId")]
    pub original_id: Option<Uuid>,

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
    async fn ios_by_original_id(
        session: &CachingSession,
        original_id: Uuid,
    ) -> Result<Vec<InputOutput>, CharybdisError> {
        let ios = InputOutput::find_iter(
            session,
            find_input_output_query!("original_id = ?"),
            (original_id,),
            100,
        )
        .await?
        .try_collect()
        .await?;

        Ok(ios)
    }

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
    async fn before_insert(&mut self, session: &CachingSession) -> Result<(), CharybdisError> {
        let now = chrono::Utc::now();

        self.id = Uuid::new_v4();
        self.created_at = Some(now);
        self.updated_at = Some(now);

        if let Some(original_id) = self.original_id {
            let original_io =
                InputOutput::find_one(session, find_input_output_query!("id = ?"), (original_id,))
                    .await?;

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
    original_id,
    description,
    description_markdown,
    updated_at
);
impl Callbacks for IoDescription {
    sanitize_description_cb_fn!();

    /// This may seem cumbersome, but end-goal with IOs is to reflect title, description and unit changes,
    /// while allowing IO to have it's own properties and value.
    async fn after_update(&mut self, session: &CachingSession) -> Result<(), CharybdisError> {
        if let Some(original_id) = self.original_id {
            let ios = InputOutput::ios_by_original_id(session, original_id).await?;

            for chunk in ios.chunks(25) {
                let mut batch = CharybdisModelBatch::new();

                for io in chunk {
                    if io.id == self.id {
                        continue;
                    }
                    let mut updated_io = io.clone();
                    updated_io.description = self.description.clone();
                    updated_io.description_markdown = self.description_markdown.clone();
                    updated_io.updated_at = self.updated_at;

                    batch.append_update(&updated_io)?;
                }

                // Execute the batch update
                batch.execute(session).await?;
            }
        }

        Ok(())
    }
}

partial_input_output!(
    IoTitle,
    node_id,
    workflow_id,
    id,
    original_id,
    title,
    updated_at
);
impl Callbacks for IoTitle {
    updated_at_cb_fn!();

    /// See IoDescription::after_update for explanation
    async fn after_update(&mut self, session: &CachingSession) -> Result<(), CharybdisError> {
        if let Some(original_id) = self.original_id {
            let ios = InputOutput::ios_by_original_id(session, original_id).await?;

            for chunk in ios.chunks(25) {
                let mut batch = CharybdisModelBatch::new();

                for io in chunk {
                    if io.id == self.id {
                        continue;
                    }
                    let mut updated_io = io.clone();
                    updated_io.title = self.title.clone();
                    updated_io.updated_at = self.updated_at;

                    batch.append_update(&updated_io)?;
                }

                batch.execute(session).await?;
            }
        }

        Ok(())
    }
}

partial_input_output!(IoDelete, node_id, workflow_id, id);
