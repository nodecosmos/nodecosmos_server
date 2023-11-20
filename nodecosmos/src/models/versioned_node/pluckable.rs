use crate::models::versioned_node::VersionedNode;
use charybdis::types::Uuid;

pub trait VersionedNodePluckable {
    fn pluck_versioned_descendants_id(&self) -> Vec<Uuid>;
}

impl VersionedNodePluckable for Vec<VersionedNode> {
    fn pluck_versioned_descendants_id(&self) -> Vec<Uuid> {
        self.iter().filter_map(|item| item.versioned_descendants_id).collect()
    }
}
