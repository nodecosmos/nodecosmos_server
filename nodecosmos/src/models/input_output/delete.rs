use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::input_output::Io;
use crate::models::workflow::Workflow;
use charybdis::batch::ModelBatch;
use charybdis::types::Uuid;
use scylla::CachingSession;

impl Io {
    pub async fn delete_by_ids(
        data: &RequestData,
        ids: Vec<Uuid>,
        workflow: &Workflow,
        flow_step_id: Option<Uuid>,
    ) -> Result<(), NodecosmosError> {
        let mut batch = Self::delete_batch();

        for output_id in ids.iter() {
            let mut output = Io {
                root_id: workflow.root_id,
                branch_id: workflow.branch_id,
                node_id: workflow.node_id,
                id: *output_id,
                workflow: Some(workflow.clone()),
                flow_step_id,
                ..Default::default()
            };

            batch.append_delete(&output)?;

            output.pull_from_next_workflow_step(data).await?;
        }

        batch.execute(data.db_session()).await?;

        Ok(())
    }

    pub async fn pull_from_initial_input_ids(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        let id = self.id;
        let workflow = self.workflow(db_session).await?;

        let initial_input_ids = workflow.initial_input_ids.as_ref().unwrap();

        if initial_input_ids.contains(&id) {
            workflow.pull_initial_input_ids(&vec![id]).execute(db_session).await?;
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

    // remove output as input from next workflow step
    pub async fn pull_from_next_workflow_step(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let current_step_wf_index;
        let flow_step_id = self.flow_step_id;
        let id = self.id;
        let workflow = self.workflow(data.db_session()).await?;

        let diagram = workflow.diagram(data.db_session()).await?;

        // get current workflow index of flow step
        if let Some(flow_step_id) = flow_step_id {
            let wf_index = diagram.workflow_index(flow_step_id);

            if let Some(wf_index) = wf_index {
                current_step_wf_index = wf_index;
            } else {
                return Err(NodecosmosError::InternalServerError("MissingFlowStepIndex".to_string()));
            }
        } else {
            current_step_wf_index = 0;
        }

        // get next flow steps within diagram
        let next_wf_index = current_step_wf_index + 1;
        let next_flow_steps = diagram.flow_steps_by_wf_index(next_wf_index)?;

        if let Some(mut next_flow_steps) = next_flow_steps {
            for flow_step in next_flow_steps.iter_mut() {
                let mut flow_step = flow_step.lock()?;
                flow_step.pull_input_id(data, id).await?;
            }
        }

        Ok(())
    }
}
