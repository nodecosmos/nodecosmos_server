use crate::errors::NodecosmosError;
use crate::models::node::{BaseNode, GetStructureNode, Node};
use crate::models::node_descendant::NodeDescendant;
use charybdis::options::Consistency;
use charybdis::stream::CharybdisModelStream;
use scylla::CachingSession;
use std::collections::HashSet;

pub trait Descendants {
    async fn descendants(
        &self,
        session: &CachingSession,
        consistency: Option<Consistency>,
    ) -> Result<CharybdisModelStream<NodeDescendant>, NodecosmosError>;

    async fn branch_descendants(
        &self,
        session: &CachingSession,
        consistency: Option<Consistency>,
    ) -> Result<Vec<NodeDescendant>, NodecosmosError>;
}

macro_rules! impl_descendants {
    ($ty:ident) => {
        impl Descendants for $ty {
            async fn descendants(
                &self,
                session: &CachingSession,
                consistency: Option<Consistency>,
            ) -> Result<CharybdisModelStream<NodeDescendant>, NodecosmosError> {
                let mut q =
                    NodeDescendant::find_by_root_id_and_branch_id_and_node_id(self.root_id, self.branch_id, self.id);

                if let Some(consistency) = consistency {
                    q = q.consistency(consistency)
                }

                let descendants = q.execute(session).await?;

                Ok(descendants)
            }

            async fn branch_descendants(
                &self,
                session: &CachingSession,
                consistency: Option<Consistency>,
            ) -> Result<Vec<NodeDescendant>, NodecosmosError> {
                let mut original_q =
                    NodeDescendant::find_by_root_id_and_branch_id_and_node_id(self.root_id, self.id, self.id);
                let mut branched_q =
                    NodeDescendant::find_by_root_id_and_branch_id_and_node_id(self.root_id, self.branch_id, self.id);

                if let Some(consistency) = consistency {
                    original_q = original_q.consistency(consistency);
                    branched_q = branched_q.consistency(consistency);
                }

                let original = original_q.execute(session).await?.try_collect().await?;
                let branched = branched_q.execute(session).await?.try_collect().await?;

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
