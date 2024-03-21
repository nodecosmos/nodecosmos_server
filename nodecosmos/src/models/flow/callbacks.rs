use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::update::BranchUpdate;
use crate::models::branch::Branch;
use crate::models::flow::{Flow, UpdateTitleFlow};
use crate::models::traits::Branchable;
use charybdis::callbacks::Callbacks;
use charybdis::model::AsNative;
use charybdis::operations::Delete;
use charybdis::types::Uuid;
use futures::TryStreamExt;
use scylla::CachingSession;

impl Callbacks for Flow {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_insert(&mut self, db_session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        let now = chrono::Utc::now();

        self.id = Uuid::new_v4();
        self.created_at = Some(now);
        self.updated_at = Some(now);

        if self.is_branched() {
            self.workflow(db_session)
                .await?
                .create_branched_if_not_exist(data)
                .await?;

            Branch::update(data, self.branch_id, BranchUpdate::CreateFlow(self.id)).await?;
        }

        Ok(())
    }

    async fn before_delete(&mut self, db_session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        let mut flow_steps = self.flow_steps(db_session).await?;

        if self.is_branched() {
            Branch::update(data, self.branch_id, BranchUpdate::DeleteFlow(self.id)).await?;
        }

        while let Some(mut flow_step) = flow_steps.try_next().await? {
            flow_step.pull_outputs_from_next_workflow_step(data).await?;
            flow_step.delete_fs_outputs(data).await?;
            flow_step.delete().execute(db_session).await?;
        }

        Ok(())
    }

    async fn after_delete(&mut self, _db_session: &CachingSession, data: &RequestData) -> Result<(), Self::Error> {
        if self.is_branched() {
            self.create_branched_if_original_exists(data).await?;
        }

        Ok(())
    }
}

impl Callbacks for UpdateTitleFlow {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_update(&mut self, _db_session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        self.updated_at = Some(chrono::Utc::now());

        if self.is_branched() {
            self.as_native().create_branched_if_not_exist(data).await?;

            Branch::update(data, self.branch_id, BranchUpdate::EditFlowTitle(self.id)).await?;
        }

        Ok(())
    }
}
