use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::update::BranchUpdate;
use crate::models::branch::Branch;
use crate::models::contribution_request::ContributionRequest;
use crate::models::like::Like;
use crate::models::node::reorder::reorder_data::ReorderData;
use crate::models::node::{Node, UpdateDescriptionNode, UpdateTitleNode};
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
        let branch = Branch::find_by_id(db_session, self.branch_id()).await?;

        Ok(branch)
    }
}

macro_rules! impl_branchable {
    ($struct_name:ident) => {
        impl Branchable for $struct_name {
            fn id(&self) -> Uuid {
                self.id
            }

            fn branch_id(&self) -> Uuid {
                self.branch_id
            }
        }
    };
}

impl_branchable!(Node);
impl_branchable!(UpdateTitleNode);
impl_branchable!(UpdateDescriptionNode);

impl Branchable for ContributionRequest {
    fn id(&self) -> Uuid {
        self.id
    }

    fn branch_id(&self) -> Uuid {
        self.id
    }
}

impl Branchable for Like {
    fn id(&self) -> Uuid {
        self.object_id
    }

    fn branch_id(&self) -> Uuid {
        self.branch_id
    }
}

impl Branchable for ReorderData {
    fn id(&self) -> Uuid {
        self.node.id
    }

    fn branch_id(&self) -> Uuid {
        self.branch_id
    }
}
