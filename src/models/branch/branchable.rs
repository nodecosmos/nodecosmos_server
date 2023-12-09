use crate::errors::NodecosmosError;
use crate::models::branch::Branch;
use charybdis::operations::Find;
use charybdis::types::Uuid;
use scylla::CachingSession;

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

    async fn branch(&self, db_session: &CachingSession) -> Result<Branch, NodecosmosError> {
        let branch = Branch {
            id: self.branch_id(),
            ..Default::default()
        }
        .find_by_primary_key(db_session)
        .await?;

        Ok(branch)
    }
}
