use serde::Serialize;

use crate::CharybdisError;

pub trait ToJson<T: Serialize> {
    fn from_json(json: &str) -> Result<String, CharybdisError>;
}

impl<T: Serialize> ToJson<T> for T {
    fn from_json(json: &str) -> Result<String, CharybdisError> {
        serde_json::to_string(json).map_err(|e| CharybdisError::JsonError(e))
    }
}
