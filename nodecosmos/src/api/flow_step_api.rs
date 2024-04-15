use actix_web::{delete, post, put, web, HttpResponse};
use charybdis::operations::{DeleteWithCallbacks, InsertWithCallbacks, UpdateWithCallbacks};

use crate::api::data::RequestData;
use crate::api::types::Response;
use crate::models::flow_step::{FlowStep, UpdateInputIdsFlowStep, UpdateNodeIdsFlowStep, UpdateOutputIdsFlowStep};
use crate::models::node::AuthNode;

const LOCKER_TTL: usize = 1000 * 10; // 10 seconds

#[post("")]
pub async fn create_flow_step(data: RequestData, mut flow_step: web::Json<FlowStep>) -> Response {
    AuthNode::auth_update(&data, flow_step.node_id, flow_step.branch_id).await?;

    data.resource_locker()
        .lock_resource(flow_step.flow_id, flow_step.branch_id, LOCKER_TTL)
        .await?;

    let res = flow_step.insert_cb(&data).execute(data.db_session()).await;

    data.resource_locker()
        .unlock_resource(flow_step.flow_id, flow_step.branch_id)
        .await?;

    res?;

    Ok(HttpResponse::Ok().json(flow_step))
}

#[put("/nodes")]
pub async fn update_flow_step_nodes(data: RequestData, mut flow_step: web::Json<UpdateNodeIdsFlowStep>) -> Response {
    AuthNode::auth_update(&data, flow_step.node_id, flow_step.branch_id).await?;

    flow_step.update_cb(&data).execute(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(flow_step))
}

#[put("/outputs")]
pub async fn update_flow_step_outputs(
    data: RequestData,
    mut flow_step: web::Json<UpdateOutputIdsFlowStep>,
) -> Response {
    AuthNode::auth_update(&data, flow_step.node_id, flow_step.branch_id).await?;

    flow_step.update_cb(&data).execute(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(flow_step))
}

#[put("/inputs")]
pub async fn update_flow_step_inputs(data: RequestData, mut flow_step: web::Json<UpdateInputIdsFlowStep>) -> Response {
    AuthNode::auth_update(&data, flow_step.node_id, flow_step.branch_id).await?;

    flow_step.update_cb(&data).execute(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(flow_step))
}

#[delete("{rootId}/{nodeId}/{branchId}/{flowId}/{flowIndex}/{id}")]
pub async fn delete_flow_step(data: RequestData, mut flow_step: web::Path<FlowStep>) -> Response {
    AuthNode::auth_update(&data, flow_step.node_id, flow_step.branch_id).await?;

    data.resource_locker()
        .lock_resource(flow_step.flow_id, flow_step.branch_id, LOCKER_TTL)
        .await?;

    flow_step.delete_cb(&data).execute(data.db_session()).await?;

    data.resource_locker()
        .unlock_resource(flow_step.flow_id, flow_step.branch_id)
        .await?;

    Ok(HttpResponse::Ok().json(flow_step.into_inner()))
}
