use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::update::BranchUpdate;
use crate::models::branch::Branch;
use crate::models::flow_step::UpdateNodeIdsFlowStep;
use crate::models::traits::set::ToHashSet;
use charybdis::model::AsNative;
use std::collections::HashSet;

impl UpdateNodeIdsFlowStep {
    pub async fn update_branch(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        let maybe_original = self.as_native().maybe_find_original(data.db_session()).await?;

        if let Some(original) = maybe_original {
            let original_node_ids = original.node_ids.clone();
            let current_node_ids = self.node_ids.clone();

            let mut created_node_ids = None;
            let mut removed_node_ids = None;

            match (original_node_ids, current_node_ids) {
                (Some(original_node_ids), Some(current_node_ids)) => {
                    // Calculate created and removed nodes
                    let created: HashSet<_> = current_node_ids
                        .iter()
                        .filter(|id| !original_node_ids.contains(id))
                        .cloned()
                        .collect();

                    let removed: HashSet<_> = original_node_ids
                        .iter()
                        .filter(|id| !current_node_ids.contains(id))
                        .cloned()
                        .collect();

                    if !created.is_empty() {
                        created_node_ids = Some(created);
                    }
                    if !removed.is_empty() {
                        removed_node_ids = Some(removed);
                    }
                }
                (Some(original_node_ids), None) => {
                    removed_node_ids = Some(original_node_ids.to_hash_set());
                }
                (None, Some(current_node_ids)) => {
                    created_node_ids = Some(current_node_ids.to_hash_set());
                }
                (None, None) => {}
            }

            Branch::update(
                data,
                self.branch_id,
                BranchUpdate::CreatedFlowStepNodes(created_node_ids),
            )
            .await?;

            Branch::update(
                data,
                self.branch_id,
                BranchUpdate::DeletedFlowStepNodes(removed_node_ids),
            )
            .await?;
        }

        Ok(())
    }
}
