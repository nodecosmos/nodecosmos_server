use crate::models::node::{UpdateDescriptionNode, UpdateTitleNode};
use charybdis::model::Model;
use charybdis::types::Uuid;
use std::collections::HashMap;

pub trait NodeId {
    fn id(&self) -> Uuid;
}

macro_rules! impl_native_node_id {
    ($t:ty) => {
        impl NodeId for $t {
            fn id(&self) -> Uuid {
                self.id
            }
        }
    };
}

#[allow(unused_macros)]
macro_rules! impl_node_id {
    ($t:ty) => {
        impl NodeId for $t {
            fn id(&self) -> Uuid {
                self.node_id
            }
        }
    };
}

impl_native_node_id!(UpdateDescriptionNode);
impl_native_node_id!(UpdateTitleNode);

pub trait GroupById<T: Model> {
    fn group_by_id(&self) -> HashMap<Uuid, T>;
}

macro_rules! impl_group_by_id {
    ($t:ty) => {
        impl GroupById<$t> for Vec<$t> {
            fn group_by_id(&self) -> HashMap<Uuid, $t> {
                let mut res = HashMap::new();

                for item in self {
                    res.insert(item.id(), item.clone());
                }

                res
            }
        }
    };
}

impl_group_by_id!(UpdateDescriptionNode);
impl_group_by_id!(UpdateTitleNode);
