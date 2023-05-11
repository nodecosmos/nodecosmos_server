use crate::actions::client_session::CurrentUser;
use crate::authorize::auth_workflow_update;
use crate::errors::NodecosmosError;
use crate::models::flow_step::{FlowStep, UpdateFlowInputIds, UpdateFlowOutputIds};

use actix_web::{delete, post, put, web, HttpResponse};
use charybdis::*;
use scylla::CachingSession;
use serde_json::json;

#[post("")]
pub async fn create_flow_step(
    db_session: web::Data<CachingSession>,
    flow_step: web::Json<FlowStep>,
    current_user: CurrentUser,
) -> Result<HttpResponse, NodecosmosError> {
    let mut flow_step = flow_step.into_inner();

    auth_workflow_update(
        &db_session,
        flow_step.node_id,
        flow_step.workflow_id,
        current_user,
    )
    .await?;

    flow_step.insert_cb(&db_session).await?;

    Ok(HttpResponse::Ok().json(json!({
        "success": true,
        "flow_step": flow_step,
    })))
}

#[derive(Deserialize)]
pub enum UpdateAttributes {
    InputIdsByNodeId,
    OutputIdsByNodeId,
}

#[derive(Deserialize)]
pub struct UpdateParams {
    pub node_id: Uuid,
    pub workflow_id: Uuid,
    pub flow_id: Uuid,
    pub step: i32,
    pub update_attribute: UpdateAttributes,
    pub update_value: Map<Uuid, Frozen<Set<Uuid>>>,
}

#[put("")]
pub async fn update_flow_step(
    db_session: web::Data<CachingSession>,
    current_user: CurrentUser,
    params: web::Json<UpdateParams>,
) -> Result<HttpResponse, NodecosmosError> {
    let params = params.into_inner();

    match params.update_attribute {
        UpdateAttributes::InputIdsByNodeId => {
            let mut flow_step = UpdateFlowInputIds::new();

            flow_step.node_id = params.node_id;
            flow_step.workflow_id = params.workflow_id;
            flow_step.flow_id = params.flow_id;
            flow_step.step = params.step;
            flow_step.input_ids_by_node_id = Some(params.update_value);

            auth_workflow_update(
                &db_session,
                params.node_id,
                params.workflow_id,
                current_user,
            )
            .await?;
            flow_step.update_cb(&db_session).await?;

            Ok(HttpResponse::Ok().json(json!({
                "success": true,
                "flow_step": flow_step,
            })))
        }
        UpdateAttributes::OutputIdsByNodeId => {
            let mut flow_step = UpdateFlowOutputIds::new();

            flow_step.node_id = params.node_id;
            flow_step.workflow_id = params.workflow_id;
            flow_step.flow_id = params.flow_id;
            flow_step.step = params.step;
            flow_step.output_ids_by_node_id = Some(params.update_value);

            auth_workflow_update(
                &db_session,
                params.node_id,
                params.workflow_id,
                current_user,
            )
            .await?;
            flow_step.update_cb(&db_session).await?;

            Ok(HttpResponse::Ok().json(json!({
                "success": true,
                "flow_step": flow_step,
            })))
        }
    }
}

#[derive(Deserialize)]
pub struct DeleteParams {
    pub node_id: Uuid,
    pub workflow_id: Uuid,
    pub flow_id: Uuid,
    pub step: i32,
}

#[delete("{node_id}/{workflow_id}/{flow_id}/{step}")]
pub async fn delete_flow_step(
    db_session: web::Data<CachingSession>,
    current_user: CurrentUser,
    params: web::Path<DeleteParams>,
) -> Result<HttpResponse, NodecosmosError> {
    let params = params.into_inner();
    let mut flow_step = FlowStep::new();

    flow_step.node_id = params.node_id;
    flow_step.workflow_id = params.workflow_id;
    flow_step.flow_id = params.flow_id;
    flow_step.step = params.step;

    let mut flow_step = flow_step.find_by_primary_key(&db_session).await?;
    auth_workflow_update(
        &db_session,
        params.node_id,
        params.workflow_id,
        current_user,
    )
    .await?;

    flow_step.delete_cb(&db_session).await?;

    Ok(HttpResponse::Ok().finish())
}
