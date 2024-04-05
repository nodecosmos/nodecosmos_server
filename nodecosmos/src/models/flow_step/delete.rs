use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::flow_step::{FlowStep, SiblingFlowStep};
use crate::models::input_output::Io;
use crate::models::traits::ModelContext;
use charybdis::batch::ModelBatch;
use charybdis::model::AsNative;
use charybdis::operations::{DeleteWithCallbacks, UpdateWithCallbacks};
use charybdis::types::Uuid;

impl FlowStep {
    pub async fn pull_output_id(&mut self, data: &RequestData, output_id: Uuid) -> Result<(), NodecosmosError> {
        if let Some(output_ids_by_node_id) = self.output_ids_by_node_id.as_mut() {
            for (_, input_ids) in output_ids_by_node_id.iter_mut() {
                input_ids.retain(|id| id != &output_id);
            }

            self.update_cb(data).execute(data.db_session()).await?;
        }

        Ok(())
    }

    pub async fn pull_input_id(&mut self, data: &RequestData, input_id: Uuid) -> Result<(), NodecosmosError> {
        if let Some(input_ids_by_node_id) = self.input_ids_by_node_id.as_mut() {
            for (_, input_ids) in input_ids_by_node_id.iter_mut() {
                input_ids.retain(|id| id != &input_id);
            }

            self.update_cb(data).execute(data.db_session()).await?;
        }

        Ok(())
    }

    pub async fn remove_inputs(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        self.input_ids_by_node_id = None;
        self.update_cb(data).execute(data.db_session()).await?;

        Ok(())
    }

    pub async fn delete_fs_outputs(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        let output_ids_by_node_id = self.output_ids_by_node_id.clone();
        let id = self.id;

        if let Some(output_ids_by_node_id) = output_ids_by_node_id {
            let workflow_borrow = self.workflow(data.db_session()).await?.lock()?;
            let workflow = workflow_borrow.as_ref().unwrap();

            let output_ids = output_ids_by_node_id.values().flatten().cloned().collect::<Vec<Uuid>>();
            for output_id in output_ids {
                let mut output = Io {
                    root_id: workflow.root_id,
                    branch_id: workflow.branch_id,
                    node_id: workflow.node_id,
                    id: output_id,
                    flow_step_id: Some(id),
                    flow_step: Some(self.clone()),

                    ..Default::default()
                };

                output.set_parent_delete_context();
                output.delete_cb(data).execute(data.db_session()).await?;
            }
        }

        Ok(())
    }

    // removes current step's outputs from next flow step's inputs
    pub async fn pull_outputs_from_next_flow_step(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let output_ids_by_node_id = self.output_ids_by_node_id.clone();
        let next_flow_step = self.next_flow_step(data).await?.lock()?;

        if let Some(next_flow_step) = next_flow_step.as_ref() {
            if let Some(output_ids_by_node_id) = output_ids_by_node_id {
                let output_ids = output_ids_by_node_id.values().flatten().cloned().collect::<Vec<Uuid>>();

                for id in output_ids {
                    next_flow_step.as_native().pull_input_id(data, id).await?;
                }
            }
        }

        Ok(())
    }

    // syncs the prev and next flow steps when a flow step is deleted
    pub async fn sync_surrounding_fs_on_deletion(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_parent_delete_context() {
            return Ok(());
        }

        let mut prev_flow_step = self.prev_flow_step(data).await?.lock()?;
        let mut next_flow_step = self.next_flow_step(data).await?.lock()?;

        match (prev_flow_step.as_mut(), next_flow_step.as_mut()) {
            (Some(prev_fs), Some(next_fs)) => {
                prev_fs.next_flow_step_id = Some(next_fs.id);
                next_fs.prev_flow_step_id = Some(prev_fs.id);

                prev_fs.updated_at = chrono::Utc::now();
                next_fs.updated_at = chrono::Utc::now();

                SiblingFlowStep::batch()
                    .append_update(prev_fs)
                    .append_update(next_fs)
                    .execute(data.db_session())
                    .await?;
            }
            (Some(prev_fs), None) => {
                prev_fs.next_flow_step_id = None;
                prev_fs.update_cb(data).execute(data.db_session()).await?;
            }
            (None, Some(next_fs)) => {
                next_fs.prev_flow_step_id = None;
                next_fs.update_cb(data).execute(data.db_session()).await?;
            }
            _ => {}
        }

        Ok(())
    }
}
