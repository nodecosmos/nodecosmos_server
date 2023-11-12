use crate::authorize::auth_workflow_update;
use crate::errors::NodecosmosError;
use crate::models::flow_step::{
    FlowStep, UpdateDescriptionFlowStep, UpdateInputIdsFlowStep, UpdateNodeIdsFlowStep, UpdateOutputIdsFlowStep,
};
use crate::models::user::CurrentUser;
use crate::services::resource_locker::ResourceLocker;
use actix_web::{delete, post, put, web, HttpResponse};
use charybdis::operations::{DeleteWithCallbacks, Find, InsertWithCallbacks, UpdateWithCallbacks};
use scylla::CachingSession;
use serde_json::json;

const LOCKER_TTL: usize = 1000 * 10; // 10 seconds

#[post("")]
pub async fn create_flow_step(
    db_session: web::Data<CachingSession>,
    locker: web::Data<ResourceLocker>,
    mut flow_step: web::Json<FlowStep>,
    current_user: CurrentUser,
) -> Result<HttpResponse, NodecosmosError> {
    auth_workflow_update(&db_session, flow_step.node_id, current_user).await?;

    locker.check_resource_lock(&flow_step.flow_id.to_string()).await?;
    locker.lock_resource(&flow_step.flow_id.to_string(), LOCKER_TTL).await?;

    let res = flow_step.insert_cb(&db_session).await;

    match res {
        Ok(_) => {
            locker.unlock_resource(&flow_step.flow_id.to_string()).await?;

            Ok(HttpResponse::Ok().json(json!({
                "flowStep": flow_step,
            })))
        }
        Err(err) => {
            locker.unlock_resource(&flow_step.flow_id.to_string()).await?;

            Err(err)
        }
    }
}

#[put("/nodes")]
pub async fn update_flow_step_nodes(
    db_session: web::Data<CachingSession>,
    current_user: CurrentUser,
    mut flow_step: web::Json<UpdateNodeIdsFlowStep>,
) -> Result<HttpResponse, NodecosmosError> {
    auth_workflow_update(&db_session, flow_step.node_id, current_user).await?;

    flow_step.update_cb(&db_session).await?;

    Ok(HttpResponse::Ok().json(json!({
        "flowStep": flow_step,
    })))
}

#[put("/outputs")]
pub async fn update_flow_step_outputs(
    db_session: web::Data<CachingSession>,
    current_user: CurrentUser,
    mut flow_step: web::Json<UpdateOutputIdsFlowStep>,
) -> Result<HttpResponse, NodecosmosError> {
    auth_workflow_update(&db_session, flow_step.node_id, current_user).await?;

    flow_step.update_cb(&db_session).await?;

    Ok(HttpResponse::Ok().json(json!({
        "flowStep": flow_step,
    })))
}

#[put("/inputs")]
pub async fn update_flow_step_inputs(
    db_session: web::Data<CachingSession>,
    current_user: CurrentUser,
    mut flow_step: web::Json<UpdateInputIdsFlowStep>,
) -> Result<HttpResponse, NodecosmosError> {
    auth_workflow_update(&db_session, flow_step.node_id, current_user).await?;

    flow_step.update_cb(&db_session).await?;

    Ok(HttpResponse::Ok().json(json!({
        "flowStep": flow_step,
    })))
}

#[put("/description")]
pub async fn update_flow_step_description(
    db_session: web::Data<CachingSession>,
    current_user: CurrentUser,
    mut flow_step: web::Json<UpdateDescriptionFlowStep>,
) -> Result<HttpResponse, NodecosmosError> {
    auth_workflow_update(&db_session, flow_step.node_id, current_user).await?;

    flow_step.update_cb(&db_session).await?;

    Ok(HttpResponse::Ok().json(json!({
        "flowStep": flow_step,
    })))
}

#[delete("{nodeId}/{workflowId}/{flowId}/{flowIndex}/{id}")]
pub async fn delete_flow_step(
    db_session: web::Data<CachingSession>,
    current_user: CurrentUser,
    flow_step: web::Path<FlowStep>,
    locker: web::Data<ResourceLocker>,
) -> Result<HttpResponse, NodecosmosError> {
    let mut flow_step = flow_step.find_by_primary_key(&db_session).await?;
    auth_workflow_update(&db_session, flow_step.node_id, current_user).await?;

    locker.check_resource_lock(&flow_step.flow_id.to_string()).await?;
    locker.lock_resource(&flow_step.flow_id.to_string(), LOCKER_TTL).await?;

    flow_step.delete_cb(&db_session).await?;

    locker.unlock_resource(&flow_step.flow_id.to_string()).await?;

    Ok(HttpResponse::Ok().json(json!({
        "flowStep": flow_step,
    })))
}
