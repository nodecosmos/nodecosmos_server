use crate::errors::NodecosmosError;
use crate::models::branch::Branch;
use crate::models::node::{Node, PkNode};
use crate::models::traits::ref_cloned::RefCloned;
use crate::models::traits::Pluck;
use crate::models::udts::{Conflict, ConflictStatus};
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
