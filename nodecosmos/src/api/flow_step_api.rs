use actix_web::{post, put, web, HttpResponse};
use charybdis::operations::{DeleteWithCallbacks, Find, InsertWithCallbacks, UpdateWithCallbacks};

use crate::api::data::RequestData;
use crate::api::types::Response;
use crate::models::flow_step::{
    FlowStep, PkFlowStep, UpdateInputIdsFlowStep, UpdateNodeIdsFlowStep, UpdateOutputIdsFlowStep,
};
use crate::models::node::AuthNode;

const LOCKER_TTL: usize = 1000 * 10; // 10 seconds

#[post("")]
pub async fn create_flow_step(data: RequestData, mut flow_step: web::Json<FlowStep>) -> Response {
    AuthNode::auth_update(&data, flow_step.branch_id, flow_step.node_id, flow_step.root_id).await?;

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
    AuthNode::auth_update(&data, flow_step.branch_id, flow_step.node_id, flow_step.root_id).await?;

    flow_step.update_cb(&data).execute(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(flow_step))
}

#[put("/outputs")]
pub async fn update_flow_step_outputs(
    data: RequestData,
    mut flow_step: web::Json<UpdateOutputIdsFlowStep>,
) -> Response {
    AuthNode::auth_update(&data, flow_step.branch_id, flow_step.node_id, flow_step.root_id).await?;

    flow_step.update_cb(&data).execute(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(flow_step))
}

#[put("/inputs")]
pub async fn update_flow_step_inputs(data: RequestData, mut flow_step: web::Json<UpdateInputIdsFlowStep>) -> Response {
    AuthNode::auth_update(&data, flow_step.branch_id, flow_step.node_id, flow_step.root_id).await?;

    flow_step.update_cb(&data).execute(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(flow_step))
}

#[post("/delete")]
pub async fn delete_flow_step(data: RequestData, fs: web::Json<PkFlowStep>) -> Response {
    AuthNode::auth_update(&data, fs.branch_id, fs.node_id, fs.root_id).await?;

    data.resource_locker()
        .lock_resource(fs.flow_id, fs.branch_id, LOCKER_TTL)
        .await?;

    let mut flow_step =
        FlowStep::find_by_primary_key_value(&(fs.branch_id, fs.node_id, fs.flow_id, fs.step_index.clone(), fs.id))
            .execute(data.db_session())
            .await?;
    flow_step.delete_cb(&data).execute(data.db_session()).await?;

    data.resource_locker()
        .unlock_resource(flow_step.flow_id, flow_step.branch_id)
        .await?;

    Ok(HttpResponse::Ok().json(flow_step))
}
