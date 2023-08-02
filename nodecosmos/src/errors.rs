use actix_web::{HttpResponse, ResponseError};
use charybdis::CharybdisError;
use colored::Colorize;
use serde_json::json;
use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub enum NodecosmosError {
    ClientSessionError(String),
    Unauthorized(serde_json::Value),
    CharybdisError(CharybdisError),
    SerdeError(serde_json::Error),
    ElasticError(elasticsearch::Error),
    // InternalServerError(String),
}

impl fmt::Display for NodecosmosError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NodecosmosError::ClientSessionError(e) => write!(f, "Session Error: {}", e),
            NodecosmosError::Unauthorized(e) => write!(f, "Unauthorized: {}", e),
            NodecosmosError::CharybdisError(e) => write!(f, "Charybdis Error: \n{}", e),
            NodecosmosError::SerdeError(e) => write!(f, "Serde Error: \n{}", e),
            NodecosmosError::ElasticError(e) => write!(f, "Elastic Error: \n{}", e),
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
            NodecosmosError::SerdeError(_) => None,
            NodecosmosError::ElasticError(_) => None,
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
                    HttpResponse::Forbidden().json(json!({ "error": {field: message} }))
                }
                _ => {
                    println!("Internal Server Error: {}", e.to_string().red());

                    HttpResponse::InternalServerError().json(json!({
                        "error": "Internal Server Error",
                        "message": e.to_string()
                    }))
                }
            },
            _ => HttpResponse::InternalServerError().json(json!({
                "error": "Internal Server Error",
                "message": self.to_string()
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

impl From<serde_json::Error> for NodecosmosError {
    fn from(e: serde_json::Error) -> Self {
        NodecosmosError::SerdeError(e)
    }
}

impl From<elasticsearch::Error> for NodecosmosError {
    fn from(e: elasticsearch::Error) -> Self {
        NodecosmosError::ElasticError(e)
    }
}
