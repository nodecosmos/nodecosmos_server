use std::collections::HashSet;

use charybdis::stream::CharybdisModelStream;
use scylla::client::caching_session::CachingSession;

use crate::errors::NodecosmosError;
use crate::models::node::{BaseNode, GetStructureNode, Node};
use crate::models::node_descendant::NodeDescendant;

pub trait Descendants {
    async fn descendants(
        &self,
        db_session: &CachingSession,
    ) -> Result<CharybdisModelStream<NodeDescendant>, NodecosmosError>;

    async fn branch_descendants(&self, db_session: &CachingSession) -> Result<Vec<NodeDescendant>, NodecosmosError>;
}

macro_rules! impl_descendants {
    ($ty:ident) => {
        impl Descendants for $ty {
            async fn descendants(
                &self,
                db_session: &CachingSession,
            ) -> Result<CharybdisModelStream<NodeDescendant>, NodecosmosError> {
                let descendants =
                    NodeDescendant::find_by_root_id_and_branch_id_and_node_id(self.root_id, self.branch_id, self.id)
                        .execute(db_session)
                        .await?;

                Ok(descendants)
            }

            /// combine descendants from original and branched nodes
            async fn branch_descendants(
                &self,
                db_session: &CachingSession,
            ) -> Result<Vec<NodeDescendant>, NodecosmosError> {
                use crate::models::traits::Branchable;

                let original = NodeDescendant::find_by_root_id_and_branch_id_and_node_id(
                    self.root_id,
                    self.original_id(),
                    self.id,
                )
                .page_size(5)
                .execute(db_session)
                .await?
                .try_collect()
                .await?;

                let branched =
                    NodeDescendant::find_by_root_id_and_branch_id_and_node_id(self.root_id, self.branch_id, self.id)
                        .execute(db_session)
                        .await?
                        .try_collect()
                        .await?;

                let mut branched_ids = HashSet::with_capacity(branched.len());
                let mut descendants = Vec::with_capacity(original.len() + branched.len());

                for descendant in branched {
                    branched_ids.insert(descendant.id);
                    descendants.push(descendant);
                }

                for descendant in original {
                    if !branched_ids.contains(&descendant.id) {
                        descendants.push(descendant);
                    }
                }

                descendants.sort_by(|a, b| {
                    a.order_index
                        .partial_cmp(&b.order_index)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });

                Ok(descendants)
            }
        }
    };
}

impl_descendants!(Node);
impl_descendants!(BaseNode);
impl_descendants!(GetStructureNode);
