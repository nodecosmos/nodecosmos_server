use crate::models::branch::branchable::Branchable;
use crate::models::node::Node;
use charybdis::types::Uuid;

impl Branchable for Node {
    fn id(&self) -> Uuid {
        self.id
    }

    fn branch_id(&self) -> Uuid {
        self.branch_id
    }
}
