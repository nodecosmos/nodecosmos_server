use crate::errors::NodecosmosError;
use crate::models::input_output::{DeleteIo, Io, UpdateDescriptionIo, UpdateTitleIo};
use crate::models::utils::{sanitize_description_cb_fn, updated_at_cb_fn};
use charybdis::batch::CharybdisModelBatch;
use charybdis::callbacks::Callbacks;
use charybdis::model::AsNative;
use charybdis::types::Uuid;
use scylla::CachingSession;

impl Callbacks<NodecosmosError> for Io {
    async fn before_insert(&mut self, session: &CachingSession) -> Result<(), NodecosmosError> {
        let now = chrono::Utc::now();

        self.id = Uuid::new_v4();
        self.created_at = Some(now);
        self.updated_at = Some(now);

        self.copy_vals_from_original(session).await?;

        Ok(())
    }
}

impl Callbacks<NodecosmosError> for UpdateDescriptionIo {
    sanitize_description_cb_fn!();

    /// This may seem cumbersome, but end-goal with IOs is to reflect title, description and unit changes,
    /// while allowing IO to have it's own properties and value.
    async fn after_update(&self, session: &CachingSession) -> Result<(), NodecosmosError> {
        if let Some(original_id) = self.original_id {
            let mut ios = Io::ios_by_original_id(session, self.root_node_id, original_id).await?;

            for chunk in ios.chunks_mut(25) {
                let mut batch = CharybdisModelBatch::new();

                for io in chunk {
                    if io.id == self.id {
                        continue;
                    }
                    io.description = self.description.clone();
                    io.description_markdown = self.description_markdown.clone();
                    io.updated_at = self.updated_at;

                    batch.append_update(io)?;
                }

                // Execute the batch update
                batch.execute(session).await?;
            }
        }

        Ok(())
    }
}

impl Callbacks<NodecosmosError> for UpdateTitleIo {
    updated_at_cb_fn!();

    async fn after_update(&self, session: &CachingSession) -> Result<(), NodecosmosError> {
        if let Some(original_id) = self.original_id {
            let mut ios = Io::ios_by_original_id(session, self.root_node_id, original_id).await?;

            for chunk in ios.chunks_mut(25) {
                let mut batch = CharybdisModelBatch::new();

                for io in chunk {
                    if io.id == self.id {
                        continue;
                    }
                    io.description = self.title.clone();
                    io.updated_at = self.updated_at;

                    batch.append_update(io)?;
                }

                // Execute the batch update
                batch.execute(session).await?;
            }
        }

        Ok(())
    }
}

impl Callbacks<NodecosmosError> for DeleteIo {
    async fn before_delete(&mut self, session: &CachingSession) -> Result<(), NodecosmosError> {
        let mut native_io = self.as_native();

        native_io.pull_from_initial_input_ids(session).await?;
        native_io.pull_form_flow_step(session).await?;
        native_io.pull_from_next_workflow_step(session).await?;

        Ok(())
    }
}
