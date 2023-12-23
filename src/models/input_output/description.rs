use crate::errors::NodecosmosError;
use crate::models::input_output::{Io, UpdateDescriptionIo};
use charybdis::batch::CharybdisModelBatch;
use scylla::CachingSession;

impl UpdateDescriptionIo {
    /// This may seem cumbersome, but end-goal with IOs is to reflect title, description and unit changes,
    /// while allowing IO to have it's own properties and value.
    pub async fn update_ios_desc_by_org_id(&mut self, session: &CachingSession) -> Result<(), NodecosmosError> {
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
