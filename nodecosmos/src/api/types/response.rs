use crate::errors::NodecosmosError;
use actix_web::HttpResponse;

pub type Response = Result<HttpResponse, NodecosmosError>;
