use crate::errors::NodecosmosError;
use crate::models::input_output::UpdateTitleIo;
use crate::models::traits::Branchable;
use charybdis::batch::ModelBatch;
use charybdis::model::AsNative;
use scylla::CachingSession;

impl UpdateTitleIo {
    pub async fn update_ios_titles_by_main_id(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        if let Some(main_id) = self.main_id {
            if self.is_branched() {
                self.as_native().clone_main_ios_to_branch(db_session).await?;
            }

            let ios = UpdateTitleIo::ios_by_main_id(db_session, self.root_node_id, self.branch_id, main_id).await?;
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
