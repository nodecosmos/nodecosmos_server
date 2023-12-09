use crate::api::data::RequestData;
use crate::api::types::Response;
use crate::models::flow_step::{
    FlowStep, UpdateDescriptionFlowStep, UpdateInputIdsFlowStep, UpdateNodeIdsFlowStep, UpdateOutputIdsFlowStep,
};
use crate::models::node::AuthNode;
use actix_web::{delete, post, put, web, HttpResponse};
use charybdis::operations::{DeleteWithCallbacks, Find, InsertWithCallbacks, UpdateWithCallbacks};

const LOCKER_TTL: usize = 1000 * 10; // 10 seconds

#[post("")]
pub async fn create_flow_step(data: RequestData, mut flow_step: web::Json<FlowStep>) -> Response {
    AuthNode::auth_update(&data, flow_step.node_id, flow_step.node_id).await?;

    data.resource_locker()
        .validate_resource_unlocked(&flow_step.flow_id.to_string())
        .await?;
    data.resource_locker()
        .lock_resource(&flow_step.flow_id.to_string(), LOCKER_TTL)
        .await?;

    let res = flow_step.insert_cb(data.db_session()).await;

    match res {
        Ok(_) => {
            data.resource_locker()
                .unlock_resource(&flow_step.flow_id.to_string())
                .await?;

            Ok(HttpResponse::Ok().json(flow_step))
        }
        Err(err) => {
            data.resource_locker()
                .unlock_resource(&flow_step.flow_id.to_string())
                .await?;

            Err(err)
        }
    }
}

#[put("/nodes")]
pub async fn update_flow_step_nodes(data: RequestData, mut flow_step: web::Json<UpdateNodeIdsFlowStep>) -> Response {
    AuthNode::auth_update(&data, flow_step.node_id, flow_step.node_id).await?;

    flow_step.update_cb(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(flow_step))
}

#[put("/outputs")]
pub async fn update_flow_step_outputs(
    data: RequestData,
    mut flow_step: web::Json<UpdateOutputIdsFlowStep>,
) -> Response {
    AuthNode::auth_update(&data, flow_step.node_id, flow_step.node_id).await?;

    flow_step.update_cb(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(flow_step))
}

#[put("/inputs")]
pub async fn update_flow_step_inputs(data: RequestData, mut flow_step: web::Json<UpdateInputIdsFlowStep>) -> Response {
    AuthNode::auth_update(&data, flow_step.node_id, flow_step.node_id).await?;

    flow_step.update_cb(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(flow_step))
}

#[put("/description")]
pub async fn update_flow_step_description(
    data: RequestData,
    mut flow_step: web::Json<UpdateDescriptionFlowStep>,
) -> Response {
    AuthNode::auth_update(&data, flow_step.node_id, flow_step.node_id).await?;

    flow_step.update_cb(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(flow_step))
}

#[delete("{nodeId}/{workflowId}/{flowId}/{flowIndex}/{id}")]
pub async fn delete_flow_step(data: RequestData, flow_step: web::Path<FlowStep>) -> Response {
    let mut flow_step = flow_step.find_by_primary_key(data.db_session()).await?;
    AuthNode::auth_update(&data, flow_step.node_id, flow_step.node_id).await?;

    data.resource_locker()
        .validate_resource_unlocked(&flow_step.flow_id.to_string())
        .await?;
    data.resource_locker()
        .lock_resource(&flow_step.flow_id.to_string(), LOCKER_TTL)
        .await?;

    flow_step.delete_cb(data.db_session()).await?;

    data.resource_locker()
        .unlock_resource(&flow_step.flow_id.to_string())
        .await?;

    Ok(HttpResponse::Ok().json(flow_step))
}
