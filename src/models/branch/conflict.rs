use crate::errors::NodecosmosError;
use crate::models::branch::Branch;
use crate::models::node::{Node, PkNode};
use crate::models::traits::Pluck;
use crate::models::udts::{Conflict, ConflictStatus};
use crate::utils::cloned_ref::ClonedRef;
use charybdis::operations::Update;
use charybdis::types::{Set, Uuid};
use scylla::CachingSession;
use std::collections::HashSet;

enum DelAncNode<'a> {
    Node(&'a Node),
    PkNode(&'a PkNode),
}

struct BranchConflict<'a> {
    branch: &'a mut Branch,
    status: ConflictStatus,
    deleted_ancestors: Option<Set<Uuid>>,
    deleted_edited_nodes: Option<Set<Uuid>>,
}

impl<'a> BranchConflict<'a> {
    pub fn new(branch: &'a mut Branch) -> Self {
        BranchConflict {
            branch,
            status: ConflictStatus::Resolved,
            deleted_ancestors: None,
            deleted_edited_nodes: None,
        }
    }

    pub async fn run(&mut self, db_session: &CachingSession) -> Result<&Self, NodecosmosError> {
        self.extract_created_nodes_conflicts(db_session).await?;
        self.extract_edited_description_nodes_conflicts(db_session).await?;

        if self.deleted_ancestors.is_some() || self.deleted_edited_nodes.is_some() {
            self.status = ConflictStatus::Pending;
        } else {
            self.status = ConflictStatus::Resolved;
        }

        Ok(self)
    }

    async fn extract_deleted_ancestors(
        &mut self,
        db_session: &CachingSession,
        del_anc_node: DelAncNode<'_>,
    ) -> Result<(), NodecosmosError> {
        let created_node_ids = self.branch.created_nodes.cloned_ref();
        let restored_node_ids = self.branch.restored_nodes.cloned_ref();
        let deleted_node_ids = self.branch.deleted_nodes.cloned_ref();
        let ancestor_id = match del_anc_node {
            DelAncNode::Node(node) => node.ancestor_ids.cloned_ref(),
            DelAncNode::PkNode(pk_node) => pk_node.ancestor_ids.cloned_ref(),
        };

        let branch_ancestor_ids = ancestor_id
            .iter()
            .filter_map(|id| {
                if created_node_ids.contains(id) || restored_node_ids.contains(id) || deleted_node_ids.contains(id) {
                    None
                } else {
                    Some(*id)
                }
            })
            .collect::<Vec<Uuid>>();

        let original_ancestor_ids_set = PkNode::find_by_ids(&db_session, &branch_ancestor_ids)
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

        if branch_node_deleted_ancestor_ids.is_empty() {
            return Ok(());
        }

        self.deleted_ancestors = match self.deleted_ancestors.as_mut() {
            Some(deleted_ancestors) => {
                let mut deleted_ancestors = deleted_ancestors.clone();
                deleted_ancestors.extend(branch_node_deleted_ancestor_ids);
                Some(deleted_ancestors)
            }
            None => Some(branch_node_deleted_ancestor_ids),
        };

        Ok(())
    }

    async fn extract_created_nodes_conflicts(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        let created_nodes = self.branch.created_nodes(db_session).await?;

        if let Some(created_nodes) = created_nodes {
            for created_node in created_nodes {
                self.extract_deleted_ancestors(db_session, DelAncNode::Node(&created_node))
                    .await?;
            }
        }

        Ok(())
    }

    async fn extract_edited_description_nodes_conflicts(
        &mut self,
        db_session: &CachingSession,
    ) -> Result<(), NodecosmosError> {
        let created_node_ids = self.branch.created_nodes.cloned_ref();
        let restored_node_ids = self.branch.restored_nodes.cloned_ref();
        let deleted_node_ids = self.branch.deleted_nodes.cloned_ref();
        let mut deleted_edited_nodes = HashSet::new();

        if let Some(edit_description_node_ids) = &self.branch.edited_node_descriptions {
            let edit_description_node_ids = edit_description_node_ids
                .iter()
                .filter_map(|id| {
                    if created_node_ids.contains(id) || deleted_node_ids.contains(id) {
                        None
                    } else {
                        Some(*id)
                    }
                })
                .collect::<Vec<Uuid>>();

            // check ancestors of edited description nodes
            let desc_branched_nodes =
                PkNode::find_by_ids_and_branch_id(&db_session, &edit_description_node_ids, self.branch.id).await?;

            for node in &desc_branched_nodes {
                self.extract_deleted_ancestors(db_session, DelAncNode::PkNode(&node))
                    .await?;
            }

            let original_nodes_ids = PkNode::find_by_ids(&db_session, &edit_description_node_ids)
                .await?
                .pluck_id_set();

            let deleted_edited_desc_nodes = edit_description_node_ids
                .iter()
                .filter_map(|id| {
                    if original_nodes_ids.contains(id) || restored_node_ids.contains(id) {
                        None
                    } else {
                        Some(*id)
                    }
                })
                .collect::<Set<Uuid>>();

            deleted_edited_nodes.extend(deleted_edited_desc_nodes);
        }

        if deleted_edited_nodes.len() > 0 {
            self.deleted_edited_nodes = Some(deleted_edited_nodes);
        }

        Ok(())
    }
}

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
        let mut branch_conflict = BranchConflict::new(self);
        branch_conflict.run(db_session).await?;

        if branch_conflict.status == ConflictStatus::Pending {
            self.conflict = Some(Conflict {
                status: branch_conflict.status.to_string(),
                deleted_ancestors: branch_conflict.deleted_ancestors,
                deleted_edited_nodes: branch_conflict.deleted_edited_nodes,
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
}
