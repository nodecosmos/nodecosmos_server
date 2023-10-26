use crate::authorize::auth_workflow_update;
use crate::errors::NodecosmosError;
use crate::models::flow_step::{
    FlowStep, UpdateDescriptionFlowStep, UpdateFlowStepNodeIds, UpdateInputIdsFlowStep, UpdateOutputIdsFlowStep,
};
use crate::models::user::CurrentUser;
use crate::services::flow_step_idx_calculator::FlowStepIdxCalculator;
use crate::services::resource_locker::ResourceLocker;
use actix_web::{delete, post, put, web, HttpResponse};
use charybdis::operations::{DeleteWithCallbacks, Find, InsertWithCallbacks, UpdateWithCallbacks};
use charybdis::types::Uuid;
use scylla::CachingSession;
use serde::Deserialize;
use serde_json::json;

const LOCKER_TTL: usize = 1000 * 10; // 10 seconds

#[derive(Deserialize)]
pub struct FlowStepCreationParams {
    #[serde(rename = "prevFlowStepId")]
    pub pref_flow_step_id: Option<Uuid>,

    #[serde(rename = "nextFlowStepId")]
    pub next_flow_step_id: Option<Uuid>,

    #[serde(rename = "nodeId")]
    pub node_id: Uuid,

    #[serde(rename = "workflowId")]
    pub workflow_id: Uuid,

    #[serde(rename = "flowId")]
    pub flow_id: Uuid,

    #[serde(rename = "nodeIds")]
    pub node_ids: Option<Vec<Uuid>>,
}

#[post("")]
pub async fn create_flow_step(
    db_session: web::Data<CachingSession>,
    locker: web::Data<ResourceLocker>,
    params: web::Json<FlowStepCreationParams>,
    current_user: CurrentUser,
) -> Result<HttpResponse, NodecosmosError> {
    let mut flow_step = FlowStep {
        node_id: params.node_id,
        workflow_id: params.workflow_id,
        flow_id: params.flow_id,
        node_ids: params.node_ids.clone(),
        id: Uuid::new_v4(),
        ..Default::default()
    };

    auth_workflow_update(&db_session, flow_step.node_id, current_user).await?;

    locker.check_resource_lock(&flow_step.flow_id.to_string()).await?;
    locker.lock_resource(&flow_step.flow_id.to_string(), LOCKER_TTL).await?;

    let index_calculator = FlowStepIdxCalculator::new(&db_session, &params).await?;
    index_calculator.validate()?;

    flow_step.flow_index = index_calculator.calculate_index();

    flow_step.insert_cb(&db_session).await?;

    locker.unlock_resource(&flow_step.flow_id.to_string()).await?;

    Ok(HttpResponse::Ok().json(json!({
        "flowStep": flow_step,
    })))
}

#[put("/nodes")]
pub async fn update_flow_step_nodes(
    db_session: web::Data<CachingSession>,
    current_user: CurrentUser,
    mut flow_step: web::Json<UpdateFlowStepNodeIds>,
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
