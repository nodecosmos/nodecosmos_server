use crate::errors::NodecosmosError;
use crate::models::flow_step::{
    FlowStep, UpdateDescriptionFlowStep, UpdateInputIdsFlowStep, UpdateNodeIdsFlowStep, UpdateOutputIdsFlowStep,
};
use crate::models::utils::{impl_updated_at_cb, sanitize_description_cb, updated_at_cb_fn};
use charybdis::callbacks::Callbacks;
use charybdis::model::AsNative;
use charybdis::operations::Find;
use scylla::CachingSession;

impl Callbacks for FlowStep {
    type Error = NodecosmosError;

    async fn before_insert(&mut self, session: &CachingSession) -> Result<(), NodecosmosError> {
        self.set_defaults();
        self.validate_conflicts(session).await?;
        self.calculate_index(session).await?;
        self.sync_surrounding_fs_on_creation(session).await?;

        Ok(())
    }

    updated_at_cb_fn!();

    async fn before_delete(&mut self, session: &CachingSession) -> Result<(), NodecosmosError> {
        self.pull_outputs_from_next_workflow_step(session).await?;
        self.delete_fs_outputs(session).await?;
        self.sync_surrounding_fs_on_del(session).await?;

        Ok(())
    }
}

impl_updated_at_cb!(UpdateInputIdsFlowStep);

impl Callbacks for UpdateNodeIdsFlowStep {
    type Error = NodecosmosError;

    updated_at_cb_fn!();

    async fn after_update(&mut self, session: &CachingSession) -> Result<(), NodecosmosError> {
        let mut flow_step = self.as_native().find_by_primary_key(session).await?;

        flow_step.delete_outputs_from_removed_nodes(session).await?;
        flow_step.remove_outputs_from_removed_nodes(session).await?;
        flow_step.remove_inputs_from_removed_nodes(session).await?;

        Ok(())
    }
}

sanitize_description_cb!(UpdateDescriptionFlowStep);

impl_updated_at_cb!(UpdateOutputIdsFlowStep);
