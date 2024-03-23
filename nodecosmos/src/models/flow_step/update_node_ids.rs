use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::update::BranchUpdate;
use crate::models::branch::Branch;
use crate::models::flow_step::UpdateNodeIdsFlowStep;
use crate::models::traits::set::ToHashSet;
use charybdis::model::AsNative;
use std::collections::{HashMap, HashSet};

impl UpdateNodeIdsFlowStep {
    pub async fn update_branch(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        let maybe_original = self.as_native().maybe_find_original(data.db_session()).await?;

        if let Some(original) = maybe_original {
            let original_node_ids = original.node_ids.clone();
            let current_node_ids = self.node_ids.clone();

            let mut created_node_ids = None;
            let mut deleted_node_ids = None;

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
                        created_node_ids = Some(created);
                    }
                    if !deleted.is_empty() {
                        deleted_node_ids = Some(deleted);
                    }
                }
                (Some(original_node_ids), None) => {
                    deleted_node_ids = Some(original_node_ids.to_hash_set());
                }
                (None, Some(current_node_ids)) => {
                    created_node_ids = Some(current_node_ids.to_hash_set());
                }
                (None, None) => {}
            }

            let mut created_flow_step_nodes = None;
            let mut deleted_flow_step_nodes = None;

            if let Some(created_node_ids) = created_node_ids {
                let mut value = HashMap::new();
                value.insert(self.id, created_node_ids);

                created_flow_step_nodes = Some(value);
            }

            if let Some(deleted_node_ids) = deleted_node_ids {
                let mut value = HashMap::new();
                value.insert(self.id, deleted_node_ids);

                deleted_flow_step_nodes = Some(value);
            }

            Branch::update(
                data,
                self.branch_id,
                BranchUpdate::CreatedFlowStepNodes(created_flow_step_nodes),
            )
            .await?;

            Branch::update(
                data,
                self.branch_id,
                BranchUpdate::DeletedFlowStepNodes(deleted_flow_step_nodes),
            )
            .await?;
        }

        Ok(())
    }
}
