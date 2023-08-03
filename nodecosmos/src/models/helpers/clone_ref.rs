pub trait ClonedRef<T> {
    fn cloned_ref(&self) -> Vec<T>;
}
impl<T: Clone> ClonedRef<T> for Option<Vec<T>> {
    fn cloned_ref(&self) -> Vec<T> {
        self.as_ref().cloned().unwrap_or_default()
    }
}
