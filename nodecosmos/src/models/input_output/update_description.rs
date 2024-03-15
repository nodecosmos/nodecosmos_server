use crate::errors::NodecosmosError;
use crate::models::input_output::UpdateDescriptionIo;
use charybdis::batch::ModelBatch;
use scylla::CachingSession;

impl UpdateDescriptionIo {
    /// This may seem cumbersome, but end-goal with IOs is to reflect title, description and unit changes,
    /// while allowing IO to have it's own properties and value.
    pub async fn update_ios_desc_by_org_id(&mut self, session: &CachingSession) -> Result<(), NodecosmosError> {
        if let Some(original_id) = self.original_id {
            let ios = UpdateDescriptionIo::ios_by_original_id(session, self.root_node_id, original_id).await?;
            let mut batch_ios = Vec::with_capacity(ios.len());

            for mut io in ios {
                if io.id == self.id {
                    continue;
                }
                io.description = self.description.clone();
                io.description_markdown = self.description_markdown.clone();
                io.updated_at = self.updated_at;

                batch_ios.push(io);
            }

            UpdateDescriptionIo::batch()
                .chunked_update(session, &batch_ios, 100)
                .await?;
        }

        Ok(())
    }
}
