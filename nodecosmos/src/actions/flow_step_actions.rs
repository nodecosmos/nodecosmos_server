use crate::actions::client_session::CurrentUser;
use crate::authorize::auth_workflow_update;
use crate::errors::NodecosmosError;
use crate::models::flow_step::{
    FlowStep, FlowStepDescription, UpdateFlowStepInputIds, UpdateFlowStepNodeIds,
    UpdateFlowStepOutputIds,
};
use actix_web::{delete, post, put, web, HttpResponse};
use charybdis::{DeleteWithCallbacks, Find, InsertWithCallbacks, UpdateWithCallbacks};
use scylla::CachingSession;
use serde_json::json;

#[post("")]
pub async fn create_flow_step(
    db_session: web::Data<CachingSession>,
    mut flow_step: web::Json<FlowStep>,
    current_user: CurrentUser,
) -> Result<HttpResponse, NodecosmosError> {
    auth_workflow_update(
        &db_session,
        flow_step.node_id,
        flow_step.workflow_id,
        current_user,
    )
    .await?;

    flow_step.insert_cb(&db_session).await?;

    Ok(HttpResponse::Ok().json(json!({
        "success": true,
        "flowStep": flow_step,
    })))
}

#[put("/nodes")]
pub async fn update_flow_step_nodes(
    db_session: web::Data<CachingSession>,
    current_user: CurrentUser,
    mut flow_step: web::Json<UpdateFlowStepNodeIds>,
) -> Result<HttpResponse, NodecosmosError> {
    auth_workflow_update(
        &db_session,
        flow_step.node_id,
        flow_step.workflow_id,
        current_user,
    )
    .await?;

    flow_step.update_cb(&db_session).await?;

    Ok(HttpResponse::Ok().json(json!({
        "success": true,
        "flowStep": flow_step,
    })))
}

#[put("/outputs")]
pub async fn update_flow_step_outputs(
    db_session: web::Data<CachingSession>,
    current_user: CurrentUser,
    mut flow_step: web::Json<UpdateFlowStepOutputIds>,
) -> Result<HttpResponse, NodecosmosError> {
    auth_workflow_update(
        &db_session,
        flow_step.node_id,
        flow_step.workflow_id,
        current_user,
    )
    .await?;

    flow_step.update_cb(&db_session).await?;

    Ok(HttpResponse::Ok().json(json!({
        "success": true,
        "flowStep": flow_step,
    })))
}

#[put("/inputs")]
pub async fn update_flow_step_inputs(
    db_session: web::Data<CachingSession>,
    current_user: CurrentUser,
    mut flow_step: web::Json<UpdateFlowStepInputIds>,
) -> Result<HttpResponse, NodecosmosError> {
    auth_workflow_update(
        &db_session,
        flow_step.node_id,
        flow_step.workflow_id,
        current_user,
    )
    .await?;

    flow_step.update_cb(&db_session).await?;

    Ok(HttpResponse::Ok().json(json!({
        "success": true,
        "flowStep": flow_step,
    })))
}

#[put("/description")]
pub async fn update_flow_step_description(
    db_session: web::Data<CachingSession>,
    current_user: CurrentUser,
    mut flow_step: web::Json<FlowStepDescription>,
) -> Result<HttpResponse, NodecosmosError> {
    auth_workflow_update(
        &db_session,
        flow_step.node_id,
        flow_step.workflow_id,
        current_user,
    )
    .await?;

    flow_step.update_cb(&db_session).await?;

    Ok(HttpResponse::Ok().json(json!({
        "success": true,
        "flowStep": flow_step,
    })))
}

#[delete("{nodeId}/{workflowId}/{flowId}/{id}")]
pub async fn delete_flow_step(
    db_session: web::Data<CachingSession>,
    current_user: CurrentUser,
    flow_step: web::Path<FlowStep>,
) -> Result<HttpResponse, NodecosmosError> {
    let mut flow_step = flow_step.find_by_primary_key(&db_session).await?;
    auth_workflow_update(
        &db_session,
        flow_step.node_id,
        flow_step.workflow_id,
        current_user,
    )
    .await?;

    flow_step.delete_cb(&db_session).await?;

    Ok(HttpResponse::Ok().json(json!({
        "success": true,
        "flowStep": flow_step,
    })))
}
