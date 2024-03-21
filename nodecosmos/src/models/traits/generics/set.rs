use std::collections::HashSet;

pub trait ToHashSet<T> {
    fn to_hash_set(self) -> HashSet<T>;
}

impl<T> ToHashSet<T> for Vec<T>
where
    T: std::hash::Hash + Eq,
{
    fn to_hash_set(self) -> HashSet<T> {
        self.into_iter().collect()
    }
}
