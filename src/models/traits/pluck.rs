use crate::models::node::{Node, PkNode};
use crate::models::node_commit::NodeCommit;
use crate::models::node_descendant::NodeDescendant;
use charybdis::types::Uuid;
use std::collections::HashSet;

pub trait Pluck {
    fn pluck_id(&self) -> Vec<Uuid>;
    fn pluck_id_set(&self) -> HashSet<Uuid>;
}

macro_rules! impl_pluck_for {
    ($type:ty) => {
        impl Pluck for Vec<$type> {
            fn pluck_id(&self) -> Vec<Uuid> {
                self.iter().map(|item| item.id).collect()
            }

            fn pluck_id_set(&self) -> HashSet<Uuid> {
                self.iter().map(|item| item.id).collect()
            }
        }
    };
}

impl_pluck_for!(NodeDescendant);
impl_pluck_for!(PkNode);
impl_pluck_for!(Node);

pub trait VersionedNodePluck {
    fn pluck_node_descendants_commit_id(&self) -> Vec<Uuid>;
}

impl VersionedNodePluck for Vec<NodeCommit> {
    fn pluck_node_descendants_commit_id(&self) -> Vec<Uuid> {
        self.iter().filter_map(|item| item.node_descendants_commit_id).collect()
    }
}
