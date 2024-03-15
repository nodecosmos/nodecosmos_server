use crate::api::data::RequestData;
use crate::api::types::Response;
use crate::models::flow::{DescriptionFlow, Flow, UpdateTitleFlow};
use crate::models::node::AuthNode;
use crate::models::user::CurrentUser;
use actix_web::{delete, get, post, put, web, HttpResponse};
use charybdis::operations::{DeleteWithCallbacks, Find, InsertWithCallbacks, UpdateWithCallbacks};
use scylla::CachingSession;

#[post("")]
pub async fn create_flow(data: RequestData, mut flow: web::Json<Flow>) -> Response {
    AuthNode::auth_update(&data, flow.node_id, flow.node_id).await?;

    flow.insert_cb(&None).execute(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(flow))
}

#[get("/{nodeId}/{workflowId}/{verticalIndex}/{startIndex}/{id}/description")]
pub async fn get_flow_description(
    db_session: web::Data<CachingSession>,
    _current_user: CurrentUser,
    flow: web::Path<DescriptionFlow>,
) -> Response {
    let flow = flow.find_by_primary_key().execute(&db_session).await?;

    Ok(HttpResponse::Ok().json(flow))
}

#[put("/title")]
pub async fn update_flow_title(data: RequestData, mut flow: web::Json<UpdateTitleFlow>) -> Response {
    AuthNode::auth_update(&data, flow.node_id, flow.node_id).await?;

    flow.update_cb(&None).execute(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(flow))
}

#[put("/description")]
pub async fn update_flow_description(data: RequestData, mut flow: web::Json<DescriptionFlow>) -> Response {
    AuthNode::auth_update(&data, flow.node_id, flow.node_id).await?;

    flow.update_cb(&None).execute(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(flow))
}

#[delete("/{nodeId}/{workflowId}/{verticalIndex}/{startIndex}/{id}")]
pub async fn delete_flow(data: RequestData, mut flow: web::Path<Flow>) -> Response {
    AuthNode::auth_update(&data, flow.node_id, flow.node_id).await?;

    flow.delete_cb(&None).execute(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(flow.into_inner()))
}
