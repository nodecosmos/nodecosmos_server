use crate::errors::NodecosmosError;
use crate::models::flow_step::flow_steps_by_index::FlowStepsByIndex;
use crate::models::flow_step::{
    FlowStep, UpdateDescriptionFlowStep, UpdateFlowStepNodeIds, UpdateInputIdsFlowStep, UpdateOutputIdsFlowStep,
};
use crate::models::utils::{impl_updated_at_cb, sanitize_description_cb, updated_at_cb_fn};
use charybdis::callbacks::Callbacks;
use charybdis::model::AsNative;
use charybdis::operations::{Find, UpdateWithCallbacks};
use charybdis::types::Uuid;
use scylla::CachingSession;

impl Callbacks<NodecosmosError> for FlowStep {
    async fn before_insert(&mut self, session: &CachingSession) -> Result<(), NodecosmosError> {
        let now = chrono::Utc::now();

        self.id = Uuid::new_v4();
        self.created_at = Some(now);
        self.updated_at = Some(now);

        let prev_fs = self.prev_flow_step(session).await?;
        let next_fs = self.next_flow_step(session).await?;

        match (prev_fs, next_fs) {
            (Some(mut prev_fs), Some(mut next_fs)) => {
                let workflow = self.workflow(session).await?;
                let mut flow_steps_by_index = FlowStepsByIndex::build(session, &workflow).await?;

                prev_fs
                    .pull_outputs_from_next_workflow_step(session, &workflow, &mut flow_steps_by_index)
                    .await?;
                prev_fs.next_flow_step_id = Some(self.id);
                prev_fs.update_cb(session).await?;

                next_fs.prev_flow_step_id = Some(self.id);
                next_fs.update_cb(session).await?;
                next_fs.remove_inputs(session).await?;
            }
            (Some(mut prev_fs), None) => {
                prev_fs.next_flow_step_id = Some(self.id);
                prev_fs.update_cb(session).await?;
            }
            (None, Some(mut next_fs)) => {
                next_fs.prev_flow_step_id = Some(self.id);
                next_fs.update_cb(session).await?;
                next_fs.remove_inputs(session).await?;
            }
            _ => {}
        }

        Ok(())
    }

    updated_at_cb_fn!();

    async fn before_delete(&mut self, session: &CachingSession) -> Result<(), NodecosmosError> {
        let workflow = self.workflow(session).await?;
        let mut flow_steps_by_index = FlowStepsByIndex::build(session, &workflow).await?;

        self.pull_outputs_from_next_workflow_step(session, &workflow, &mut flow_steps_by_index)
            .await?;
        self.delete_fs_outputs(session, &workflow).await?;

        if let Some(mut prev_flow_step) = self.prev_flow_step(session).await? {
            prev_flow_step.next_flow_step_id = self.next_flow_step_id;
            prev_flow_step.update_cb(session).await?;
        }

        if let Some(mut next_flow_step) = self.next_flow_step(session).await? {
            next_flow_step.prev_flow_step_id = self.prev_flow_step_id;
            next_flow_step.update_cb(session).await?;
        }

        Ok(())
    }
}

impl_updated_at_cb!(UpdateInputIdsFlowStep);

impl Callbacks<NodecosmosError> for UpdateFlowStepNodeIds {
    updated_at_cb_fn!();

    async fn after_update(&self, session: &CachingSession) -> Result<(), NodecosmosError> {
        let mut flow_step = self.as_native().find_by_primary_key(session).await?;
        let workflow = flow_step.workflow(session).await?;

        flow_step.delete_outputs_from_removed_nodes(session, &workflow).await?;
        flow_step.remove_inputs_from_removed_nodes(session).await?;

        Ok(())
    }
}

sanitize_description_cb!(UpdateDescriptionFlowStep);

impl_updated_at_cb!(UpdateOutputIdsFlowStep);
