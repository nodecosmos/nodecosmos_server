use crate::models::node_commit::NodeCommit;
use charybdis::types::Uuid;

pub trait VersionedNodePluckable {
    fn pluck_node_descendants_commit_id(&self) -> Vec<Uuid>;
}

impl VersionedNodePluckable for Vec<NodeCommit> {
    fn pluck_node_descendants_commit_id(&self) -> Vec<Uuid> {
        self.iter().filter_map(|item| item.node_descendants_commit_id).collect()
    }
}
