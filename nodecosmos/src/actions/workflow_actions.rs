use crate::authorize::{auth_workflow_creation, auth_workflow_update};
use crate::errors::NodecosmosError;
use crate::models::flow::BaseFlow;
use crate::models::flow_step::FlowStep;
use crate::models::input_output::InputOutput;
use crate::models::user::CurrentUser;
use crate::models::workflow::{UpdateInitialInputsWorkflow, UpdateWorkflowTitle, Workflow};
use actix_web::{delete, get, post, put, web, HttpResponse};
use charybdis::operations::{DeleteWithCallbacks, Find, InsertWithCallbacks, New, UpdateWithCallbacks};
use charybdis::types::Uuid;
use scylla::CachingSession;
use serde::Deserialize;
use serde_json::json;

#[get("/{node_id}")]
pub async fn get_workflow(
    db_session: web::Data<CachingSession>,
    node_id: web::Path<Uuid>,
) -> Result<HttpResponse, NodecosmosError> {
    let node_id = node_id.into_inner();

    let workflow = Workflow::find_one_by_partition_key_value(&db_session, (node_id,)).await?;

    // flows
    let mut flow = BaseFlow::new();
    flow.node_id = node_id;
    let flows = flow.find_by_partition_key(&db_session).await?.try_collect().await?;

    // flow steps
    let mut flow_step = FlowStep::new();
    flow_step.node_id = node_id;
    let flow_steps = flow_step
        .find_by_partition_key(&db_session)
        .await?
        .try_collect()
        .await?;

    // input outputs
    let input_outputs = InputOutput::find_by_partition_key_value(&db_session, (workflow.root_node_id,))
        .await?
        .try_collect()
        .await?;

    Ok(HttpResponse::Ok().json(json!({
        "workflow": workflow,
        "flows": flows,
        "flowSteps": flow_steps,
        "inputOutputs": input_outputs,
    })))
}

#[post("")]
pub async fn create_workflow(
    db_session: web::Data<CachingSession>,
    current_user: CurrentUser,
    mut workflow: web::Json<Workflow>,
) -> Result<HttpResponse, NodecosmosError> {
    auth_workflow_creation(&db_session, &workflow, &current_user).await?;

    workflow.insert_cb(&db_session).await?;

    let input_outputs = InputOutput::find_by_partition_key_value(&db_session, (workflow.root_node_id,))
        .await?
        .try_collect()
        .await?;

    Ok(HttpResponse::Ok().json(json!({
        "workflow": workflow,
        "inputOutputs": input_outputs,
    })))
}

#[put("/initial_input_ids")]
pub async fn update_initial_inputs(
    db_session: web::Data<CachingSession>,
    current_user: CurrentUser,
    mut workflow: web::Json<UpdateInitialInputsWorkflow>,
) -> Result<HttpResponse, NodecosmosError> {
    auth_workflow_update(&db_session, workflow.node_id, current_user).await?;

    workflow.update_cb(&db_session).await?;

    Ok(HttpResponse::Ok().json(json!({
        "workflow": workflow,
    })))
}

#[put("/title")]
pub async fn update_workflow_title(
    db_session: web::Data<CachingSession>,
    current_user: CurrentUser,
    mut workflow: web::Json<UpdateWorkflowTitle>,
) -> Result<HttpResponse, NodecosmosError> {
    auth_workflow_update(&db_session, workflow.node_id, current_user).await?;

    workflow.update_cb(&db_session).await?;

    Ok(HttpResponse::Ok().json(json!({
        "workflow": workflow,
    })))
}

#[derive(Deserialize)]
pub struct DeleteWfParams {
    node_id: Uuid,
    workflow_id: Uuid,
}

#[delete("/{node_id}/{workflow_id}")]
pub async fn delete_workflow(
    db_session: web::Data<CachingSession>,
    current_user: CurrentUser,
    params: web::Path<DeleteWfParams>,
) -> Result<HttpResponse, NodecosmosError> {
    let mut workflow = Workflow::find_by_primary_key_value(&db_session, (params.node_id, params.workflow_id)).await?;

    auth_workflow_update(&db_session, workflow.node_id, current_user).await?;

    workflow.delete_cb(&db_session).await?;

    Ok(HttpResponse::Ok().json(json!({
        "workflow": workflow,
    })))
}
