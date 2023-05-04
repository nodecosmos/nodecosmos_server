use crate::actions::client_session::CurrentUser;
use crate::authorize::auth_node_update;
use crate::errors::NodecosmosError;
use crate::models::input_output::{find_input_output_query, InputOutput};
use crate::models::node::{find_node_query, Node};
use crate::models::workflow::Workflow;
use crate::models::workflow_step::WorkflowStep;

use actix_web::{get, post, web, HttpResponse};
use charybdis::*;
use scylla::CachingSession;
use serde_json::json;

#[post("")]
pub async fn create_workflow_step(
    db_session: web::Data<CachingSession>,
    current_user: CurrentUser,
    workflow: web::Json<WorkflowStep>,
) -> Result<HttpResponse, NodecosmosError> {
    unimplemented!()
}

pub struct Params {
    pub workflow_id: Uuid,
    pub id: Uuid,
}

#[delete("/{workflow_id}/{id}")]
pub async fn delete_workflow_step(
    db_session: web::Data<CachingSession>,
    current_user: CurrentUser,
    params: web::Path<Params>,
) -> Result<HttpResponse, NodecosmosError> {
    unimplemented!()
}
