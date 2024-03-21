use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::update::BranchUpdate;
use crate::models::branch::Branch;
use crate::models::input_output::{Io, UpdateTitleIo};
use crate::models::traits::Branchable;
use charybdis::callbacks::Callbacks;
use charybdis::model::AsNative;
use scylla::CachingSession;

impl Callbacks for Io {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_insert(&mut self, session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        self.validate_root_node_id(session).await?;
        self.set_defaults();
        self.copy_vals_from_original(session).await?;

        if self.is_branched() {
            Branch::update(data, self.branch_id, BranchUpdate::CreateIo(self.id)).await?;
        }

        Ok(())
    }

    async fn before_delete(&mut self, session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        self.pull_from_initial_input_ids(session).await?;
        self.pull_form_flow_step_outputs(data).await?;
        self.pull_from_next_workflow_step(data).await?;

        if self.is_branched() {
            Branch::update(data, self.branch_id, BranchUpdate::DeleteIo(self.id)).await?;
        }

        Ok(())
    }

    async fn after_delete(&mut self, _session: &CachingSession, data: &RequestData) -> Result<(), Self::Error> {
        if self.is_branched() {
            self.create_branched_if_original_exists(data).await?;
        }

        Ok(())
    }
}

impl Callbacks for UpdateTitleIo {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_update(&mut self, _session: &CachingSession, data: &RequestData) -> Result<(), Self::Error> {
        self.updated_at = Some(chrono::Utc::now());

        if self.is_branched() {
            self.as_native().create_branched_if_original_exists(data).await?;
            Branch::update(data, self.branch_id, BranchUpdate::EditIOTitle(self.id)).await?;
        }

        Ok(())
    }

    async fn after_update(&mut self, session: &CachingSession, _data: &RequestData) -> Result<(), NodecosmosError> {
        self.update_ios_titles_by_org_id(session).await?;

        Ok(())
    }
}
