use crate::errors::NodecosmosError;
use crate::models::input_output::UpdateTitleIo;
use charybdis::batch::ModelBatch;
use scylla::CachingSession;

impl UpdateTitleIo {
    /// This may seem cumbersome, but end-goal with IOs is to reflect title, description and unit changes,
    /// while allowing IO to have it's own properties and value.
    pub async fn update_ios_titles_by_org_id(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        if let Some(original_id) = self.original_id {
            let ios =
                UpdateTitleIo::ios_by_original_id(db_session, self.root_node_id, self.branch_id, original_id).await?;
            let mut batch_ios = Vec::with_capacity(ios.len());

            for mut io in ios {
                if io.id == self.id {
                    continue;
                }
                io.title = self.title.clone();
                io.updated_at = self.updated_at;

                batch_ios.push(io);
            }

            UpdateTitleIo::batch()
                .chunked_update(db_session, &batch_ios, 100)
                .await?;
        }

        Ok(())
    }
}
