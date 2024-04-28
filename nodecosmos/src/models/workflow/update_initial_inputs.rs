use std::collections::HashMap;

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::update::BranchUpdate;
use crate::models::branch::Branch;
use crate::models::traits::Branchable;
use crate::models::workflow::UpdateInitialInputsWorkflow;

impl UpdateInitialInputsWorkflow {
    pub async fn update_branch(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        Branch::update(data, self.branch_id, BranchUpdate::EditNodeWorkflow(self.node_id)).await?;

        let original_wf = Self::maybe_find_first_by_branch_id_and_node_id(self.original_id(), self.node_id)
            .execute(data.db_session())
            .await?;

        if let Some(original_wf) = original_wf {
            let original_input_ids = original_wf.initial_input_ids.unwrap_or_default();
            let added_input_ids = self
                .initial_input_ids
                .clone()
                .unwrap_or_default()
                .iter()
                .filter_map(|id| {
                    if !original_input_ids.contains(id) {
                        Some(*id)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            let mut inputs = HashMap::new();
            inputs.insert(self.node_id, added_input_ids);

            Branch::update(data, self.branch_id, BranchUpdate::CreatedWorkflowInitialInputs(inputs)).await?;

            let removed_input_ids = original_input_ids
                .iter()
                .filter_map(|id| {
                    if !self.initial_input_ids.clone().unwrap_or_default().contains(id) {
                        Some(*id)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();

            let mut inputs = HashMap::new();
            inputs.insert(self.node_id, removed_input_ids);

            Branch::update(data, self.branch_id, BranchUpdate::DeleteWorkflowInitialInputs(inputs)).await?;
        } else {
            // wf is created within a branch
            let mut inputs = HashMap::new();
            inputs.insert(self.node_id, self.initial_input_ids.clone().unwrap_or_default());

            Branch::update(data, self.branch_id, BranchUpdate::CreatedWorkflowInitialInputs(inputs)).await?;
        }

        Ok(())
    }
}
