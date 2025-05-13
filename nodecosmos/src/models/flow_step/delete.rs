use charybdis::operations::{DeleteWithCallbacks, UpdateWithCallbacks};
use charybdis::types::Uuid;

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::flow_step::FlowStep;
use crate::models::io::Io;
use crate::models::traits::{Branchable, ModelContext};

impl FlowStep {
    pub async fn pull_input_id(&mut self, data: &RequestData, input_id: Uuid) -> Result<(), NodecosmosError> {
        if let Some(input_ids_by_node_id) = self.input_ids_by_node_id.as_mut() {
            for (_, input_ids) in input_ids_by_node_id.iter_mut() {
                input_ids.retain(|id| id != &input_id);
            }

            self.update_cb(data).execute(data.db_session()).await?;
        }

        Ok(())
    }

    pub async fn delete_outputs(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_branch() {
            return Ok(());
        }

        if let Some(output_ids_by_node_id) = &self.output_ids_by_node_id {
            for output_id in output_ids_by_node_id.values().flatten() {
                let mut output = Io {
                    root_id: self.root_id,
                    branch_id: self.branch_id,
                    node_id: self.node_id,
                    id: *output_id,
                    flow_step_id: Some(self.id),

                    ..Default::default()
                };

                output.set_parent_delete_context();
                output.delete_cb(data).execute(data.db_session()).await?;
            }
        }

        Ok(())
    }
}
