use charybdis::operations::Update;
use std::collections::HashMap;

use charybdis::types::Uuid;
use serde::{Deserialize, Serialize};

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::Branch;
use crate::models::traits::{Branchable, Merge};
use crate::models::workflow::UpdateInitialInputsWorkflow;

#[derive(Serialize, Deserialize)]
pub struct MergeWorkflows {
    created_workflow_initial_inputs: HashMap<Uuid, Vec<Uuid>>,
    deleted_workflow_initial_inputs: HashMap<Uuid, Vec<Uuid>>,
    combined: Vec<(Uuid, Vec<Uuid>)>,
    branch_id: Uuid,
}

impl MergeWorkflows {
    pub fn new(branch: &Branch) -> Self {
        let created_workflow_initial_inputs = branch.created_workflow_initial_inputs.clone().unwrap_or_default();
        let deleted_workflow_initial_inputs = branch.deleted_workflow_initial_inputs.clone().unwrap_or_default();
        let combined: Vec<(Uuid, Vec<Uuid>)> = created_workflow_initial_inputs
            .clone()
            .into_iter()
            .chain(deleted_workflow_initial_inputs.clone().into_iter())
            .collect();

        Self {
            created_workflow_initial_inputs,
            deleted_workflow_initial_inputs,
            combined,
            branch_id: branch.id,
        }
    }

    pub async fn update_initial_inputs(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        for (node_id, _) in &self.combined {
            let mut workflow = UpdateInitialInputsWorkflow::find_by_branch_id_and_node_id(self.branch_id, *node_id)
                .execute(data.db_session())
                .await?;

            if let Some(created_inputs) = self.created_workflow_initial_inputs.get(node_id) {
                workflow.initial_input_ids.merge(Some(created_inputs.clone()));
            }

            if let Some(deleted_inputs) = self.deleted_workflow_initial_inputs.get(node_id) {
                if let Some(initial_input_ids) = &mut workflow.initial_input_ids {
                    initial_input_ids.retain(|id| !deleted_inputs.contains(id));
                }
            }

            workflow.set_original_id();
            workflow.update().execute(data.db_session()).await?;
        }

        Ok(())
    }

    pub async fn undo_update_initial_inputs(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        for (node_id, _) in &self.combined {
            let mut workflow = UpdateInitialInputsWorkflow::find_by_branch_id_and_node_id(self.branch_id, *node_id)
                .execute(data.db_session())
                .await?;

            if let Some(created_inputs) = self.created_workflow_initial_inputs.get(node_id) {
                if let Some(initial_input_ids) = &mut workflow.initial_input_ids {
                    initial_input_ids.retain(|id| !created_inputs.contains(&id));
                }
            }

            if let Some(deleted_inputs) = self.deleted_workflow_initial_inputs.get(node_id) {
                workflow.initial_input_ids.merge(Some(deleted_inputs.clone()));
            }

            workflow.set_original_id();
            workflow.update().execute(data.db_session()).await?;
        }

        Ok(())
    }
}
