use crate::errors::NodecosmosError;
use crate::models::flow::{Flow, FlowDescription, UpdateFlowTitle};
use crate::models::flow_step::FlowStep;
use crate::models::helpers::{created_at_cb_fn, impl_updated_at_cb, sanitize_description_cb, updated_at_cb_fn};
use charybdis::callbacks::Callbacks;
use scylla::CachingSession;

impl Callbacks<NodecosmosError> for Flow {
    created_at_cb_fn!();

    updated_at_cb_fn!();

    async fn after_delete(&self, session: &CachingSession) -> Result<(), NodecosmosError> {
        FlowStep::delete_by_node_id_and_workflow_id_and_flow_id(session, self.node_id, self.workflow_id, self.id)
            .await?;

        Ok(())
    }
}

impl_updated_at_cb!(UpdateFlowTitle);

sanitize_description_cb!(FlowDescription);
