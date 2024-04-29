use crate::models::branch::Branch;
use crate::models::contribution_request::ContributionRequest;
use charybdis::types::Uuid;
use nodecosmos_macros::Branchable;
use serde::Deserialize;

use crate::models::node::reorder::data::ReorderData;

/// Branching records follows simple rule:
/// - Original model has the same id and branch_id
/// - Branched model has different id and branch_id
///
/// `nodecosmos-macros` provides a derives for this trait.
/// `Branchable` is a derive for models that have `object_id` and `branch_id` fields.
/// `Branchable` is a derive for models that have `node_id` and `branch_id` fields.
pub trait Branchable {
    fn original_id(&self) -> Uuid;

    fn branch_id(&self) -> Uuid;

    fn is_original(&self) -> bool {
        self.branch_id() == self.original_id()
    }

    fn is_branched(&self) -> bool {
        self.branch_id() != self.original_id()
    }

    /// Sets the branch_id to the original_id
    fn set_original_id(&mut self);
}

impl Branchable for ReorderData {
    fn original_id(&self) -> Uuid {
        self.node.id
    }

    fn branch_id(&self) -> Uuid {
        self.branch_id
    }

    fn set_original_id(&mut self) {}
}

impl Branchable for Branch {
    fn original_id(&self) -> Uuid {
        self.root_id
    }

    fn branch_id(&self) -> Uuid {
        self.id
    }

    fn set_original_id(&mut self) {
        panic!("Branch model cannot be original")
    }
}

impl Branchable for ContributionRequest {
    fn original_id(&self) -> Uuid {
        self.root_id
    }

    fn branch_id(&self) -> Uuid {
        self.id
    }

    fn set_original_id(&mut self) {
        panic!("Branch model cannot be original")
    }
}

#[derive(Branchable, Deserialize)]
pub struct NodeBranchParams {
    #[branch(original_id)]
    pub original_id: Uuid,

    pub branch_id: Uuid,
    pub node_id: Uuid,
}

#[derive(Branchable)]
pub struct ModelBranchParams {
    #[branch(original_id)]
    pub original_id: Uuid,
    pub branch_id: Uuid,
    pub node_id: Uuid,
    pub id: Uuid,
}
