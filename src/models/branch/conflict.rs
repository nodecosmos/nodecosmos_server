use crate::errors::NodecosmosError;
use crate::models::branch::Branch;
use crate::models::node::PkNode;
use crate::models::traits::Pluck;
use crate::models::udts::{Conflict, ConflictStatus};
use crate::utils::cloned_ref::ClonedRef;
use charybdis::operations::Update;
use charybdis::types::{Set, Uuid};
use scylla::CachingSession;
use std::collections::HashSet;

impl Branch {
    pub async fn validate_no_existing_conflicts(&mut self) -> Result<(), NodecosmosError> {
        if let Some(conflicts) = &self.conflict {
            if conflicts.status == ConflictStatus::Pending.to_string() {
                return Err(NodecosmosError::Conflict("Conflicts not resolved".to_string()));
            }
        }

        Ok(())
    }

    pub async fn check_conflicts(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        let deleted_ancestors = self.extract_deleted_ancestors(db_session).await?;
        let deleted_edited_nodes = self.extract_deleted_edited_nodes(db_session).await?;
        let mut has_conflicts = deleted_ancestors.is_some();
        has_conflicts |= deleted_edited_nodes.is_some();
        // has_conflicts |= deleted_nodes(db_session).await?;
        // has_conflicts |= deleted_workflows(db_session).await?;
        // has_conflicts |= deleted_flows(db_session).await?;
        // has_conflicts |= deleted_flow_steps(db_session).await?;
        // has_conflicts |= deleted_ios(db_session).await?;

        if has_conflicts {
            self.conflict = Some(Conflict {
                status: ConflictStatus::Pending.to_string(),
                deleted_ancestors,
                deleted_edited_nodes,
            });

            self.update().execute(db_session).await?;

            return Err(NodecosmosError::Conflict("Conflicts not resolved".to_string()));
        } else if let Some(conflict) = &mut self.conflict {
            conflict.deleted_ancestors = None;
            conflict.deleted_edited_nodes = None;
            conflict.status = ConflictStatus::Resolved.to_string();

            self.update().execute(db_session).await?;
        }

        Ok(())
    }

    /// Checks if any of original ancestors of created nodes were deleted
    pub async fn extract_deleted_ancestors(
        &mut self,
        db_session: &CachingSession,
    ) -> Result<Option<Set<Uuid>>, NodecosmosError> {
        let created_node_ids = self.created_nodes.cloned_ref();
        let restored_node_ids = self.restored_nodes.cloned_ref();
        let created_nodes = self.created_nodes(db_session).await?;
        let mut deleted_ancestor_ids = HashSet::new();

        if let Some(created_nodes) = created_nodes {
            for created_node in created_nodes {
                let ancestor_ids = created_node.ancestor_ids.cloned_ref();
                let branch_ancestor_ids = ancestor_ids
                    .iter()
                    .filter_map(|id| {
                        if created_node_ids.contains(id) || restored_node_ids.contains(id) {
                            None
                        } else {
                            Some(*id)
                        }
                    })
                    .collect::<Vec<Uuid>>();

                let original_ancestor_ids_set = PkNode::find_and_collect_by_ids(&db_session, &branch_ancestor_ids)
                    .await?
                    .pluck_id_set();

                let branch_node_deleted_ancestor_ids = branch_ancestor_ids
                    .iter()
                    .filter_map(|id| {
                        if original_ancestor_ids_set.contains(id) {
                            None
                        } else {
                            Some(*id)
                        }
                    })
                    .collect::<Set<Uuid>>();

                deleted_ancestor_ids.extend(branch_node_deleted_ancestor_ids);
            }
        }

        if deleted_ancestor_ids.is_empty() {
            Ok(None)
        } else {
            Ok(Some(deleted_ancestor_ids))
        }
    }

    pub async fn extract_deleted_edited_nodes(
        &mut self,
        db_session: &CachingSession,
    ) -> Result<Option<Set<Uuid>>, NodecosmosError> {
        let created_node_ids = self.created_nodes.cloned_ref();
        let restored_node_ids = self.restored_nodes.cloned_ref();
        let deleted_node_ids = self.deleted_nodes.cloned_ref();
        let mut deleted_edited_nodes = HashSet::new();

        if let Some(edit_description_node_ids) = &self.edited_node_descriptions {
            let edit_description_node_ids = edit_description_node_ids
                .iter()
                .filter_map(|id| {
                    if created_node_ids.contains(id) || restored_node_ids.contains(id) || deleted_node_ids.contains(id)
                    {
                        None
                    } else {
                        Some(*id)
                    }
                })
                .collect::<Vec<Uuid>>();

            let original_nodes = PkNode::find_and_collect_by_ids(&db_session, &edit_description_node_ids)
                .await?
                .pluck_id_set();

            let deleted_original_nodes = edit_description_node_ids
                .iter()
                .filter_map(|id| if original_nodes.contains(id) { None } else { Some(*id) })
                .collect::<Set<Uuid>>();

            deleted_edited_nodes.extend(deleted_original_nodes);
        }

        if deleted_edited_nodes.is_empty() {
            Ok(None)
        } else {
            Ok(Some(deleted_edited_nodes))
        }
    }
}
