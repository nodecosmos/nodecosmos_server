use crate::actions::client_session::CurrentUser;
use crate::authorize::auth_workflow_update;
use crate::errors::NodecosmosError;
use crate::models::flow::{Flow, FlowDescription, UpdateFlowTitle};

use actix_web::{delete, get, post, put, web, HttpResponse};
use charybdis::{DeleteWithCallbacks, Find, InsertWithCallbacks, New, UpdateWithCallbacks, Uuid};
use scylla::CachingSession;
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Deserialize)]
pub struct PrimaryKeyParams {
    pub node_id: Uuid,
    pub workflow_id: Uuid,
    pub id: Uuid,
}

#[post("")]
pub async fn create_flow(
    db_session: web::Data<CachingSession>,
    current_user: CurrentUser,
    flow: web::Json<Flow>,
) -> Result<HttpResponse, NodecosmosError> {
    let mut flow = flow.into_inner();

    auth_workflow_update(&db_session, flow.node_id, flow.workflow_id, current_user).await?;

    flow.insert_cb(&db_session).await?;

    Ok(HttpResponse::Ok().json(json!({
        "success": true,
        "flow": flow,
    })))
}

#[get("/{node_id}/{workflow_id}/{id}/description")]
pub async fn get_flow_description(
    db_session: web::Data<CachingSession>,
    _current_user: CurrentUser,
    params: web::Path<PrimaryKeyParams>,
) -> Result<HttpResponse, NodecosmosError> {
    let params = params.into_inner();
    let mut flow = FlowDescription::new();

    flow.node_id = params.node_id;
    flow.workflow_id = params.workflow_id;
    flow.id = params.id;

    let flow = flow.find_by_primary_key(&db_session).await?;

    Ok(HttpResponse::Ok().json(json!({
        "success": true,
        "flow": flow,
    })))
}

#[put("/title")]
pub async fn update_flow_title(
    db_session: web::Data<CachingSession>,
    current_user: CurrentUser,
    flow: web::Json<UpdateFlowTitle>,
) -> Result<HttpResponse, NodecosmosError> {
    let mut flow = flow.into_inner();

    auth_workflow_update(&db_session, flow.node_id, flow.workflow_id, current_user).await?;

    flow.update_cb(&db_session).await?;

    Ok(HttpResponse::Ok().json(json!({
        "success": true,
        "flow": flow,
    })))
}

#[put("/description")]
pub async fn update_flow_description(
    db_session: web::Data<CachingSession>,
    current_user: CurrentUser,
    flow: web::Json<FlowDescription>,
) -> Result<HttpResponse, NodecosmosError> {
    let mut flow = flow.into_inner();

    auth_workflow_update(&db_session, flow.node_id, flow.workflow_id, current_user).await?;

    flow.update_cb(&db_session).await?;

    Ok(HttpResponse::Ok().json(json!({
        "success": true,
        "flow": flow,
    })))
}

#[derive(Deserialize)]
pub struct DeleteParams {
    pub node_id: Uuid,
    pub workflow_id: Uuid,
    pub id: Uuid,
}

#[delete("{node_id}/{workflow_id}/{id}")]
pub async fn delete_flow(
    db_session: web::Data<CachingSession>,
    current_user: CurrentUser,
    params: web::Path<DeleteParams>,
) -> Result<HttpResponse, NodecosmosError> {
    let params = params.into_inner();
    let mut flow = Flow::new();

    flow.node_id = params.node_id;
    flow.workflow_id = params.workflow_id;
    flow.id = params.id;

    let mut flow = flow.find_by_primary_key(&db_session).await?;
    auth_workflow_update(
        &db_session,
        params.node_id,
        params.workflow_id,
        current_user,
    )
    .await?;

    flow.delete_cb(&db_session).await?;

    Ok(HttpResponse::Ok().json(json!({
        "success": true,
        "flow": flow,
    })))
}
