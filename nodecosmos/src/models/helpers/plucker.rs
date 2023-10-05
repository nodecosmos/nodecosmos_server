use crate::models::node_descendant::NodeDescendant;
use charybdis::Uuid;

pub trait Pluckable {
    fn pluck_id(&self) -> Vec<Uuid>;
}
macro_rules! impl_pluckable_for {
    ($type:ty) => {
        impl Pluckable for Vec<$type> {
            fn pluck_id(&self) -> Vec<Uuid> {
                self.iter().map(|item| item.id).collect()
            }
        }
    };
}

impl_pluckable_for!(NodeDescendant);
