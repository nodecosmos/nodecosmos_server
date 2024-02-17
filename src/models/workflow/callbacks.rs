use crate::errors::NodecosmosError;
use crate::models::flow::DeleteFlow;
use crate::models::flow_step::DeleteFlowStep;
use crate::models::input_output::DeleteIo;
use crate::models::utils::{created_at_cb_fn, impl_updated_at_cb, updated_at_cb_fn};
use crate::models::workflow::{UpdateInitialInputsWorkflow, UpdateWorkflowTitle, Workflow};
use charybdis::callbacks::Callbacks;
use scylla::CachingSession;

impl Callbacks for Workflow {
    type Extension = Option<()>;
    type Error = NodecosmosError;

    created_at_cb_fn!();

    updated_at_cb_fn!();

    async fn after_delete(&mut self, session: &CachingSession, _ext: &Self::Extension) -> Result<(), NodecosmosError> {
        DeleteFlowStep::delete_by_node_id_and_workflow_id(self.node_id, self.id)
            .execute(session)
            .await?;
        DeleteFlow::delete_by_node_id_and_workflow_id(self.node_id, self.id)
            .execute(session)
            .await?;

        DeleteIo::delete_by_root_node_id_and_node_id(self.root_node_id, self.node_id)
            .execute(session)
            .await?;

        DeleteIo::delete_by_root_node_id_and_node_id(self.root_node_id, self.node_id)
            .execute(session)
            .await?;

        Ok(())
    }
}

impl_updated_at_cb!(UpdateInitialInputsWorkflow);

impl_updated_at_cb!(UpdateWorkflowTitle);
