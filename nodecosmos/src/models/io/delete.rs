use crate::api::data::RequestData;
use crate::constants::MAX_PARALLEL_REQUESTS;
use crate::errors::NodecosmosError;
use crate::models::flow_step::FlowStep;
use crate::models::io::Io;
use crate::models::workflow::Workflow;
use scylla::CachingSession;

impl Io {
    pub async fn pull_from_initial_input_ids(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        Workflow {
            root_id: self.root_id,
            branch_id: self.branch_id,
            node_id: self.node_id,
            ..Default::default()
        }
        .pull_initial_input_ids(&vec![self.id])
        .execute(db_session)
        .await?;

        Ok(())
    }

    pub async fn pull_from_flow_steps_inputs(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if let Some(flow_step_ids) = &self.inputted_by_flow_steps {
            let flow_steps = FlowStep::find_by_node_id_and_branch_id_and_ids(
                data.db_session(),
                self.node_id,
                self.branch_id,
                flow_step_ids,
            )
            .await?;

            for flow_step in &mut flow_steps.chunks(MAX_PARALLEL_REQUESTS) {
                let mut flow_step = flow_step.to_vec();
                let mut futures = vec![];

                for flow_step in &mut flow_step {
                    let future = flow_step.pull_input_id(data, self.id);

                    futures.push(future);
                }

                futures::future::try_join_all(futures).await?;
            }
        }

        Ok(())
    }

    pub async fn pull_form_flow_step_outputs(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let id = self.id;
        let flow_step = self.flow_step(data.db_session()).await?;

        if let Some(flow_step) = &mut flow_step.as_mut() {
            flow_step.pull_output_id(data, id).await?;
        }

        Ok(())
    }
}
