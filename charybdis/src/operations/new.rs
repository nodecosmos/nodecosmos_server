use crate::model::Model;

pub trait New <T: Default + Model> {
    fn new() -> T;
}

impl <T: Default + Model> New<T> for T {
    fn new() -> T {
        T::default()
    }
}
