use crate::errors::NodecosmosError;
use crate::models::flow::{Flow, FlowDescription, UpdateFlowTitle};
use crate::models::flow_step::flow_steps_by_index::FlowStepsByIndex;
use crate::models::utils::{created_at_cb_fn, impl_updated_at_cb, sanitize_description_cb, updated_at_cb_fn};
use charybdis::callbacks::Callbacks;
use charybdis::operations::Delete;
use futures::TryStreamExt;
use scylla::CachingSession;

impl Callbacks<NodecosmosError> for Flow {
    created_at_cb_fn!();

    updated_at_cb_fn!();

    async fn before_delete(&mut self, session: &CachingSession) -> Result<(), NodecosmosError> {
        let mut flow_steps = self.flow_steps(session).await?;

        let workflow = self.workflow(session).await?;
        let mut flow_steps_by_index = FlowStepsByIndex::build(session, &workflow).await?;

        while let Some(mut flow_step) = flow_steps.try_next().await? {
            flow_step
                .pull_outputs_from_next_workflow_step(session, &workflow, &mut flow_steps_by_index)
                .await?;
            flow_step.delete_fs_outputs(session, &workflow).await?;
            flow_step.delete(session).await?;
        }

        Ok(())
    }
}

impl_updated_at_cb!(UpdateFlowTitle);

sanitize_description_cb!(FlowDescription);
