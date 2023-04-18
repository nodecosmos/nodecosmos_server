use crate::model::BaseModel;

pub trait New<T: Default + BaseModel> {
    fn new() -> T;
}

impl<T: Default + BaseModel> New<T> for T {
    fn new() -> T {
        T::default()
    }
}
