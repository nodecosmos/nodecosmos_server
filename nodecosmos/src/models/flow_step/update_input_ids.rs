use std::collections::{HashMap, HashSet};

use anyhow::Context;
use charybdis::batch::ModelBatch;
use charybdis::model::AsNative;
use charybdis::operations::Find;
use charybdis::types::Uuid;

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::update::BranchUpdate;
use crate::models::branch::Branch;
use crate::models::flow_step::UpdateInputIdsFlowStep;
use crate::models::io::Io;
use crate::models::traits::{Branchable, HashMapVecToSet};

impl UpdateInputIdsFlowStep {
    pub async fn preserve_branch_ios(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_branched() {
            // for each input and output create branch io if it does not exist
            let io_ids = self
                .input_ids_by_node_id
                .clone()
                .unwrap_or_default()
                .into_values()
                .flatten()
                .into_iter()
                .collect();

            let ios =
                Io::find_by_branch_id_and_root_id_and_ids(data.db_session(), self.original_id(), self.root_id, &io_ids)
                    .await?
                    .into_iter()
                    .map(|mut io| {
                        io.branch_id = self.branch_id;

                        io
                    })
                    .collect::<Vec<Io>>();

            Io::unlogged_batch()
                .append_inserts_if_not_exist(&ios)
                .execute(data.db_session())
                .await
                .context("Failed to preserve branch ios")?;
        }

        Ok(())
    }

    pub async fn update_ios(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        let current = self.find_by_primary_key().execute(data.db_session()).await?;

        let current_input_ids: HashSet<Uuid> = current
            .input_ids_by_node_id
            .clone()
            .unwrap_or_default()
            .into_values()
            .flatten()
            .collect();

        let update_input_ids: HashSet<Uuid> = self
            .input_ids_by_node_id
            .clone()
            .unwrap_or_default()
            .into_values()
            .flatten()
            .collect();

        let added_input_ids: HashSet<Uuid> = update_input_ids.difference(&current_input_ids).cloned().collect();
        let removed_input_ids: HashSet<Uuid> = current_input_ids.difference(&update_input_ids).cloned().collect();

        let mut batch = Io::statement_batch();

        if added_input_ids.len() > 0 {
            for added_io in added_input_ids {
                batch.append_statement(
                    Io::PUSH_INPUTTED_BY_FLOW_STEPS_QUERY,
                    (vec![self.id], self.branch_id, self.root_id, added_io),
                );
            }
        }

        if removed_input_ids.len() > 0 {
            for removed_io in removed_input_ids {
                batch.append_statement(
                    Io::PULL_INPUTTED_BY_FLOW_STEPS_QUERY,
                    (vec![self.id], self.branch_id, self.root_id, removed_io),
                );
            }
        }

        batch.execute(data.db_session()).await?;

        Ok(())
    }

    pub async fn update_branch(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        let maybe_original = self.as_native().maybe_find_original(data.db_session()).await?;

        if let Some(original) = maybe_original {
            let original_node_inputs: Option<HashMap<Uuid, HashSet<Uuid>>> = original
                .input_ids_by_node_id
                .clone()
                .map(|input_ids_by_node_id| input_ids_by_node_id.hash_map_vec_to_set());

            let branched_node_inputs: Option<HashMap<Uuid, HashSet<Uuid>>> = self
                .input_ids_by_node_id
                .clone()
                .map(|input_ids_by_node_id| input_ids_by_node_id.hash_map_vec_to_set());

            let mut created_input_ids_by_node_id = HashMap::new();
            let mut removed_input_ids_by_node_id = HashMap::new();

            match (original_node_inputs, branched_node_inputs) {
                (Some(original_node_inputs), Some(branched_node_inputs)) => {
                    for (node_id, original_input_ids) in &original_node_inputs {
                        match branched_node_inputs.get(node_id) {
                            Some(current_input_ids) => {
                                // Calculate created and removed inputs.
                                let created: HashSet<Uuid> = current_input_ids
                                    .iter()
                                    .filter(|id| !original_input_ids.contains(id))
                                    .cloned()
                                    .collect();

                                let removed: HashSet<Uuid> = original_input_ids
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
                                // All original inputs for this node_id are considered removed.
                                removed_input_ids_by_node_id.insert(*node_id, original_input_ids.clone());
                            }
                        }
                    }

                    // Check for created inputs for node IDs only in the current map.
                    for (node_id, current_input_ids) in &branched_node_inputs {
                        if !original_node_inputs.contains_key(node_id) {
                            created_input_ids_by_node_id.insert(*node_id, current_input_ids.clone());
                        }
                    }
                }
                (None, Some(branched_node_inputs)) => {
                    // All current inputs are considered created.
                    for (node_id, current_input_ids) in &branched_node_inputs {
                        created_input_ids_by_node_id.insert(*node_id, current_input_ids.clone());
                    }
                }
                (Some(original_node_inputs), None) => {
                    // All original inputs are considered removed.
                    for (node_id, original_input_ids) in &original_node_inputs {
                        removed_input_ids_by_node_id.insert(*node_id, original_input_ids.clone());
                    }
                }
                (None, None) => {
                    // No inputs to update.
                }
            }

            let mut created_node_inputs_by_fow_step = HashMap::new();
            created_node_inputs_by_fow_step.insert(self.id, created_input_ids_by_node_id);

            let mut deleted_node_inputs_by_flow_step = HashMap::new();
            deleted_node_inputs_by_flow_step.insert(self.id, removed_input_ids_by_node_id);

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
