use crate::errors::NodecosmosError;
use crate::models::branch::branchable::Branchable;
use crate::models::branch::AuthBranch;
use crate::models::node::{Node, UpdateDescriptionNode, UpdateTitleNode};
use charybdis::types::Uuid;
use scylla::CachingSession;

impl Branchable for Node {
    fn id(&self) -> Uuid {
        self.id
    }

    fn branch_id(&self) -> Uuid {
        self.branch_id
    }
}

impl Branchable for UpdateTitleNode {
    fn id(&self) -> Uuid {
        self.id
    }

    fn branch_id(&self) -> Uuid {
        self.branch_id
    }
}

impl Branchable for UpdateDescriptionNode {
    fn id(&self) -> Uuid {
        self.id
    }

    fn branch_id(&self) -> Uuid {
        self.branch_id
    }
}

impl Node {
    pub async fn init_auth_branch(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        let branch = AuthBranch::find_by_id(db_session, self.branch_id).await?;
        self.auth_branch = Some(branch);

        Ok(())
    }

    pub async fn auth_branch(
        &mut self,
        db_session: &CachingSession,
    ) -> Result<Option<&mut AuthBranch>, NodecosmosError> {
        if self.auth_branch.is_none() {
            self.init_auth_branch(db_session).await?;
        }

        Ok(self.auth_branch.as_mut())
    }
}
