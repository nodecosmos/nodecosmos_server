use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::update::BranchUpdate;
use crate::models::branch::Branch;
use crate::models::flow_step::UpdateInputIdsFlowStep;
use crate::models::traits::hash_map::HashMapVecValToSet;
use charybdis::model::AsNative;
use charybdis::types::{Frozen, Map, Set, Uuid};
use std::collections::{HashMap, HashSet};

impl UpdateInputIdsFlowStep {
    pub async fn update_branch(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        let maybe_original = self.as_native().maybe_find_original(data.db_session()).await?;

        if let Some(original) = maybe_original {
            let original_input_ids_by_node_id: Option<HashMap<Uuid, HashSet<Uuid>>> = original
                .input_ids_by_node_id
                .clone()
                .map(|input_ids_by_node_id| input_ids_by_node_id.hash_map_vec_val_to_set());

            let current_input_ids_by_node_id: Option<HashMap<Uuid, HashSet<Uuid>>> = self
                .input_ids_by_node_id
                .clone()
                .map(|input_ids_by_node_id| input_ids_by_node_id.hash_map_vec_val_to_set());

            let mut created_input_ids_by_node_id = HashMap::new();
            let mut removed_input_ids_by_node_id = HashMap::new();

            match (original_input_ids_by_node_id, current_input_ids_by_node_id) {
                (Some(original_input_ids_by_node_id), Some(current_input_ids_by_node_id)) => {
                    for (node_id, original_input_ids) in &original_input_ids_by_node_id {
                        match current_input_ids_by_node_id.get(node_id) {
                            Some(current_input_ids) => {
                                // Calculate created and removed inputs
                                let created: Set<Uuid> = current_input_ids
                                    .iter()
                                    .filter(|id| !original_input_ids.contains(id))
                                    .cloned()
                                    .collect();

                                let removed: Set<Uuid> = original_input_ids
                                    .iter()
                                    .filter(|id| !current_input_ids.contains(id))
                                    .cloned()
                                    .collect();

                                if !created.is_empty() {
                                    created_input_ids_by_node_id.insert(*node_id, created);
                                }
                                if !removed.is_empty() {
                                    removed_input_ids_by_node_id.insert(*node_id, removed);
                                }
                            }
                            None => {
                                // All original inputs for this node_id are considered removed
                                removed_input_ids_by_node_id.insert(*node_id, original_input_ids.clone());
                            }
                        }
                    }

                    // Check for created inputs for node IDs only in the current map
                    for (node_id, current_input_ids) in &current_input_ids_by_node_id {
                        if !original_input_ids_by_node_id.contains_key(node_id) {
                            created_input_ids_by_node_id.insert(*node_id, current_input_ids.clone());
                        }
                    }
                }
                (None, Some(current_input_ids_by_node_id)) => {
                    // All current inputs are considered created
                    for (node_id, current_input_ids) in &current_input_ids_by_node_id {
                        created_input_ids_by_node_id.insert(*node_id, current_input_ids.clone());
                    }
                }
                (Some(original_input_ids_by_node_id), None) => {
                    // All original inputs are considered removed
                    for (node_id, original_input_ids) in &original_input_ids_by_node_id {
                        removed_input_ids_by_node_id.insert(*node_id, original_input_ids.clone());
                    }
                }
                (None, None) => {
                    // No inputs to update
                }
            }

            let mut created_input_ids_by_fs_id_and_node_id: Option<HashMap<Uuid, HashMap<Uuid, HashSet<Uuid>>>> = None;
            let mut removed_input_ids_by_fs_id_and_node_id: Option<HashMap<Uuid, HashMap<Uuid, HashSet<Uuid>>>> = None;

            if !created_input_ids_by_node_id.is_empty() {
                let mut value = HashMap::new();
                value.insert(self.id, created_input_ids_by_node_id);

                created_input_ids_by_fs_id_and_node_id = Some(value);
            }

            if !removed_input_ids_by_node_id.is_empty() {
                let mut value = HashMap::new();
                value.insert(self.id, removed_input_ids_by_node_id);

                removed_input_ids_by_fs_id_and_node_id = Some(value);
            }

            Branch::update(
                data,
                self.branch_id,
                BranchUpdate::CreatedFlowStepInputs(created_input_ids_by_fs_id_and_node_id),
            )
            .await?;

            Branch::update(
                data,
                self.branch_id,
                BranchUpdate::DeletedFlowStepInputs(removed_input_ids_by_fs_id_and_node_id),
            )
            .await?;
        }

        Ok(())
    }
}
