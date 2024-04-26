use actix_web::HttpResponse;

use crate::errors::NodecosmosError;

pub type Response = Result<HttpResponse, NodecosmosError>;
