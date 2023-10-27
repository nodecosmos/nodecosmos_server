use crate::errors::NodecosmosError;
use crate::models::flow_step::flow_steps_by_index::FlowStepsByIndex;
use crate::models::helpers::{sanitize_description_cb_fn, updated_at_cb_fn};
use crate::models::input_output::{InputOutput, UpdateDescriptionInputOutput, UpdateTitleInputOutput};
use charybdis::batch::CharybdisModelBatch;
use charybdis::callbacks::Callbacks;
use charybdis::types::Uuid;
use scylla::CachingSession;

impl Callbacks<NodecosmosError> for InputOutput {
    async fn before_insert(&mut self, session: &CachingSession) -> Result<(), NodecosmosError> {
        let now = chrono::Utc::now();

        self.id = Uuid::new_v4();
        self.created_at = Some(now);
        self.updated_at = Some(now);

        self.copy_vals_from_original(session).await?;

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

        let mut flow_steps_by_index = FlowStepsByIndex::build(session, &workflow).await?;
        self.remove_from_next_workflow_step(session, flow_step.as_ref(), &mut flow_steps_by_index)
            .await?;

        if let Some(mut flow_step) = flow_step {
            flow_step.pull_output_id(session, self.id).await?;
        }

        Ok(())
    }
}

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

impl Callbacks<NodecosmosError> for UpdateTitleInputOutput {
    updated_at_cb_fn!();

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
