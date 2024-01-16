use crate::errors::NodecosmosError;
use crate::models::flow_step::FlowStep;
use crate::models::input_output::Io;
use charybdis::model::AsNative;
use charybdis::operations::{New, UpdateWithCallbacks};
use charybdis::types::Uuid;
use scylla::CachingSession;

impl FlowStep {
    pub async fn pull_output_id(&mut self, session: &CachingSession, output_id: Uuid) -> Result<(), NodecosmosError> {
        if let Some(output_ids_by_node_id) = self.output_ids_by_node_id.as_mut() {
            for (_, input_ids) in output_ids_by_node_id.iter_mut() {
                input_ids.retain(|id| id != &output_id);
            }

            self.update_cb(session).await?;
        }

        Ok(())
    }

    pub async fn pull_input_id(&mut self, session: &CachingSession, input_id: Uuid) -> Result<(), NodecosmosError> {
        if let Some(input_ids_by_node_id) = self.input_ids_by_node_id.as_mut() {
            for (_, input_ids) in input_ids_by_node_id.iter_mut() {
                input_ids.retain(|id| id != &input_id);
            }

            self.update_cb(session).await?;
        }

        Ok(())
    }

    pub async fn remove_inputs(&mut self, session: &CachingSession) -> Result<(), NodecosmosError> {
        self.input_ids_by_node_id = None;
        self.update_cb(session).await?;

        Ok(())
    }

    pub(crate) async fn delete_fs_outputs(&mut self, session: &CachingSession) -> Result<(), NodecosmosError> {
        let output_ids_by_node_id = self.output_ids_by_node_id.clone();
        let id = self.id;
        let workflow = self.workflow(session).await?;

        if let Some(output_ids_by_node_id) = output_ids_by_node_id {
            if let Some(workflow) = workflow.as_ref() {
                let output_ids = output_ids_by_node_id.values().flatten().cloned().collect::<Vec<Uuid>>();
                Io::delete_by_ids(session, output_ids.clone(), workflow, Some(id)).await?;
            }
        }

        Ok(())
    }

    // removes outputs as inputs from next flow step
    pub async fn pull_outputs_from_next_flow_step(&mut self, session: &CachingSession) -> Result<(), NodecosmosError> {
        let mut next_flow_step = self.next_flow_step(session).await?;

        if let Some(next_flow_step) = next_flow_step.as_mut() {
            let output_ids_by_node_id = self.output_ids_by_node_id.clone();

            if let Some(output_ids_by_node_id) = output_ids_by_node_id {
                let output_ids = output_ids_by_node_id.values().flatten().cloned().collect::<Vec<Uuid>>();

                for id in output_ids {
                    next_flow_step.as_native().pull_input_id(session, id).await?;
                }
            }
        }

        Ok(())
    }

    // removes outputs as inputs from next workflow step
    pub async fn pull_outputs_from_next_workflow_step(
        &mut self,
        session: &CachingSession,
    ) -> Result<(), NodecosmosError> {
        let output_ids_by_node_id = self.output_ids_by_node_id.clone();
        let flow_step_id = self.id;
        let workflow = self.workflow(session).await?;

        if let (Some(output_ids_by_node_id), Some(workflow)) = (output_ids_by_node_id, workflow) {
            let output_ids = output_ids_by_node_id.values().flatten().cloned().collect::<Vec<Uuid>>();

            for id in output_ids {
                let mut output = Io::new();
                output.root_node_id = workflow.root_node_id;
                output.node_id = workflow.node_id;
                output.workflow_id = workflow.id;
                output.id = id;
                output.workflow = Some(workflow.clone());
                output.flow_step_id = Some(flow_step_id);

                output.pull_from_next_workflow_step(session).await?;
            }
        }

        Ok(())
    }

    // syncs the prev and next flow steps when a new flow step is deleted
    pub async fn sync_surrounding_fs_on_del(&mut self, session: &CachingSession) -> Result<(), NodecosmosError> {
        let mut prev_flow_step = self.prev_flow_step(session).await?;
        let mut next_flow_step = self.next_flow_step(session).await?;

        if let Some(prev_flow_step) = prev_flow_step.as_mut() {
            prev_flow_step.next_flow_step_id = self.next_flow_step_id;
            prev_flow_step.as_native().update_cb(session).await?;
        }

        if let Some(next_flow_step) = next_flow_step.as_mut() {
            next_flow_step.prev_flow_step_id = self.prev_flow_step_id;
            next_flow_step.as_native().update_cb(session).await?;
        }

        Ok(())
    }
}
