use actix_web::{HttpResponse, ResponseError};
use serde_json::json;
use std::error::Error;
use std::fmt;

#[derive(Debug, Clone)]
pub enum NodecosmosError {
    ClientSessionError(String),
    Unauthorized(serde_json::Value),
}

impl fmt::Display for NodecosmosError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NodecosmosError::ClientSessionError(e) => write!(f, "Session Error: {}", e),
            NodecosmosError::Unauthorized(e) => write!(f, "Unauthorized: {}", e),
        }
    }
}

impl Error for NodecosmosError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            NodecosmosError::ClientSessionError(_) => None,
            NodecosmosError::Unauthorized(_) => None,
        }
    }
}

impl ResponseError for NodecosmosError {
    fn error_response(&self) -> HttpResponse {
        match self {
            NodecosmosError::Unauthorized(e) => HttpResponse::Unauthorized().json(e),
            _ => HttpResponse::InternalServerError().json(json!({
                "error": "Internal Server Error",
                "message": "Something went wrong"
            })),
        }
    }
}
