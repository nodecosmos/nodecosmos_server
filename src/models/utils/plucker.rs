use crate::models::node::PkNode;
use crate::models::node_descendant::NodeDescendant;
use charybdis::types::Uuid;
use std::collections::HashSet;

pub trait Pluckable {
    fn pluck_id(&self) -> Vec<Uuid>;
    fn pluck_id_set(&self) -> HashSet<Uuid>;
}

macro_rules! impl_pluckable_for {
    ($type:ty) => {
        impl Pluckable for Vec<$type> {
            fn pluck_id(&self) -> Vec<Uuid> {
                self.iter().map(|item| item.id).collect()
            }

            fn pluck_id_set(&self) -> HashSet<Uuid> {
                self.iter().map(|item| item.id).collect()
            }
        }
    };
}

impl_pluckable_for!(NodeDescendant);
impl_pluckable_for!(PkNode);
