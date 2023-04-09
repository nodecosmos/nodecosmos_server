use serde::Serialize;

pub trait ToJSON {
    fn to_json(&self) -> String;
}

impl<T: Serialize> ToJSON for T {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap()
    }
}
