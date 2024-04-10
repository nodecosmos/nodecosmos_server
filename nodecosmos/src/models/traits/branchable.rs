use crate::models::node::reorder::data::ReorderData;
use charybdis::types::Uuid;

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

    fn branchise_id(&self, id: Uuid) -> Uuid {
        if self.is_original() {
            id
        } else {
            self.branch_id()
        }
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
