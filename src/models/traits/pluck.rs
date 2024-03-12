use crate::models::node_commit::NodeCommit;
use crate::models::traits::Id;
use charybdis::model::Model;
use charybdis::types::Uuid;
use std::collections::HashSet;

pub trait Pluck {
    fn pluck_id(&self) -> Vec<Uuid>;
    fn pluck_id_set(&self) -> HashSet<Uuid>;
}

impl<T: Model + Id> Pluck for Vec<T> {
    fn pluck_id(&self) -> Vec<Uuid> {
        self.iter().map(|item| item.id()).collect()
    }

    fn pluck_id_set(&self) -> HashSet<Uuid> {
        self.iter().map(|item| item.id()).collect()
    }
}

pub trait VersionedNodePluck {
    fn pluck_node_descendants_commit_id(&self) -> Vec<Uuid>;
}

impl VersionedNodePluck for Vec<NodeCommit> {
    fn pluck_node_descendants_commit_id(&self) -> Vec<Uuid> {
        self.iter().filter_map(|item| item.node_descendants_commit_id).collect()
    }
}
