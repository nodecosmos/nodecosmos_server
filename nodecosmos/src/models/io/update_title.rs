use charybdis::batch::ModelBatch;
use charybdis::model::AsNative;
use scylla::CachingSession;

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::update::BranchUpdate;
use crate::models::branch::Branch;
use crate::models::io::UpdateTitleIo;
use crate::models::traits::Branchable;

impl UpdateTitleIo {
    pub async fn update_ios_titles_by_main_id(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        if let Some(main_id) = self.main_id {
            if self.is_branch() {
                self.as_native().clone_main_ios_to_branch(db_session).await?;
            }

            let ios = UpdateTitleIo::ios_by_main_id(db_session, self.branch_id, main_id).await?;
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
                .chunked_update(db_session, &batch_ios, crate::constants::BATCH_CHUNK_SIZE)
                .await?;
        }

        Ok(())
    }

    pub async fn update_branch(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_branch() {
            self.as_native().create_branched_if_original_exists(data).await?;
            Branch::update(data, self.branch_id, BranchUpdate::EditNodeWorkflow(self.node_id)).await?;
            Branch::update(data, self.branch_id, BranchUpdate::EditIoTitle(self.id)).await?;
        }

        Ok(())
    }
}
