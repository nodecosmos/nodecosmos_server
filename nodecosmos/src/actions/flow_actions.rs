use crate::actions::client_session::CurrentUser;
use crate::authorize::auth_workflow_update;
use crate::errors::NodecosmosError;
use crate::models::flow::{Flow, UpdateFlowDescription, UpdateFlowTitle};

use actix_web::{delete, post, put, web, HttpResponse};
use charybdis::*;
use scylla::CachingSession;
use serde_json::json;

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

#[derive(Deserialize)]
pub enum UpdateAttributes {
    Title,
    Description,
}

#[derive(Deserialize)]
pub struct UpdateParams {
    pub workflow_id: Uuid,
    pub node_id: Uuid,
    pub id: Uuid,
    pub update_attribute: UpdateAttributes,
    pub update_value: String,
}

#[put("")]
pub async fn update_flow(
    db_session: web::Data<CachingSession>,
    current_user: CurrentUser,
    params: web::Json<UpdateParams>,
) -> Result<HttpResponse, NodecosmosError> {
    let params = params.into_inner();

    match params.update_attribute {
        UpdateAttributes::Title => {
            let mut flow = UpdateFlowTitle::new();
            flow.title = params.update_value;
            flow.node_id = params.node_id;
            flow.workflow_id = params.workflow_id;
            flow.id = params.id;

            auth_workflow_update(
                &db_session,
                params.node_id,
                params.workflow_id,
                current_user,
            )
            .await?;
            flow.update_cb(&db_session).await?;

            Ok(HttpResponse::Ok().json(json!({
                "success": true,
                "flow": flow,
            })))
        }
        UpdateAttributes::Description => {
            let mut flow = UpdateFlowDescription::new();
            flow.description = params.update_value;
            flow.node_id = params.node_id;
            flow.workflow_id = params.workflow_id;
            flow.id = params.id;

            auth_workflow_update(
                &db_session,
                params.node_id,
                params.workflow_id,
                current_user,
            )
            .await?;
            flow.update_cb(&db_session).await?;

            Ok(HttpResponse::Ok().json(json!({
                "success": true,
                "flow": flow,
            })))
        }
    }
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

    Ok(HttpResponse::Ok().finish())
}
