use crate::api::data::RequestData;
use crate::constants::MAX_PARALLEL_REQUESTS;
use crate::errors::NodecosmosError;
use crate::models::flow_step::{FlowStep, UpdateOutputIdsFlowStep};
use crate::models::io::Io;
use crate::models::traits::ModelContext;
use crate::models::workflow::UpdateInitialInputsWorkflow;

impl Io {
    pub async fn pull_from_initial_input_ids(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.initial_input && !self.is_parent_delete_context() {
            UpdateInitialInputsWorkflow {
                branch_id: self.branch_id,
                node_id: self.node_id,
                ..Default::default()
            }
            .pull_initial_input(data.db_session(), self.id)
            .await?;
        }

        Ok(())
    }

    pub async fn pull_from_flow_step_outputs(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if !self.is_parent_delete_context() {
            if let (Some(flow_step_id), Some(flow_step_node_id)) = (self.flow_step_id, self.flow_step_node_id) {
                UpdateOutputIdsFlowStep::find_first_by_branch_id_and_id(self.branch_id, flow_step_id)
                    .execute(data.db_session())
                    .await?
                    .pull_output(data, flow_step_node_id, self.id)
                    .await?;
            }
        }

        Ok(())
    }

    pub async fn pull_from_flow_steps_inputs(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if let Some(flow_step_ids) = &self.inputted_by_flow_steps {
            let mut flow_steps = FlowStep::find_by_branch_id_and_node_id_and_ids(
                data.db_session(),
                self.branch_id,
                self.node_id,
                flow_step_ids,
            )
            .await?;

            for flow_step_chunks in &mut flow_steps.chunks_mut(MAX_PARALLEL_REQUESTS) {
                let mut futures = vec![];

                for flow_step in flow_step_chunks {
                    let future = flow_step.pull_input_id(data, self.id);

                    futures.push(future);
                }

                futures::future::try_join_all(futures).await?;
            }
        }

        Ok(())
    }
}
