use crate::models::node::{Node, PkNode, UpdateDescriptionNode, UpdateTitleNode};
use crate::models::node_descendant::NodeDescendant;
use charybdis::types::Uuid;

pub trait Id {
    fn id(&self) -> Uuid;
}

macro_rules! impl_id {
    ($t:ty) => {
        impl Id for $t {
            fn id(&self) -> Uuid {
                self.id
            }
        }
    };
}

impl_id!(Node);
impl_id!(UpdateDescriptionNode);
impl_id!(UpdateTitleNode);
impl_id!(PkNode);
impl_id!(NodeDescendant);
