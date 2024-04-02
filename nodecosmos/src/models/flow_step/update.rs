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

    pub fn merge_original_inputs(&mut self, original: &FlowStep) {
        let mut original_inputs = original.input_ids_by_node_id.clone();
        original_inputs.merge_unique(self.input_ids_by_node_id.clone());

        self.input_ids_by_node_id = original_inputs;
    }

    pub fn merge_original_nodes(&mut self, original: &FlowStep) {
        let mut original_nodes = original.node_ids.clone();
        original_nodes.merge_unique(self.node_ids.clone());

        self.node_ids = original_nodes;
    }

    pub fn merge_original_outputs(&mut self, original: &FlowStep) {
        let mut original_outputs = original.output_ids_by_node_id.clone();
        original_outputs.merge_unique(self.output_ids_by_node_id.clone());

        self.output_ids_by_node_id = original_outputs;
    }
}
