use crate::actions::client_session::CurrentUser;
use crate::authorize::auth_workflow_creation;
use crate::errors::NodecosmosError;
use crate::models::flow::BaseFlow;
use crate::models::flow_step::FlowStep;
use crate::models::materialized_views::base_ios_by_root_node_id::InputOutputsByRootNodeId;
use crate::models::workflow::{find_workflow_query, UpdateInitialInputsWorkflow, Workflow};
use actix_web::{get, post, put, web, HttpResponse};
use charybdis::{Find, InsertWithCallbacks, New, UpdateWithCallbacks, Uuid};
use scylla::CachingSession;
use serde_json::json;

#[get("/{node_id}")]
pub async fn get_workflow(
    db_session: web::Data<CachingSession>,
    node_id: web::Path<Uuid>,
) -> Result<HttpResponse, NodecosmosError> {
    let node_id = node_id.into_inner();

    let workflow =
        Workflow::find_one(&db_session, find_workflow_query!("node_id = ?"), (node_id,)).await?;

    // flows
    let mut flow = BaseFlow::new();
    flow.node_id = node_id;
    flow.workflow_id = workflow.id;
    let flows_res = flow.find_by_partition_key(&db_session).await?;
    let flows: Vec<BaseFlow> = flows_res.into_iter().flatten().collect();

    // flow steps
    let mut flow_step = FlowStep::new();
    flow_step.node_id = node_id;
    flow_step.workflow_id = workflow.id;
    let flow_steps_res = flow_step.find_by_partition_key(&db_session).await?;
    let flow_steps: Vec<FlowStep> = flow_steps_res.into_iter().flatten().collect();

    // input outputs
    let mut base_ios = InputOutputsByRootNodeId::new();
    base_ios.root_node_id = workflow.root_node_id;
    let input_outputs_res = base_ios.find_by_partition_key(&db_session).await?;
    let input_outputs: Vec<InputOutputsByRootNodeId> =
        input_outputs_res.into_iter().flatten().collect();

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
    auth_workflow_creation(&db_session, workflow.node_id, current_user).await?;

    workflow.insert_cb(&db_session).await?;

    let mut base_ios = InputOutputsByRootNodeId::new();
    base_ios.root_node_id = workflow.root_node_id;
    let input_outputs_res = base_ios.find_by_partition_key(&db_session).await?;
    let input_outputs: Vec<InputOutputsByRootNodeId> =
        input_outputs_res.into_iter().flatten().collect();

    Ok(HttpResponse::Ok().json(json!({
        "success": true,
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
    auth_workflow_creation(&db_session, workflow.node_id, current_user).await?;

    workflow.update_cb(&db_session).await?;

    Ok(HttpResponse::Ok().json(json!({
        "success": true,
        "workflow": workflow,
    })))
}
