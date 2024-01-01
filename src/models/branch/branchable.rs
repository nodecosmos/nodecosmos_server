use crate::errors::NodecosmosError;
use crate::models::branch::Branch;
use charybdis::operations::Find;
use charybdis::types::Uuid;
use scylla::CachingSession;

/// Branching records follows simple rule:
/// - Original model has the same id and branch_id
/// - Branched model has different id and branch_id
pub trait Branchable {
    fn id(&self) -> Uuid;

    fn branch_id(&self) -> Uuid;

    fn is_original(&self) -> bool {
        self.branch_id() == self.id()
    }

    fn is_branched(&self) -> bool {
        !self.is_original()
    }

    fn branchise_id(&self, id: Uuid) -> Uuid {
        if self.is_original() {
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
