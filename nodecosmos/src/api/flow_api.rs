use crate::api::data::RequestData;
use crate::api::types::Response;
use crate::models::flow::{Flow, UpdateTitleFlow};
use crate::models::node::AuthNode;
use actix_web::{delete, post, put, web, HttpResponse};
use charybdis::operations::{DeleteWithCallbacks, InsertWithCallbacks, UpdateWithCallbacks};

#[post("")]
pub async fn create_flow(data: RequestData, mut flow: web::Json<Flow>) -> Response {
    AuthNode::auth_update(&data, flow.node_id, flow.branch_id).await?;

    flow.insert_cb(&data).execute(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(flow))
}

#[put("/title")]
pub async fn update_flow_title(data: RequestData, mut flow: web::Json<UpdateTitleFlow>) -> Response {
    AuthNode::auth_update(&data, flow.node_id, flow.branch_id).await?;

    flow.update_cb(&data).execute(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(flow))
}

#[delete("/{nodeId}/{branchId}/{workflowId}/{verticalIndex}/{startIndex}/{id}")]
pub async fn delete_flow(data: RequestData, mut flow: web::Path<Flow>) -> Response {
    AuthNode::auth_update(&data, flow.node_id, flow.branch_id).await?;

    flow.delete_cb(&data).execute(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(flow.into_inner()))
}
