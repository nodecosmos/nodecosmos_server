use std::collections::HashSet;

pub trait ClonedRef<T> {
    type Iterable<I>;

    fn cloned_ref(&self) -> Self::Iterable<T>;
}

impl<T: Clone> ClonedRef<T> for Option<Vec<T>> {
    type Iterable<I> = Vec<T>;

    fn cloned_ref(&self) -> Vec<T> {
        self.as_ref().cloned().unwrap_or_default()
    }
}

impl<T: Clone> ClonedRef<T> for Option<HashSet<T>> {
    type Iterable<I> = HashSet<T>;

    fn cloned_ref(&self) -> HashSet<T> {
        self.as_ref().cloned().unwrap_or_default()
    }
}
