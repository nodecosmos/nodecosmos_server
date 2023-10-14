use crate::CharybdisError;
use serde::Deserialize;

pub trait FromJson<'a, T: Deserialize<'a>> {
    fn from_json(json: &'a str) -> Result<T, CharybdisError>;
}

impl<'a, T: Deserialize<'a>> FromJson<'a, T> for T {
    fn from_json(json: &'a str) -> Result<T, CharybdisError> {
        serde_json::from_str(json).map_err(|e| CharybdisError::JsonError(e))
    }
}
