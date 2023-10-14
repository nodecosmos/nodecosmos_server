use actix_web::http::StatusCode;
use actix_web::{HttpResponse, HttpResponseBuilder, ResponseError};
use charybdis::errors::CharybdisError;
use colored::Colorize;
use serde_json::json;
use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub enum RedisError {
    PoolError(deadpool_redis::PoolError),
    RedisError(deadpool_redis::redis::RedisError),
}

impl fmt::Display for RedisError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RedisError::PoolError(e) => write!(f, "Pool Error: {}", e),
            RedisError::RedisError(e) => write!(f, "Redis Error: {}", e),
        }
    }
}

impl Error for RedisError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            RedisError::PoolError(e) => Some(e),
            RedisError::RedisError(e) => Some(e),
        }
    }
}

#[derive(Debug)]
pub enum NodecosmosError {
    ClientSessionError(String),
    Unauthorized(serde_json::Value),
    ResourceLocked(String),
    CharybdisError(CharybdisError),
    SerdeError(serde_json::Error),
    ElasticError(elasticsearch::Error),
    RedisError(RedisError),
    InternalServerError(String),
    Forbidden(String),
    ActixError(actix_web::Error),
    Conflict(String),
    UnsupportedMediaType,
    ValidationError((String, String)),
}

impl fmt::Display for NodecosmosError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NodecosmosError::ClientSessionError(e) => write!(f, "Session Error: {}", e),
            NodecosmosError::Unauthorized(e) => write!(f, "Unauthorized: {}", e),
            NodecosmosError::CharybdisError(e) => write!(f, "Charybdis Error: \n{}", e),
            NodecosmosError::SerdeError(e) => write!(f, "Serde Error: \n{}", e),
            NodecosmosError::ElasticError(e) => write!(f, "Elastic Error: \n{}", e),
            NodecosmosError::ResourceLocked(e) => write!(f, "ResourceLocked Error: \n{}", e),
            NodecosmosError::RedisError(e) => write!(f, "Redis Pool Error: \n{}", e),
            NodecosmosError::InternalServerError(e) => write!(f, "InternalServerError: \n{}", e),
            NodecosmosError::UnsupportedMediaType => write!(f, "Unsupported Media Type"),
            NodecosmosError::Forbidden(e) => write!(f, "Forbidden: {}", e),
            NodecosmosError::ActixError(e) => write!(f, "Actix Error: {}", e),
            NodecosmosError::Conflict(e) => write!(f, "Conflict: {}", e),
            NodecosmosError::ValidationError((field, message)) => {
                write!(f, "Validation Error: {}: {}", field, message)
            }
        }
    }
}

impl Error for NodecosmosError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            NodecosmosError::ClientSessionError(_) => None,
            NodecosmosError::Unauthorized(_) => None,
            NodecosmosError::CharybdisError(e) => Some(e),
            NodecosmosError::SerdeError(e) => Some(e),
            NodecosmosError::ElasticError(e) => Some(e),
            NodecosmosError::ResourceLocked(_) => None,
            NodecosmosError::RedisError(e) => Some(e),
            NodecosmosError::InternalServerError(_) => None,
            NodecosmosError::UnsupportedMediaType => None,
            NodecosmosError::Forbidden(_) => None,
            NodecosmosError::ActixError(e) => Some(e),
            NodecosmosError::Conflict(_) => None,
            NodecosmosError::ValidationError(_) => None,
        }
    }
}

impl ResponseError for NodecosmosError {
    fn error_response(&self) -> HttpResponse {
        match self {
            NodecosmosError::Unauthorized(e) => HttpResponse::Unauthorized().json(e),
            NodecosmosError::ResourceLocked(e) => {
                HttpResponseBuilder::new(StatusCode::LOCKED).json(e)
            }
            NodecosmosError::CharybdisError(e) => match e {
                CharybdisError::NotFoundError(e) => HttpResponse::NotFound().json(json!({
                    "error": "Not Found",
                    "message": e.to_string()
                })),
                _ => {
                    println!("Internal Server Error: {}", e.to_string().red());

                    HttpResponse::InternalServerError().json(json!({
                        "error": "Internal Server Error",
                        "message": e.to_string()
                    }))
                }
            },
            NodecosmosError::UnsupportedMediaType => HttpResponse::UnsupportedMediaType().finish(),
            NodecosmosError::Forbidden(e) => HttpResponse::Forbidden().json(json!({
                "error": "Forbidden",
                "message": e
            })),
            NodecosmosError::Conflict(e) => HttpResponse::Conflict().json(json!({
                "error": "Conflict",
                "message": e
            })),
            NodecosmosError::ValidationError((field, message)) => {
                HttpResponse::BadRequest().json(json!({ "error": {field: message} }))
            }
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

impl From<deadpool_redis::PoolError> for NodecosmosError {
    fn from(e: deadpool_redis::PoolError) -> Self {
        NodecosmosError::RedisError(RedisError::PoolError(e))
    }
}

impl From<deadpool_redis::redis::RedisError> for NodecosmosError {
    fn from(e: deadpool_redis::redis::RedisError) -> Self {
        NodecosmosError::RedisError(RedisError::RedisError(e))
    }
}

impl From<actix_web::Error> for NodecosmosError {
    fn from(e: actix_web::Error) -> Self {
        NodecosmosError::ActixError(e)
    }
}
