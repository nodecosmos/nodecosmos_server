use charybdis::types::Uuid;

pub trait Branchable {
    fn id(&self) -> Uuid;

    fn branch_id(&self) -> Uuid;

    fn is_main_branch(&self) -> bool {
        self.branch_id() == self.id()
    }

    fn is_different_branch(&self) -> bool {
        !self.is_main_branch()
    }

    fn branched_id(&self, id: Uuid) -> Uuid {
        if self.is_main_branch() {
            id
        } else {
            self.branch_id()
        }
    }
}
