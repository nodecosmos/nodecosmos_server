use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::update::BranchUpdate;
use crate::models::branch::Branch;
use crate::models::flow_step::UpdateOutputIdsFlowStep;
use crate::models::traits::hash_map::HashMapVecValToSet;
use charybdis::model::AsNative;
use charybdis::types::{Set, Uuid};
use std::collections::{HashMap, HashSet};

impl UpdateOutputIdsFlowStep {
    pub async fn update_branch(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        let maybe_original = self.as_native().maybe_find_original(data.db_session()).await?;

        if let Some(original) = maybe_original {
            let original_output_ids_by_node_id: Option<HashMap<Uuid, HashSet<Uuid>>> = original
                .output_ids_by_node_id
                .clone()
                .map(|output_ids_by_node_id| output_ids_by_node_id.hash_map_vec_val_to_set());

            let current_output_ids_by_node_id: Option<HashMap<Uuid, HashSet<Uuid>>> = self
                .output_ids_by_node_id
                .clone()
                .map(|output_ids_by_node_id| output_ids_by_node_id.hash_map_vec_val_to_set());

            let mut created_output_ids_by_node_id = HashMap::new();
            let mut removed_output_ids_by_node_id = HashMap::new();

            match (original_output_ids_by_node_id, current_output_ids_by_node_id) {
                (Some(original_output_ids_by_node_id), Some(current_output_ids_by_node_id)) => {
                    for (node_id, original_output_ids) in &original_output_ids_by_node_id {
                        match current_output_ids_by_node_id.get(node_id) {
                            Some(current_output_ids) => {
                                // Calculate created and removed outputs
                                let created: Set<Uuid> = current_output_ids
                                    .iter()
                                    .filter(|id| !original_output_ids.contains(id))
                                    .cloned()
                                    .collect();

                                let removed: Set<Uuid> = original_output_ids
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
                                // All original outputs for this node_id are considered removed
                                removed_output_ids_by_node_id.insert(*node_id, original_output_ids.clone());
                            }
                        }
                    }

                    // Check for created outputs for node IDs only in the current map
                    for (node_id, current_output_ids) in &current_output_ids_by_node_id {
                        if !original_output_ids_by_node_id.contains_key(node_id) {
                            created_output_ids_by_node_id.insert(*node_id, current_output_ids.clone());
                        }
                    }
                }
                (None, Some(current_output_ids_by_node_id)) => {
                    // All current outputs are considered created
                    for (node_id, current_output_ids) in &current_output_ids_by_node_id {
                        created_output_ids_by_node_id.insert(*node_id, current_output_ids.clone());
                    }
                }
                (Some(original_output_ids_by_node_id), None) => {
                    // All original outputs are considered removed
                    for (node_id, original_output_ids) in &original_output_ids_by_node_id {
                        removed_output_ids_by_node_id.insert(*node_id, original_output_ids.clone());
                    }
                }
                (None, None) => {
                    // No outputs to update
                }
            }

            let mut created_output_ids_by_fs_id_and_node_id = None;

            let mut removed_output_ids_by_fs_id_and_node_id = None;

            if !created_output_ids_by_node_id.is_empty() {
                let mut value = HashMap::new();
                value.insert(self.id, created_output_ids_by_node_id);

                created_output_ids_by_fs_id_and_node_id = Some(value);
            }

            if !removed_output_ids_by_node_id.is_empty() {
                let mut value = HashMap::new();
                value.insert(self.id, removed_output_ids_by_node_id);

                removed_output_ids_by_fs_id_and_node_id = Some(value);
            }

            Branch::update(
                data,
                self.branch_id,
                BranchUpdate::CreatedFlowStepInputs(created_output_ids_by_fs_id_and_node_id),
            )
            .await?;

            Branch::update(
                data,
                self.branch_id,
                BranchUpdate::DeletedFlowStepInputs(removed_output_ids_by_fs_id_and_node_id),
            )
            .await?;
        }

        Ok(())
    }
}
