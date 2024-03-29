use std::collections::HashSet;

pub trait RefCloned<T> {
    type Iterable<I>;

    fn ref_cloned(&self) -> Self::Iterable<T>;
}

impl<T: Clone> RefCloned<T> for Option<Vec<T>> {
    type Iterable<I> = Vec<T>;

    fn ref_cloned(&self) -> Vec<T> {
        self.as_ref().cloned().unwrap_or_default()
    }
}

impl<T: Clone> RefCloned<T> for Option<HashSet<T>> {
    type Iterable<I> = HashSet<T>;

    fn ref_cloned(&self) -> HashSet<T> {
        self.as_ref().cloned().unwrap_or_default()
    }
}
