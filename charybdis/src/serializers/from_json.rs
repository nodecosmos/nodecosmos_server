use serde::Deserialize;

pub trait FromJson<'a> {
    fn from_json(json: &'a str) -> Self;
}

impl<'a, T: Deserialize<'a>> FromJson<'a> for T {
    fn from_json(json: &'a str) -> Self {
        serde_json::from_str(json).unwrap()
    }
}
