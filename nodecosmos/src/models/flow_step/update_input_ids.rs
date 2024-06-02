use std::collections::{HashMap, HashSet};

use anyhow::Context;
use charybdis::batch::ModelBatch;
use charybdis::operations::Find;
use charybdis::types::Uuid;

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::update::BranchUpdate;
use crate::models::branch::Branch;
use crate::models::flow_step::{FlowStep, UpdateInputIdsFlowStep};
use crate::models::io::Io;
use crate::models::traits::{Branchable, FindOrInsertBranched, HashMapVecToSet, ModelBranchParams};

impl UpdateInputIdsFlowStep {
    pub async fn preserve_branch_ios(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_branch() {
            // for each input and output create branch io if it does not exist
            let io_ids: Vec<Uuid> = self
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

        let new_input_ids: HashSet<Uuid> = current
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

        let added_input_ids: HashSet<Uuid> = update_input_ids.difference(&new_input_ids).cloned().collect();
        let removed_input_ids: HashSet<Uuid> = new_input_ids.difference(&update_input_ids).cloned().collect();

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

        self.preserve_branch_ios(data).await?;

        let current_db_inputs: Option<HashMap<Uuid, HashSet<Uuid>>> = current
            .input_ids_by_node_id
            .clone()
            .map(|input_ids_by_node_id| input_ids_by_node_id.hash_map_vec_to_set());

        let new_node_inputs: Option<HashMap<Uuid, HashSet<Uuid>>> = self
            .input_ids_by_node_id
            .clone()
            .map(|input_ids_by_node_id| input_ids_by_node_id.hash_map_vec_to_set());

        let mut created_input_ids_by_node_id = HashMap::new();
        let mut removed_input_ids_by_node_id = HashMap::new();

        match (current_db_inputs, new_node_inputs) {
            (Some(current_db_inputs), Some(new_node_inputs)) => {
                // handle current db inputs delta
                for (node_id, current_db_input_ids) in &current_db_inputs {
                    match new_node_inputs.get(node_id) {
                        Some(new_input_ids) => {
                            // Calculate created and removed inputs.
                            let created: HashSet<Uuid> = new_input_ids
                                .iter()
                                .filter(|id| !current_db_input_ids.contains(id))
                                .cloned()
                                .collect();

                            let removed: HashSet<Uuid> = current_db_input_ids
                                .iter()
                                .filter(|id| !new_input_ids.contains(id))
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
                            removed_input_ids_by_node_id.insert(*node_id, current_db_input_ids.clone());
                        }
                    }
                }

                // handle new inputs that are not in current db outputs
                for (node_id, new_input_ids) in &new_node_inputs {
                    if !current_db_inputs.contains_key(node_id) {
                        created_input_ids_by_node_id.insert(*node_id, new_input_ids.clone());
                    }
                }
            }
            (None, Some(new_node_inputs)) => {
                // All new_input_ids inputs are considered created.
                for (node_id, new_input_ids) in &new_node_inputs {
                    created_input_ids_by_node_id.insert(*node_id, new_input_ids.clone());
                }
            }
            (Some(current_db_inputs), None) => {
                // All current_db inputs are considered removed.
                for (node_id, current_db_input_ids) in &current_db_inputs {
                    removed_input_ids_by_node_id.insert(*node_id, current_db_input_ids.clone());
                }
            }
            (None, None) => {
                // No inputs to update.
            }
        }

        let mut created_node_inputs_by_fow_step = HashMap::new();
        created_node_inputs_by_fow_step.insert(self.id, created_input_ids_by_node_id);

        Branch::update(
            data.db_session(),
            self.branch_id,
            BranchUpdate::CreateFlowStepInputs(created_node_inputs_by_fow_step),
        )
        .await?;

        Branch::update(
            data.db_session(),
            self.branch_id,
            BranchUpdate::DeleteFlowStepInputs((self.id, removed_input_ids_by_node_id)),
        )
        .await?;

        Ok(())
    }
}
