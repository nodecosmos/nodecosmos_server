use crate::authorize::auth_workflow_update;
use crate::errors::NodecosmosError;
use crate::models::flow::{Flow, FlowDescription, UpdateFlowTitle};
use crate::models::user::CurrentUser;
use actix_web::{delete, get, post, put, web, HttpResponse};
use charybdis::operations::{DeleteWithCallbacks, Find, InsertWithCallbacks, UpdateWithCallbacks};
use scylla::CachingSession;
use serde_json::json;

#[post("")]
pub async fn create_flow(
    db_session: web::Data<CachingSession>,
    current_user: CurrentUser,
    mut flow: web::Json<Flow>,
) -> Result<HttpResponse, NodecosmosError> {
    auth_workflow_update(&db_session, flow.node_id, current_user).await?;

    flow.insert_cb(&db_session).await?;

    Ok(HttpResponse::Ok().json(json!({
        "flow": flow,
    })))
}

#[get("/{nodeId}/{workflowId}/{startIndex}{verticalIndex}/description")]
pub async fn get_flow_description(
    db_session: web::Data<CachingSession>,
    _current_user: CurrentUser,
    flow: web::Path<FlowDescription>,
) -> Result<HttpResponse, NodecosmosError> {
    let flow = flow.find_by_primary_key(&db_session).await?;

    Ok(HttpResponse::Ok().json(json!({
        "flow": flow,
    })))
}

#[put("/title")]
pub async fn update_flow_title(
    db_session: web::Data<CachingSession>,
    current_user: CurrentUser,
    mut flow: web::Json<UpdateFlowTitle>,
) -> Result<HttpResponse, NodecosmosError> {
    auth_workflow_update(&db_session, flow.node_id, current_user).await?;

    flow.update_cb(&db_session).await?;

    Ok(HttpResponse::Ok().json(json!({
        "flow": flow,
    })))
}

#[put("/description")]
pub async fn update_flow_description(
    db_session: web::Data<CachingSession>,
    current_user: CurrentUser,
    mut flow: web::Json<FlowDescription>,
) -> Result<HttpResponse, NodecosmosError> {
    auth_workflow_update(&db_session, flow.node_id, current_user).await?;

    flow.update_cb(&db_session).await?;

    Ok(HttpResponse::Ok().json(json!({
        "flow": flow,
    })))
}

#[delete("/{nodeId}/{workflowId}/{startIndex}{verticalIndex}")]
pub async fn delete_flow(
    db_session: web::Data<CachingSession>,
    current_user: CurrentUser,
    mut flow: web::Path<Flow>,
) -> Result<HttpResponse, NodecosmosError> {
    auth_workflow_update(&db_session, flow.node_id, current_user).await?;

    flow.delete_cb(&db_session).await?;

    Ok(HttpResponse::Ok().json(json!({
        "flow": flow.into_inner(),
    })))
}
