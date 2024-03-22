use crate::api::data::RequestData;
use crate::api::WorkflowParams;
use crate::errors::NodecosmosError;
use crate::models::branch::update::BranchUpdate;
use crate::models::branch::Branch;
use crate::models::flow::Flow;
use crate::models::flow_step::FlowStep;
use crate::models::input_output::{Io, UpdateTitleIo};
use crate::models::traits::{Branchable, FindOrInsertBranchedFromParams};
use crate::models::workflow::Workflow;
use charybdis::callbacks::Callbacks;
use charybdis::model::AsNative;
use scylla::CachingSession;

impl Callbacks for Io {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_insert(&mut self, db_session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        self.validate_root_node_id(db_session).await?;
        self.set_defaults();
        self.copy_vals_from_main(db_session).await?;

        if self.is_branched() {
            let params = WorkflowParams {
                node_id: self.node_id,
                branch_id: self.branch_id,
            };

            if let Some(flow_step_id) = self.flow_step_id {
                let fs = FlowStep::find_or_insert_branched(db_session, &params, flow_step_id).await?;
                Flow::find_or_insert_branched(db_session, &params, fs.flow_id).await?;
            }

            Workflow::find_or_insert_branched(db_session, &params, self.workflow_id).await?;

            Branch::update(data, self.branch_id, BranchUpdate::CreateIo(self.id)).await?;
        }

        Ok(())
    }

    async fn before_delete(&mut self, db_session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        self.pull_from_initial_input_ids(db_session).await?;
        self.pull_form_flow_step_outputs(data).await?;
        self.pull_from_next_workflow_step(data).await?;

        if self.is_branched() {
            Branch::update(data, self.branch_id, BranchUpdate::DeleteIo(self.id)).await?;
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

impl Callbacks for UpdateTitleIo {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_update(&mut self, db_session: &CachingSession, data: &RequestData) -> Result<(), Self::Error> {
        self.updated_at = Some(chrono::Utc::now());

        if self.is_branched() {
            self.as_native().create_branched_if_original_exists(data).await?;
            Branch::update(data, self.branch_id, BranchUpdate::EditIOTitle(self.id)).await?;
        }

        self.update_ios_titles_by_main_id(db_session).await?;

        Ok(())
    }
}
