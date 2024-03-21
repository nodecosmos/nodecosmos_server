use std::collections::{HashMap, HashSet};

/// Trait to convert a HashMap with Vec values to a HashMap with HashSet values
pub trait HashMapVecValToSet<T> {
    fn hash_map_vec_val_to_set(self) -> HashMap<T, HashSet<T>>;
}

impl<T: std::hash::Hash + Eq> HashMapVecValToSet<T> for HashMap<T, Vec<T>> {
    fn hash_map_vec_val_to_set(self) -> HashMap<T, HashSet<T>> {
        self.into_iter().map(|(k, v)| (k, v.into_iter().collect())).collect()
    }
}
