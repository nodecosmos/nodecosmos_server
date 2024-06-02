use std::collections::{HashMap, HashSet};

use charybdis::types::Uuid;

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::update::BranchUpdate;
use crate::models::branch::Branch;
use crate::models::flow_step::{FlowStep, UpdateOutputIdsFlowStep};
use crate::models::traits::{Branchable, FindOrInsertBranched, HashMapVecToSet, ModelBranchParams};

impl UpdateOutputIdsFlowStep {
    pub async fn update_branch(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        Branch::update(data.db_session(), self.branch_id, BranchUpdate::EditNode(self.node_id)).await?;

        let current = FlowStep::find_or_insert_branched(
            data,
            ModelBranchParams {
                original_id: self.original_id(),
                branch_id: self.branch_id,
                node_id: self.node_id,
                id: self.id,
            },
        )
        .await?;

        current.preserve_flow_step_outputs(data).await?;

        let current_db_outputs: Option<HashMap<Uuid, HashSet<Uuid>>> = current
            .output_ids_by_node_id
            .clone()
            .map(|output_ids_by_node_id| output_ids_by_node_id.hash_map_vec_to_set());

        let new_node_outputs: Option<HashMap<Uuid, HashSet<Uuid>>> = self
            .output_ids_by_node_id
            .clone()
            .map(|output_ids_by_node_id| output_ids_by_node_id.hash_map_vec_to_set());

        let mut created_output_ids_by_node_id = HashMap::new();
        let mut removed_output_ids_by_node_id = HashMap::new();

        match (current_db_outputs, new_node_outputs) {
            (Some(current_db_outputs), Some(new_node_outputs)) => {
                // // handle current db outputs delta
                for (node_id, current_db_output_ids) in &current_db_outputs {
                    match new_node_outputs.get(node_id) {
                        Some(new_output_ids) => {
                            // Calculate created and removed outputs.
                            let created: HashSet<Uuid> = new_output_ids
                                .iter()
                                .filter(|id| !current_db_output_ids.contains(id))
                                .cloned()
                                .collect();

                            let removed: HashSet<Uuid> = current_db_output_ids
                                .iter()
                                .filter(|id| !new_output_ids.contains(id))
                                .cloned()
                                .collect();

                            if !created.is_empty() {
                                created_output_ids_by_node_id.insert(*node_id, created);
                            }
                            if !removed.is_empty() {
                                removed_output_ids_by_node_id.insert(*node_id, removed);
                            }
                        }
                        None => {
                            // All original outputs for this node_id are considered removed.
                            removed_output_ids_by_node_id.insert(*node_id, current_db_output_ids.clone());
                        }
                    }
                }

                // handle new outputs that are not in current db outputs
                for (node_id, new_output_ids) in &new_node_outputs {
                    if !current_db_outputs.contains_key(node_id) {
                        created_output_ids_by_node_id.insert(*node_id, new_output_ids.clone());
                    }
                }
            }
            (None, Some(new_node_outputs)) => {
                // All new_output_ids outputs are considered created.
                for (node_id, new_output_ids) in &new_node_outputs {
                    created_output_ids_by_node_id.insert(*node_id, new_output_ids.clone());
                }
            }
            (Some(current_db_outputs), None) => {
                // All current_db outputs are considered removed.
                for (node_id, current_db_output_ids) in &current_db_outputs {
                    removed_output_ids_by_node_id.insert(*node_id, current_db_output_ids.clone());
                }
            }
            (None, None) => {
                // No outputs to update.
            }
        }

        let mut created_node_outputs_by_fow_step = HashMap::new();
        created_node_outputs_by_fow_step.insert(self.id, created_output_ids_by_node_id);

        let mut deleted_node_outputs_by_flow_step = HashMap::new();
        deleted_node_outputs_by_flow_step.insert(self.id, removed_output_ids_by_node_id);

        Branch::update(
            data.db_session(),
            self.branch_id,
            BranchUpdate::CreateFlowStepOutputs(created_node_outputs_by_fow_step),
        )
        .await?;

        Branch::update(
            data.db_session(),
            self.branch_id,
            BranchUpdate::DeletedFlowStepOutputs(deleted_node_outputs_by_flow_step),
        )
        .await?;

        Ok(())
    }
}
