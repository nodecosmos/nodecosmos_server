use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::flow_step::FlowStep;
use crate::models::input_output::Io;
use crate::models::traits::ref_cloned::RefCloned;
use crate::models::traits::Merge;
use charybdis::operations::{DeleteWithCallbacks, UpdateWithCallbacks};

impl FlowStep {
    // delete outputs models
    pub async fn delete_outputs_from_removed_nodes(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let output_ids_by_node_id = self.output_ids_by_node_id.clone();
        let cloned_node_ids = self.node_ids.ref_cloned();
        let id = self.id;
        let workflow = self.workflow(data.db_session()).await?;

        if let Some(output_ids_by_node_id) = output_ids_by_node_id {
            for (node_id, output_ids) in output_ids_by_node_id.iter() {
                if !cloned_node_ids.contains(node_id) {
                    for output_id in output_ids {
                        let mut output = Io {
                            root_id: workflow.root_id,
                            branch_id: workflow.branch_id,
                            node_id: workflow.node_id,
                            id: *output_id,
                            workflow: Some(workflow.clone()),
                            flow_step_id: Some(id),

                            ..Default::default()
                        };

                        output.delete_cb(data).execute(data.db_session()).await?;
                    }
                }
            }
        }

        Ok(())
    }

    // remove outputs references
    pub async fn remove_outputs_from_removed_nodes(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let node_ids = self.node_ids.ref_cloned();

        if let Some(output_ids_by_node_id) = self.output_ids_by_node_id.as_mut() {
            output_ids_by_node_id.retain(|node_id, _| node_ids.contains(node_id));
            self.update_cb(data).execute(data.db_session()).await?;
        }

        Ok(())
    }

    // remove inputs references
    pub async fn remove_inputs_from_removed_nodes(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let node_ids = self.node_ids.ref_cloned();

        if let Some(input_ids_by_node_id) = self.input_ids_by_node_id.as_mut() {
            input_ids_by_node_id.retain(|node_id, _| node_ids.contains(node_id));
            self.update_cb(data).execute(data.db_session()).await?;
        }

        Ok(())
    }

    pub fn merge_inputs(&mut self, original: &FlowStep) {
        self.input_ids_by_node_id.merge(original.input_ids_by_node_id.clone());
    }

    pub fn merge_nodes(&mut self, original: &FlowStep) {
        self.node_ids.merge_unique(original.node_ids.clone());
    }

    pub fn merge_outputs(&mut self, original: &FlowStep) {
        self.output_ids_by_node_id.merge(original.output_ids_by_node_id.clone());
    }
}
