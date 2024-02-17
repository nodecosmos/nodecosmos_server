use crate::errors::NodecosmosError;
use crate::models::flow_step::FlowStep;
use crate::models::input_output::Io;
use crate::utils::cloned_ref::ClonedRef;
use charybdis::operations::UpdateWithCallbacks;
use scylla::CachingSession;

impl FlowStep {
    // delete outputs models
    pub async fn delete_outputs_from_removed_nodes(&mut self, session: &CachingSession) -> Result<(), NodecosmosError> {
        let output_ids_by_node_id = self.output_ids_by_node_id.clone();
        let cloned_node_ids = self.node_ids.cloned_ref();
        let id = self.id;
        let workflow = self.workflow(session).await?;

        if let Some(output_ids_by_node_id) = output_ids_by_node_id {
            if let Some(workflow) = workflow.as_ref() {
                for (node_id, output_ids) in output_ids_by_node_id.iter() {
                    if !cloned_node_ids.contains(node_id) {
                        Io::delete_by_ids(session, output_ids.clone(), workflow, Some(id)).await?;
                    }
                }
            }
        }

        Ok(())
    }

    // remove outputs references
    pub async fn remove_outputs_from_removed_nodes(&mut self, session: &CachingSession) -> Result<(), NodecosmosError> {
        let node_ids = self.node_ids.cloned_ref();

        if let Some(output_ids_by_node_id) = self.output_ids_by_node_id.as_mut() {
            output_ids_by_node_id.retain(|node_id, _| node_ids.contains(node_id));
            self.update_cb(&None).execute(session).await?;
        }

        Ok(())
    }

    // remove inputs references
    pub async fn remove_inputs_from_removed_nodes(&mut self, session: &CachingSession) -> Result<(), NodecosmosError> {
        let node_ids = self.node_ids.cloned_ref();

        if let Some(input_ids_by_node_id) = self.input_ids_by_node_id.as_mut() {
            input_ids_by_node_id.retain(|node_id, _| node_ids.contains(node_id));
            self.update_cb(&None).execute(session).await?;
        }

        Ok(())
    }
}
