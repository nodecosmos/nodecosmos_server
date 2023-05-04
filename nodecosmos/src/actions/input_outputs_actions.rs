use crate::actions::client_session::CurrentUser;
use crate::authorize::auth_node_update;
use crate::errors::NodecosmosError;
use crate::models::input_output::InputOutput;
use crate::models::node::{find_node_query, Node};

use actix_web::{get, post, web, HttpResponse};
use charybdis::*;
use scylla::CachingSession;
use serde_json::json;

#[post("")]
pub async fn create_io(
    db_session: web::Data<CachingSession>,
    current_user: CurrentUser,
    workflow: web::Json<InputOutput>,
) -> Result<HttpResponse, NodecosmosError> {
    unimplemented!()
}
