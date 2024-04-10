use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::update::BranchUpdate;
use crate::models::branch::Branch;
use crate::models::flow_step::UpdateOutputIdsFlowStep;
use crate::models::traits::HashMapVecToSet;
use charybdis::model::AsNative;
use charybdis::types::Uuid;
use std::collections::{HashMap, HashSet};

impl UpdateOutputIdsFlowStep {
    pub async fn update_branch(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        let maybe_original = self.as_native().maybe_find_original(data.db_session()).await?;

        if let Some(original) = maybe_original {
            let original_node_outputs: Option<HashMap<Uuid, HashSet<Uuid>>> = original
                .output_ids_by_node_id
                .clone()
                .map(|output_ids_by_node_id| output_ids_by_node_id.hash_map_vec_to_set());

            let branched_node_outputs: Option<HashMap<Uuid, HashSet<Uuid>>> = self
                .output_ids_by_node_id
                .clone()
                .map(|output_ids_by_node_id| output_ids_by_node_id.hash_map_vec_to_set());

            let mut created_output_ids_by_node_id = HashMap::new();
            let mut removed_output_ids_by_node_id = HashMap::new();

            match (original_node_outputs, branched_node_outputs) {
                (Some(original_node_outputs), Some(branched_node_outputs)) => {
                    for (node_id, original_output_ids) in &original_node_outputs {
                        match branched_node_outputs.get(node_id) {
                            Some(current_output_ids) => {
                                // Calculate created and removed outputs.
                                let created: HashSet<Uuid> = current_output_ids
                                    .iter()
                                    .filter(|id| !original_output_ids.contains(id))
                                    .cloned()
                                    .collect();

                                let removed: HashSet<Uuid> = original_output_ids
                                    .iter()
                                    .filter(|id| !current_output_ids.contains(id))
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
                                removed_output_ids_by_node_id.insert(*node_id, original_output_ids.clone());
                            }
                        }
                    }

                    // Check for created outputs for node IDs only in the current map.
                    for (node_id, current_output_ids) in &branched_node_outputs {
                        if !original_node_outputs.contains_key(node_id) {
                            created_output_ids_by_node_id.insert(*node_id, current_output_ids.clone());
                        }
                    }
                }
                (None, Some(branched_node_outputs)) => {
                    // All current outputs are considered created.
                    for (node_id, current_output_ids) in &branched_node_outputs {
                        created_output_ids_by_node_id.insert(*node_id, current_output_ids.clone());
                    }
                }
                (Some(original_node_outputs), None) => {
                    // All original outputs are considered removed.
                    for (node_id, original_output_ids) in &original_node_outputs {
                        removed_output_ids_by_node_id.insert(*node_id, original_output_ids.clone());
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
                data,
                self.branch_id,
                BranchUpdate::CreatedFlowStepOutputs(created_node_outputs_by_fow_step),
            )
            .await?;

            Branch::update(
                data,
                self.branch_id,
                BranchUpdate::DeletedFlowStepOutputs(deleted_node_outputs_by_flow_step),
            )
            .await?;
        }

        Ok(())
    }
}
