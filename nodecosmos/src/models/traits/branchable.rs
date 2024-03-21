use crate::api::WorkflowParams;
use crate::errors::NodecosmosError;
use crate::models::branch::Branch;
use crate::models::input_output::{Io, UpdateTitleIo};
use crate::models::like::Like;
use crate::models::node::reorder::data::ReorderData;
use charybdis::types::Uuid;
use scylla::CachingSession;

/// Branching records follows simple rule:
/// - Original model has the same id and branch_id
/// - Branched model has different id and branch_id
///
/// `nodecosmos-macros` provides a derive macro for this trait.
pub trait Branchable {
    fn original_id(&self) -> Uuid;

    fn branch_id(&self) -> Uuid;

    fn is_original(&self) -> bool {
        self.branch_id() == self.original_id()
    }

    fn is_branched(&self) -> bool {
        self.branch_id() != self.original_id()
    }

    fn branchise_id(&self, id: Uuid) -> Uuid {
        if self.is_original() {
            id
        } else {
            self.branch_id()
        }
    }

    async fn branch(&self, db_session: &CachingSession) -> Result<Branch, NodecosmosError> {
        let branch = Branch::find_by_id(self.branch_id()).execute(db_session).await?;

        Ok(branch)
    }
}

impl Branchable for Like {
    fn original_id(&self) -> Uuid {
        self.object_id
    }

    fn branch_id(&self) -> Uuid {
        self.branch_id
    }
}

impl Branchable for ReorderData {
    fn original_id(&self) -> Uuid {
        self.node.id
    }

    fn branch_id(&self) -> Uuid {
        self.branch_id
    }
}

impl Branchable for WorkflowParams {
    fn original_id(&self) -> Uuid {
        self.node_id
    }

    fn branch_id(&self) -> Uuid {
        self.branch_id
    }
}

impl Branchable for Io {
    fn original_id(&self) -> Uuid {
        self.root_node_id
    }

    fn branch_id(&self) -> Uuid {
        self.branch_id
    }
}

impl Branchable for UpdateTitleIo {
    fn original_id(&self) -> Uuid {
        self.root_node_id
    }

    fn branch_id(&self) -> Uuid {
        self.branch_id
    }
}
