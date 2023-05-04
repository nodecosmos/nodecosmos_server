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

#[get("/{node_id}")]
pub async fn get_workflow(
    db_session: web::Data<CachingSession>,
    node_id: web::Path<Uuid>,
) -> Result<HttpResponse, NodecosmosError> {
    let node_id = node_id.into_inner();

    let mut workflow = Workflow::new();
    workflow.node_id = node_id;

    let workflow = workflow.find_by_primary_key(&db_session).await?;

    let mut workflow_step = WorkflowStep::new();
    workflow_step.workflow_id = workflow.id;

    let mut workflow_steps = workflow_step.find_by_partition_key(&db_session).await?;

    let mut steps = vec![];
    let mut input_output_ids = vec![];
    let mut input_outputs = vec![];

    while let Some(step) = workflow_steps.next() {
        if let Ok(step) = step {
            step.input_ids
                .iter()
                .cloned()
                .for_each(|io| input_output_ids.push(io));
            step.output_ids
                .iter()
                .cloned()
                .for_each(|io| input_output_ids.push(io));

            steps.push(step);
        }
    }

    let mut input_outputs_res = InputOutput::find(
        &db_session,
        find_input_output_query!("id in (?)"),
        input_output_ids,
    )
    .await?;

    while let Some(input_output) = input_outputs_res.next() {
        if let Ok(input_output) = input_output {
            input_outputs.push(input_output);
        }
    }

    Ok(HttpResponse::Ok().json(json!({
        "workflow": workflow,
        "workflow_steps": steps,
        "input_outputs": input_outputs,
    })))
}

#[post("")]
pub async fn create_workflow(
    db_session: web::Data<CachingSession>,
    current_user: CurrentUser,
    workflow: web::Json<Workflow>,
) -> Result<HttpResponse, NodecosmosError> {
    let mut workflow = workflow.into_inner();
    let node = Node::find_one(&db_session, find_node_query!("id = ?"), (workflow.node_id,)).await?;

    auth_node_update(&node, &current_user).await?;

    workflow.insert_cb(&db_session).await?;

    Ok(HttpResponse::Ok().json(json!({
        "success": true,
    })))
}

#[post("/input_output")]
pub async fn create_io(
    db_session: web::Data<CachingSession>,
    current_user: CurrentUser,
    workflow: web::Json<Workflow>,
) -> Result<HttpResponse, NodecosmosError> {
    unimplemented!()
}
