use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::update::BranchUpdate;
use crate::models::branch::Branch;
use crate::models::flow_step::UpdateInputIdsFlowStep;
use crate::models::traits::hash_map::HashMapVecValToSet;
use charybdis::model::AsNative;
use charybdis::types::{Set, Uuid};
use std::collections::{HashMap, HashSet};

impl UpdateInputIdsFlowStep {
    pub async fn update_branch(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        let maybe_original = self.as_native().maybe_find_original(data.db_session()).await?;

        if let Some(original) = maybe_original {
            let original_node_inputs: Option<HashMap<Uuid, HashSet<Uuid>>> = original
                .input_ids_by_node_id
                .clone()
                .map(|input_ids_by_node_id| input_ids_by_node_id.hash_map_vec_val_to_set());

            let branched_node_inputs: Option<HashMap<Uuid, HashSet<Uuid>>> = self
                .input_ids_by_node_id
                .clone()
                .map(|input_ids_by_node_id| input_ids_by_node_id.hash_map_vec_val_to_set());

            let mut created_input_ids_by_node_id = HashMap::new();
            let mut removed_input_ids_by_node_id = HashMap::new();

            match (original_node_inputs, branched_node_inputs) {
                (Some(original_node_inputs), Some(branched_node_inputs)) => {
                    for (node_id, original_input_ids) in &original_node_inputs {
                        match branched_node_inputs.get(node_id) {
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
                    for (node_id, current_input_ids) in &branched_node_inputs {
                        if !original_node_inputs.contains_key(node_id) {
                            created_input_ids_by_node_id.insert(*node_id, current_input_ids.clone());
                        }
                    }
                }
                (None, Some(branched_node_inputs)) => {
                    // All current inputs are considered created
                    for (node_id, current_input_ids) in &branched_node_inputs {
                        created_input_ids_by_node_id.insert(*node_id, current_input_ids.clone());
                    }
                }
                (Some(original_node_inputs), None) => {
                    // All original inputs are considered removed
                    for (node_id, original_input_ids) in &original_node_inputs {
                        removed_input_ids_by_node_id.insert(*node_id, original_input_ids.clone());
                    }
                }
                (None, None) => {
                    // No inputs to update
                }
            }

            let mut created_node_inputs_by_fow_step: Option<HashMap<Uuid, HashMap<Uuid, HashSet<Uuid>>>> = None;
            let mut deleted_node_inputs_by_flow_step: Option<HashMap<Uuid, HashMap<Uuid, HashSet<Uuid>>>> = None;

            if !created_input_ids_by_node_id.is_empty() {
                let mut value = HashMap::new();
                value.insert(self.id, created_input_ids_by_node_id);

                created_node_inputs_by_fow_step = Some(value);
            }

            if !removed_input_ids_by_node_id.is_empty() {
                let mut value = HashMap::new();
                value.insert(self.id, removed_input_ids_by_node_id);

                deleted_node_inputs_by_flow_step = Some(value);
            }

            Branch::update(
                data,
                self.branch_id,
                BranchUpdate::CreatedFlowStepInputs(created_node_inputs_by_fow_step),
            )
            .await?;

            Branch::update(
                data,
                self.branch_id,
                BranchUpdate::DeletedFlowStepInputs(deleted_node_inputs_by_flow_step),
            )
            .await?;
        }

        Ok(())
    }
}
