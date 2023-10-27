use crate::errors::NodecosmosError;
use crate::models::flow::DeleteFlow;
use crate::models::flow_step::DeleteFlowStep;
use crate::models::helpers::{created_at_cb_fn, impl_updated_at_cb, updated_at_cb_fn};
use crate::models::input_output::DeleteInputOutput;
use crate::models::workflow::{UpdateInitialInputsWorkflow, UpdateWorkflowTitle, Workflow};
use charybdis::callbacks::Callbacks;
use scylla::CachingSession;

impl Callbacks<NodecosmosError> for Workflow {
    created_at_cb_fn!();

    updated_at_cb_fn!();

    async fn after_delete(&self, session: &CachingSession) -> Result<(), NodecosmosError> {
        DeleteFlowStep::delete_by_node_id_and_workflow_id(session, self.node_id, self.id).await?;
        DeleteFlow::delete_by_node_id_and_workflow_id(session, self.node_id, self.id).await?;

        DeleteInputOutput::delete_by_root_node_id_and_node_id(session, self.root_node_id, self.node_id).await?;
        DeleteInputOutput::delete_by_root_node_id_and_node_id(session, self.root_node_id, self.node_id)
            .await?
            .try_collect()
            .await?;

        Ok(())
    }
}

impl_updated_at_cb!(UpdateInitialInputsWorkflow);

impl_updated_at_cb!(UpdateWorkflowTitle);
