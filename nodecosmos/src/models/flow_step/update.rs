use crate::models::flow_step::FlowStep;
use crate::models::traits::Merge;

impl FlowStep {
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
