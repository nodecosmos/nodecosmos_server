use actix_web::{HttpResponse, ResponseError};
use charybdis::CharybdisError;
use serde_json::json;
use std::error::Error;
use std::fmt;

#[derive(Debug, Clone)]
pub enum NodecosmosError {
    ClientSessionError(String),
    Unauthorized(serde_json::Value),
    CharybdisError(CharybdisError),
    // InternalServerError(String),
}

impl fmt::Display for NodecosmosError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NodecosmosError::ClientSessionError(e) => write!(f, "Session Error: {}", e),
            NodecosmosError::Unauthorized(e) => write!(f, "Unauthorized: {}", e),
            NodecosmosError::CharybdisError(e) => write!(f, "Charybdis Error: \n{}", e),
            // NodecosmosError::InternalServerError(e) => write!(f, "InternalServerError: \n{}", e),
        }
    }
}

impl Error for NodecosmosError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            NodecosmosError::ClientSessionError(_) => None,
            NodecosmosError::Unauthorized(_) => None,
            NodecosmosError::CharybdisError(_) => None,
            // NodecosmosError::InternalServerError(_) => None,
        }
    }
}

impl ResponseError for NodecosmosError {
    fn error_response(&self) -> HttpResponse {
        match self {
            NodecosmosError::Unauthorized(e) => HttpResponse::Unauthorized().json(e),
            NodecosmosError::CharybdisError(e) => match e {
                CharybdisError::NotFoundError(e) => HttpResponse::NotFound().json(json!({
                    "error": "Not Found",
                    "message": e.to_string()
                })),
                CharybdisError::ValidationError((field, message)) => {
                    HttpResponse::Conflict().json(json!({ "error": {field: message} }))
                }
                _ => HttpResponse::InternalServerError().json(json!({
                    "error": "Internal Server Error",
                    "message": e.to_string()
                })),
            },
            _ => HttpResponse::InternalServerError().json(json!({
                "error": "Internal Server Error",
                "message": "Something went wrong"
            })),
        }
    }
}

// TODO: Map CharybdisError to HTTP errors accordingly
//  ATM, we just return InternalServerError.
//  We should make clear distinction between user errors (validation, etc.)
//  and internal errors.
impl From<CharybdisError> for NodecosmosError {
    fn from(e: CharybdisError) -> Self {
        NodecosmosError::CharybdisError(e)
    }
}
