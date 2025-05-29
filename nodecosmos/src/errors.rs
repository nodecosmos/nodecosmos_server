use std::error::Error;
use std::fmt;

use actix_web::http::StatusCode;
use actix_web::{HttpResponse, HttpResponseBuilder, ResponseError};
use charybdis::errors::CharybdisError;
use log::{error, warn};
use serde_json::json;

#[derive(Debug)]
pub enum NodecosmosError {
    // 400s
    Unauthorized(&'static str),
    ResourceLocked(&'static str),
    ResourceAlreadyLocked(String),
    Forbidden(String),
    NotFound(String),
    Conflict(String),
    UnsupportedMediaType,
    ValidationError((&'static str, &'static str)),
    PreconditionFailed(&'static str),
    BadRequest(String),
    // 400 | 500
    CharybdisError(CharybdisError),
    // 500
    ConfigError(String),
    ClientSessionError(String),
    SerdeError(serde_json::Error),
    ElasticError(elasticsearch::Error),
    RedisError(redis::RedisError),
    PoolError(deadpool::managed::PoolError<redis::RedisError>),
    LockerError(String),
    AwsSdkError(String),
    TemplateError(String),
    DecodeError(base64::DecodeError),
    YjsError(yrs::encoding::read::Error),
    FatalDeleteError(String),

    /// Merge and recovery fails
    FatalMergeError(String),

    /// Reorder and recovery fails
    FatalReorderError(String),

    InternalServerError(String),
    QuickXmlError(quick_xml::Error),
    BroadcastError(String),
    ParseError(strum::ParseError),
    EmailError(String),
    EmailAlreadyExists,
    ImportError(String),
}

impl fmt::Display for NodecosmosError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NodecosmosError::Unauthorized(e) => write!(f, "Unauthorized: {}", e),
            NodecosmosError::ResourceLocked(e) => write!(f, "ResourceLocked Error: {}", e),
            NodecosmosError::ResourceAlreadyLocked(e) => write!(f, "ResourceAlreadyLocked Error: {}", e),
            NodecosmosError::Forbidden(e) => write!(f, "Forbidden: {}", e),
            NodecosmosError::Conflict(e) => write!(f, "Conflict: {}", e),
            NodecosmosError::UnsupportedMediaType => write!(f, "Unsupported Media Type"),
            NodecosmosError::ValidationError((field, message)) => {
                write!(f, "Validation Error: {}: {}", field, message)
            }
            NodecosmosError::NotFound(e) => write!(f, "Not Found: {}", e),
            NodecosmosError::PreconditionFailed(e) => write!(f, "Precondition Failed: {}", e),
            NodecosmosError::BadRequest(e) => write!(f, "Bad Request: {}", e),
            NodecosmosError::CharybdisError(e) => write!(f, "Charybdis Error: {}", e),
            NodecosmosError::ConfigError(e) => write!(f, "Config Error: {}", e),
            NodecosmosError::ClientSessionError(e) => write!(f, "Client Session Error: {}", e),
            NodecosmosError::SerdeError(e) => write!(f, "Serde Error: {}", e),
            NodecosmosError::ElasticError(e) => write!(f, "Elastic Error: {}", e),
            NodecosmosError::RedisError(e) => write!(f, "Redis Error: {}", e),
            NodecosmosError::PoolError(e) => write!(f, "Pool Error: {}", e),
            NodecosmosError::LockerError(e) => write!(f, "Locker Error: {}", e),
            NodecosmosError::AwsSdkError(e) => write!(f, "Aws SDK Error: {}", e),
            NodecosmosError::TemplateError(e) => write!(f, "TemplateError: {}", e),
            NodecosmosError::DecodeError(e) => write!(f, "Decode Error: {}", e),
            NodecosmosError::YjsError(e) => write!(f, "Yjs Error: {}", e),
            NodecosmosError::FatalDeleteError(e) => write!(f, "Fatal Delete Error: {}", e),
            NodecosmosError::FatalMergeError(e) => write!(f, "Fatal Merge Error: {}", e),
            NodecosmosError::FatalReorderError(e) => write!(f, "Fatal Reorder Error: {}", e),
            NodecosmosError::QuickXmlError(e) => write!(f, "QuickXmlError Error: {}", e),
            NodecosmosError::InternalServerError(e) => write!(f, "InternalServerError: {}", e),
            NodecosmosError::BroadcastError(e) => write!(f, "BroadcastError: {}", e),
            NodecosmosError::ParseError(e) => write!(f, "ParseError: {}", e),
            NodecosmosError::EmailError(e) => write!(f, "EmailError: {}", e),
            NodecosmosError::EmailAlreadyExists => write!(f, "EmailAlreadyExists"),
            NodecosmosError::ImportError(e) => write!(f, "ImportError: {}", e),
        }
    }
}

impl Error for NodecosmosError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            NodecosmosError::CharybdisError(e) => Some(e),
            NodecosmosError::SerdeError(e) => Some(e),
            NodecosmosError::ElasticError(e) => Some(e),
            NodecosmosError::ResourceLocked(_) => None,
            NodecosmosError::RedisError(e) => Some(e),
            NodecosmosError::PoolError(e) => Some(e),
            NodecosmosError::DecodeError(e) => Some(e),
            NodecosmosError::YjsError(e) => Some(e),
            NodecosmosError::QuickXmlError(e) => Some(e),
            NodecosmosError::ParseError(e) => Some(e),
            _ => None,
        }
    }
}

impl ResponseError for NodecosmosError {
    fn error_response(&self) -> HttpResponse {
        match self {
            NodecosmosError::Unauthorized(e) => HttpResponse::Unauthorized().json(json!({
                "status": 401,
                "message": e
            })),
            NodecosmosError::ValidationError((field, message)) => HttpResponse::BadRequest().json(json!({
                "status": 403,
                "message": {field.to_string(): message.to_string()}
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
            NodecosmosError::BadRequest(e) => HttpResponse::BadRequest().json(json!({
                "status": 400,
                "message": e
            })),
            NodecosmosError::ImportError(e) => HttpResponse::BadRequest().json(json!({
                "status": 400,
                "message": e
            })),
            NodecosmosError::UnsupportedMediaType => HttpResponse::UnsupportedMediaType().json({
                json!({
                    "status": 415,
                    "message": "Unsupported Media Type"
                })
            }),
            NodecosmosError::ResourceLocked(e) => {
                warn!("{}", self);

                HttpResponseBuilder::new(StatusCode::LOCKED).json({
                    json!({
                        "status": 423,
                        "message": e
                    })
                })
            }
            NodecosmosError::CharybdisError(e) => match e {
                CharybdisError::NotFoundError(_e) => {
                    warn!("{}", e);

                    HttpResponse::NotFound().json(json!({
                        "status": 404,
                        "message": "Not Found"
                    }))
                }
                _ => NodecosmosError::InternalServerError(format!("CharybdisError: {}", e)).error_response(),
            },
            _ => {
                error!("InternalServerError: {}", self);

                HttpResponse::InternalServerError().json(json!({
                    "status": 500,
                    "message": "Internal Server Error"
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

impl From<redis::RedisError> for NodecosmosError {
    fn from(e: redis::RedisError) -> Self {
        NodecosmosError::RedisError(e)
    }
}

impl From<deadpool::managed::PoolError<redis::RedisError>> for NodecosmosError {
    fn from(e: deadpool::managed::PoolError<redis::RedisError>) -> Self {
        NodecosmosError::PoolError(e)
    }
}

impl<T> From<std::sync::PoisonError<T>> for NodecosmosError {
    fn from(e: std::sync::PoisonError<T>) -> Self {
        NodecosmosError::InternalServerError(e.to_string())
    }
}

impl From<base64::DecodeError> for NodecosmosError {
    fn from(e: base64::DecodeError) -> Self {
        NodecosmosError::DecodeError(e)
    }
}

impl From<yrs::encoding::read::Error> for NodecosmosError {
    fn from(e: yrs::encoding::read::Error) -> Self {
        NodecosmosError::YjsError(e)
    }
}

impl From<quick_xml::Error> for NodecosmosError {
    fn from(e: quick_xml::Error) -> Self {
        NodecosmosError::QuickXmlError(e)
    }
}

impl From<strum::ParseError> for NodecosmosError {
    fn from(e: strum::ParseError) -> Self {
        NodecosmosError::ParseError(e)
    }
}

impl From<anyhow::Error> for NodecosmosError {
    fn from(e: anyhow::Error) -> Self {
        NodecosmosError::InternalServerError(format!("{:?}", e))
    }
}

impl From<actix_web::Error> for NodecosmosError {
    fn from(e: actix_web::Error) -> Self {
        NodecosmosError::InternalServerError(format!("{:?}", e))
    }
}

impl From<yrs::error::UpdateError> for NodecosmosError {
    fn from(e: yrs::error::UpdateError) -> Self {
        NodecosmosError::InternalServerError(format!("{:?}", e))
    }
}
