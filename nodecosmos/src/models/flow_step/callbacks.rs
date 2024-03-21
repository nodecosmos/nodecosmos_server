use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::update::BranchUpdate;
use crate::models::branch::Branch;
use crate::models::flow::Flow;
use crate::models::flow_step::{FlowStep, UpdateInputIdsFlowStep, UpdateNodeIdsFlowStep, UpdateOutputIdsFlowStep};
use crate::models::traits::Branchable;
use crate::models::utils::updated_at_cb_fn;
use charybdis::callbacks::Callbacks;
use charybdis::model::AsNative;
use charybdis::operations::Find;
use scylla::CachingSession;

impl Callbacks for FlowStep {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_insert(&mut self, db_session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        self.set_defaults();
        self.validate_conflicts(db_session).await?;
        self.calculate_index(db_session).await?;
        self.sync_surrounding_fs_on_creation(data).await?;

        if self.is_branched() {
            let flow =
                Flow::find_by_node_id_and_branch_id_and_id(db_session, self.node_id, self.branch_id, self.flow_id)
                    .await?;
            flow.create_branched_if_not_exist(data).await?;

            Branch::update(data, self.branch_id, BranchUpdate::CreateFlowStep(self.id)).await?;
        }

        Ok(())
    }

    updated_at_cb_fn!();

    async fn before_delete(&mut self, _db_session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        self.pull_outputs_from_next_workflow_step(data).await?;
        self.delete_fs_outputs(data).await?;
        self.sync_surrounding_fs_on_del(data).await?;

        if self.is_branched() {
            Branch::update(data, self.branch_id, BranchUpdate::DeleteFlowStep(self.id)).await?;
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

impl Callbacks for UpdateInputIdsFlowStep {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_update(&mut self, _db_session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        self.updated_at = Some(chrono::Utc::now());

        if self.is_branched() {
            self.as_native().create_branched_if_not_exist(data).await?;
            self.update_branch(data).await?;
        }

        Ok(())
    }
}

impl Callbacks for UpdateNodeIdsFlowStep {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_update(&mut self, _db_session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        self.updated_at = Some(chrono::Utc::now());

        if self.is_branched() {
            self.as_native().create_branched_if_not_exist(data).await?;
            self.update_branch(data).await?;
        }

        Ok(())
    }

    async fn after_update(&mut self, db_session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        let mut flow_step = self.as_native().find_by_primary_key().execute(db_session).await?;

        flow_step.delete_outputs_from_removed_nodes(data).await?;
        flow_step.remove_outputs_from_removed_nodes(data).await?;
        flow_step.remove_inputs_from_removed_nodes(data).await?;

        Ok(())
    }
}

impl Callbacks for UpdateOutputIdsFlowStep {
    type Extension = RequestData;
    type Error = NodecosmosError;

    async fn before_update(&mut self, _db_session: &CachingSession, data: &RequestData) -> Result<(), NodecosmosError> {
        self.updated_at = Some(chrono::Utc::now());

        if self.is_branched() {
            self.as_native().create_branched_if_not_exist(data).await?;
            self.update_branch(data).await?;
        }

        Ok(())
    }
}
