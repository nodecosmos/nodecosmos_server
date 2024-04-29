use scylla::CachingSession;

use crate::api::data::RequestData;
use crate::constants::MAX_PARALLEL_REQUESTS;
use crate::errors::NodecosmosError;
use crate::models::flow_step::FlowStep;
use crate::models::io::Io;
use crate::models::traits::{Branchable, FindOrInsertBranched, ModelBranchParams};
use crate::models::workflow::Workflow;

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

    pub async fn pull_form_flow_step_outputs(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let id = self.id;

        if let Some(flow_step_id) = self.flow_step_id {
            let mut fs = FlowStep::find_or_insert_branched(
                data,
                ModelBranchParams {
                    original_id: self.original_id(),
                    branch_id: self.branch_id,
                    node_id: self.node_id,
                    id: flow_step_id,
                },
            )
            .await?;
            fs.pull_output_id(data, id).await?;
        }

        Ok(())
    }
}
