use crate::actions::client_session::CurrentUser;
use crate::authorize::auth_node_update;
use crate::errors::NodecosmosError;
use crate::models::node::{find_node_query, Node};
use crate::models::workflow_step::{
    UpdateWorkflowStepDescription, UpdateWorkflowStepInputIds, UpdateWorkflowStepOutputIds,
    UpdateWorkflowStepTitle, WorkflowStep,
};

use actix_web::{delete, post, put, web, HttpResponse};
use charybdis::*;
use scylla::CachingSession;
use serde_json::{from_str, json};

#[post("")]
pub async fn create_workflow_step(
    db_session: web::Data<CachingSession>,
    current_user: CurrentUser,
    workflow_step: web::Json<WorkflowStep>,
) -> Result<HttpResponse, NodecosmosError> {
    let mut workflow_step = workflow_step.into_inner();
    let node = Node::find_one(
        &db_session,
        find_node_query!("id = ?"),
        (workflow_step.node_id,),
    )
    .await?;

    auth_node_update(&node, &current_user).await?;

    workflow_step.insert_cb(&db_session).await?;

    Ok(HttpResponse::Ok().json(json!({
        "success": true,
        "workflow_step": workflow_step,
    })))
}

#[derive(Deserialize)]
pub enum UpdateAttributes {
    Title,
    Description,
    InputIds,
    OutputIds,
}

#[derive(Deserialize)]
pub struct UpdateParams {
    pub workflow_id: Uuid,
    pub id: Uuid,
    pub update_attribute: UpdateAttributes,
    pub update_value: String,
}

#[put("")]
pub async fn update_workflow_step(
    db_session: web::Data<CachingSession>,
    current_user: CurrentUser,
    params: web::Json<UpdateParams>,
) -> Result<HttpResponse, NodecosmosError> {
    let params = params.into_inner();

    match params.update_attribute {
        UpdateAttributes::Title => {
            let mut workflow_step = UpdateWorkflowStepTitle::new();
            workflow_step.title = params.update_value;

            workflow_step.workflow_id = params.workflow_id;
            workflow_step.id = params.id;

            authorize(&db_session, &workflow_step.node_id, current_user).await?;
            workflow_step.update_cb(&db_session).await?;

            Ok(HttpResponse::Ok().json(json!({
                "success": true,
                "workflow_step": workflow_step,
            })))
        }
        UpdateAttributes::Description => {
            let mut workflow_step = UpdateWorkflowStepDescription::new();
            workflow_step.description = params.update_value;

            workflow_step.workflow_id = params.workflow_id;
            workflow_step.id = params.id;

            authorize(&db_session, &workflow_step.node_id, current_user).await?;
            workflow_step.update_cb(&db_session).await?;

            Ok(HttpResponse::Ok().json(json!({
                "success": true,
                "workflow_step": workflow_step,
            })))
        }
        UpdateAttributes::InputIds => {
            let mut workflow_step = UpdateWorkflowStepInputIds::new();
            workflow_step.input_ids = Some(from_str(&params.update_value)?);

            workflow_step.workflow_id = params.workflow_id;
            workflow_step.id = params.id;

            authorize(&db_session, &workflow_step.node_id, current_user).await?;
            workflow_step.update_cb(&db_session).await?;

            Ok(HttpResponse::Ok().json(json!({
                "success": true,
                "workflow_step": workflow_step,
            })))
        }
        UpdateAttributes::OutputIds => {
            let mut workflow_step = UpdateWorkflowStepOutputIds::new();
            workflow_step.output_ids = Some(from_str(&params.update_value)?);

            workflow_step.workflow_id = params.workflow_id;
            workflow_step.id = params.id;

            authorize(&db_session, &workflow_step.node_id, current_user).await?;
            workflow_step.update_cb(&db_session).await?;

            Ok(HttpResponse::Ok().json(json!({
                "success": true,
                "workflow_step": workflow_step,
            })))
        }
    }
}

#[derive(Deserialize)]
pub struct DeleteParams {
    pub workflow_id: Uuid,
    pub id: Uuid,
}

#[delete("/{workflow_id}/{id}")]
pub async fn delete_workflow_step(
    db_session: web::Data<CachingSession>,
    current_user: CurrentUser,
    params: web::Path<DeleteParams>,
) -> Result<HttpResponse, NodecosmosError> {
    let params = params.into_inner();
    let mut workflow_step = WorkflowStep::new();
    workflow_step.workflow_id = params.workflow_id;
    workflow_step.id = params.id;

    let workflow_step = workflow_step.find_by_primary_key(&db_session).await?;
    authorize(&db_session, &workflow_step.node_id, current_user).await?;

    let mut workflow_step = WorkflowStep::new();
    workflow_step.workflow_id = params.workflow_id;
    workflow_step.id = params.id;

    workflow_step.delete_cb(&db_session).await?;

    Ok(HttpResponse::Ok().finish())
}

async fn authorize(
    db_session: &CachingSession,
    node_id: &Uuid,
    current_user: CurrentUser,
) -> Result<(), NodecosmosError> {
    let node = Node::find_one(&db_session, find_node_query!("id = ?"), (node_id,)).await?;

    auth_node_update(&node, &current_user).await?;

    Ok(())
}
