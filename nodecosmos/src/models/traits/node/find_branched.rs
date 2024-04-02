use crate::errors::NodecosmosError;
use crate::models::node::{GetStructureNode, Node, UpdateTitleNode};
use charybdis::model::Model;
use charybdis::operations::Find;
use charybdis::options::Consistency;
use charybdis::types::Uuid;
use scylla::CachingSession;

pub trait FindBranched: Model {
    async fn find_branched_or_original(
        db_session: &CachingSession,
        id: Uuid,
        branch_id: Uuid,
        consistency: Option<Consistency>,
    ) -> Result<Self, NodecosmosError>;

    async fn find_branched(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError>;
}

macro_rules! impl_find_branched_or_original {
    ($struct_name:ident) => {
        impl FindBranched for $struct_name {
            async fn find_branched_or_original(
                db_session: &CachingSession,
                id: Uuid,
                branch_id: Uuid,
                consistency: Option<Consistency>,
            ) -> Result<Self, NodecosmosError> {
                let pk = &(id, branch_id);
                let mut node_q = Self::maybe_find_by_primary_key_value(pk);

                if let Some(consistency) = consistency {
                    node_q = node_q.consistency(consistency);
                }

                let node = node_q.execute(db_session).await?;

                return match node {
                    Some(node) => Ok(node),
                    None => {
                        let mut node = Self::find_by_primary_key_value(&(id, id))
                            .execute(db_session)
                            .await?;
                        node.branch_id = branch_id;

                        Ok(node)
                    }
                };
            }

            async fn find_branched(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
                let branch_self = Self::maybe_find_by_primary_key_value(&(self.id, self.branch_id))
                    .execute(db_session)
                    .await?;

                match branch_self {
                    Some(mut branch_self) => {
                        branch_self.parent = self.parent.take();
                        branch_self.auth_branch = self.auth_branch.take();
                        branch_self.ctx = self.ctx;

                        *self = branch_self;
                    }
                    None => {
                        let parent = self.parent.take();
                        let auth_branch = self.auth_branch.take();
                        let branch_id = self.branch_id;
                        let ctx = self.ctx.clone();

                        *self = Self::find_by_primary_key_value(&(self.id, self.id))
                            .execute(db_session)
                            .await?;

                        self.branch_id = branch_id;
                        self.parent = parent;
                        self.auth_branch = auth_branch;
                        self.ctx = ctx;
                    }
                }

                Ok(())
            }
        }
    };
}

impl_find_branched_or_original!(Node);
impl_find_branched_or_original!(GetStructureNode);
impl_find_branched_or_original!(UpdateTitleNode);
