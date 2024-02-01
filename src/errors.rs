use crate::utils::logger::log_fatal;
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
    // 400s
    Unauthorized(serde_json::Value),
    ResourceLocked(&'static str),
    Forbidden(String),
    NotFound(String),
    Conflict(String),
    UnsupportedMediaType,
    ValidationError((String, String)),
    PreconditionFailed(&'static str),
    // 400 | 500
    CharybdisError(CharybdisError),
    // 500
    ClientSessionError(String),
    SerdeError(serde_json::Error),
    ElasticError(elasticsearch::Error),
    RedisError(RedisError),
    ActixError(actix_web::Error),
    LockerError(String),
    InternalServerError(String),
}

impl fmt::Display for NodecosmosError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NodecosmosError::Unauthorized(e) => write!(f, "Unauthorized: {}", e),
            NodecosmosError::ResourceLocked(e) => write!(f, "ResourceLocked Error: \n{}", e),
            NodecosmosError::Forbidden(e) => write!(f, "Forbidden: {}", e),
            NodecosmosError::Conflict(e) => write!(f, "Conflict: {}", e),
            NodecosmosError::UnsupportedMediaType => write!(f, "Unsupported Media Type"),
            NodecosmosError::ValidationError((field, message)) => {
                write!(f, "Validation Error: {}: {}", field, message)
            }
            NodecosmosError::NotFound(e) => write!(f, "Not Found: {}", e),
            NodecosmosError::PreconditionFailed(e) => write!(f, "Precondition Failed: {}", e),
            NodecosmosError::CharybdisError(e) => write!(f, "Charybdis Error: \n{}", e),
            NodecosmosError::ClientSessionError(e) => write!(f, "Session Error: {}", e),
            NodecosmosError::SerdeError(e) => write!(f, "Serde Error: \n{}", e),
            NodecosmosError::ElasticError(e) => write!(f, "Elastic Error: \n{}", e),
            NodecosmosError::RedisError(e) => write!(f, "Redis Pool Error: \n{}", e),
            NodecosmosError::ActixError(e) => write!(f, "Actix Error: {}", e),
            NodecosmosError::LockerError(e) => write!(f, "Locker Error: {}", e),
            NodecosmosError::InternalServerError(e) => write!(f, "InternalServerError: \n{}", e),
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
            NodecosmosError::LockerError(_) => None,
            NodecosmosError::NotFound(_) => None,
            NodecosmosError::PreconditionFailed(_) => None,
        }
    }
}

impl ResponseError for NodecosmosError {
    fn error_response(&self) -> HttpResponse {
        match self {
            NodecosmosError::Unauthorized(_) => HttpResponse::Unauthorized().json({
                json!({
                    "status": 401,
                    "message": "Unauthorized"
                })
            }),
            NodecosmosError::ValidationError((field, message)) => HttpResponse::BadRequest().json(json!({
                "status": 403,
                "message": {field: message}
            })),
            NodecosmosError::Forbidden(e) => HttpResponse::Forbidden().json(json!({
                "status": 403,
                "message": e
            })),
            NodecosmosError::NotFound(e) => HttpResponse::NotFound().json(json!({
                "status": 404,
                "message": e
            })),
            NodecosmosError::Conflict(e) => HttpResponse::Conflict().json(json!({
                "status": 409,
                "message": e
            })),
            NodecosmosError::PreconditionFailed(e) => HttpResponse::PreconditionFailed().json(json!({
                "status": 412,
                "message": e
            })),
            NodecosmosError::UnsupportedMediaType => HttpResponse::UnsupportedMediaType().json({
                json!({
                    "status": 415,
                    "message": "Unsupported Media Type"
                })
            }),
            NodecosmosError::ResourceLocked(e) => HttpResponseBuilder::new(StatusCode::LOCKED).json({
                json!({
                    "status": 423,
                    "message": e
                })
            }),
            NodecosmosError::CharybdisError(e) => match e {
                CharybdisError::NotFoundError(e) => HttpResponse::NotFound().json(json!({
                    "status": 404,
                    "message": e.to_string()
                })),
                _ => {
                    NodecosmosError::InternalServerError(format!("CharybdisError: {}", e.to_string())).error_response()
                }
            },
            _ => {
                println!("Internal Server Error: {}", self.to_string().red());

                HttpResponse::InternalServerError().json(json!({
                    "status": 500,
                    "message": self.to_string()
                }))
            }
        }
    }
}

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

impl<T> From<std::sync::PoisonError<T>> for NodecosmosError {
    fn from(e: std::sync::PoisonError<T>) -> Self {
        NodecosmosError::InternalServerError(e.to_string())
    }
}
