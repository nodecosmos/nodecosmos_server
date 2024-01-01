use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::Branch;
use crate::models::node::find_pk_node;
use crate::models::node::PkNode;
use crate::models::udts::{Conflict, ConflictStatus, ConflictType};
use crate::utils::cloned_ref::ClonedRef;
use charybdis::operations::Update;
use charybdis::types::Uuid;
use std::collections::HashSet;

impl Branch {
    pub async fn validate_no_existing_conflicts(&mut self) -> Result<(), NodecosmosError> {
        if let Some(conflicts) = &self.conflicts {
            if conflicts
                .iter()
                .any(|c| c.status == ConflictStatus::Pending.to_string())
            {
                return Err(NodecosmosError::Conflict("Conflicts not resolved".to_string()));
            }
        }

        Ok(())
    }

    pub async fn check_conflicts(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        self.check_ancestors_deletion(data).await?;
        // self.check_prev_flow_step_deletion(data).await?;
        // self.check_next_flow_step_deletion(data).await?;
        // self.check_prev_step_output_deletion(data).await?;
        // self.check_flow_step_node_deletion(data).await?;
        // self.check_no_previous_flow(data).await?;
        // self.check_no_initial_input(data).await?;

        Ok(())
    }

    /// Check if any of original ancestors of created nodes were deleted
    pub async fn check_ancestors_deletion(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let created_node_ids = self.created_nodes.cloned_ref();
        let restored_node_ids = self.restored_nodes.cloned_ref();
        let created_nodes = self.created_nodes(data.db_session()).await?;
        let mut has_conflicts = false;

        if let Some(created_nodes) = created_nodes {
            for created_node in created_nodes {
                let ancestor_ids = created_node.ancestor_ids.cloned_ref();
                let original_ancestors_ids = ancestor_ids
                    .iter()
                    .filter_map(|id| {
                        if created_node_ids.contains(id) || restored_node_ids.contains(id) {
                            None
                        } else {
                            Some(*id)
                        }
                    })
                    .collect::<Vec<Uuid>>();

                let db_session = data.db_session();

                let ancestors = find_pk_node!(
                    db_session,
                    "id IN ? AND branch_id IN ?",
                    (original_ancestors_ids.clone(), original_ancestors_ids.clone())
                )
                .await?
                .try_collect()
                .await?;

                if ancestors.len() != original_ancestors_ids.len() {
                    has_conflicts = true;

                    let conflicts = self.conflicts.get_or_insert_with(|| HashSet::new());
                    conflicts.insert(Conflict {
                        object_id: created_node.id,
                        c_type: ConflictType::AncestorsDeleted.to_string(),
                        status: ConflictStatus::Pending.to_string(),
                    });
                }
            }
        }

        if has_conflicts {
            self.update(data.db_session()).await?;

            return Err(NodecosmosError::Conflict("Conflicts not resolved".to_string()));
        }

        Ok(())
    }
}
