use std::collections::{HashMap, HashSet};

use charybdis::model::AsNative;
use charybdis::operations::DeleteWithCallbacks;

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::update::BranchUpdate;
use crate::models::branch::Branch;
use crate::models::flow_step::UpdateNodeIdsFlowStep;
use crate::models::io::Io;
use crate::models::traits::{ModelContext, ToHashSet};

impl UpdateNodeIdsFlowStep {
    pub async fn delete_output_records_from_removed_nodes(
        &mut self,
        data: &RequestData,
    ) -> Result<(), NodecosmosError> {
        if let Some(output_ids_by_node_id) = self.output_ids_by_node_id.as_ref() {
            for (node_id, output_ids) in output_ids_by_node_id.iter() {
                if self.node_ids.is_none() || self.node_ids.as_ref().is_some_and(|ids| !ids.contains(node_id)) {
                    for output_id in output_ids {
                        let mut output = Io {
                            root_id: self.root_id,
                            branch_id: self.branch_id,
                            node_id: self.node_id,
                            id: *output_id,
                            flow_step_id: Some(self.id),

                            ..Default::default()
                        };

                        output.set_parent_delete_context();
                        output.delete_cb(data).execute(data.db_session()).await?;
                    }
                }
            }
        }

        Ok(())
    }

    pub async fn remove_output_references_from_removed_nodes(&mut self) -> Result<(), NodecosmosError> {
        if let Some(output_ids_by_node_id) = self.output_ids_by_node_id.as_mut() {
            output_ids_by_node_id.retain(|node_id, _| self.node_ids.as_ref().is_some_and(|ids| ids.contains(node_id)));
        }

        Ok(())
    }

    pub async fn remove_input_references_from_removed_nodes(&mut self) -> Result<(), NodecosmosError> {
        if let Some(input_ids_by_node_id) = self.input_ids_by_node_id.as_mut() {
            input_ids_by_node_id.retain(|node_id, _| self.node_ids.as_ref().is_some_and(|ids| ids.contains(node_id)));
        }

        Ok(())
    }

    pub async fn update_branch(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        let maybe_original = self.as_native().maybe_find_original(data.db_session()).await?;

        if let Some(original) = maybe_original {
            let original_node_ids = original.node_ids.clone();
            let current_node_ids = self.node_ids.clone();

            let mut created_node_ids = HashSet::new();
            let mut deleted_node_ids = HashSet::new();

            match (original_node_ids, current_node_ids) {
                (Some(original_node_ids), Some(current_node_ids)) => {
                    // Calculate created and deleted nodes
                    let created: HashSet<_> = current_node_ids
                        .iter()
                        .filter(|id| !original_node_ids.contains(id))
                        .cloned()
                        .collect();

                    let deleted: HashSet<_> = original_node_ids
                        .iter()
                        .filter(|id| !current_node_ids.contains(id))
                        .cloned()
                        .collect();

                    if !created.is_empty() {
                        created_node_ids = created;
                    }
                    if !deleted.is_empty() {
                        deleted_node_ids = deleted;
                    }
                }
                (Some(original_node_ids), None) => {
                    deleted_node_ids = original_node_ids.to_hash_set();
                }
                (None, Some(current_node_ids)) => {
                    created_node_ids = current_node_ids.to_hash_set();
                }
                (None, None) => {}
            }

            let mut created_flow_step_nodes = HashMap::new();
            created_flow_step_nodes.insert(self.id, created_node_ids);

            let mut deleted_flow_step_nodes = HashMap::new();
            deleted_flow_step_nodes.insert(self.id, deleted_node_ids);

            Branch::update(
                data.db_session(),
                self.branch_id,
                BranchUpdate::CreatedFlowStepNodes(created_flow_step_nodes),
            )
            .await?;

            Branch::update(
                data.db_session(),
                self.branch_id,
                BranchUpdate::DeletedFlowStepNodes(deleted_flow_step_nodes),
            )
            .await?;
        }

        Ok(())
    }
}
