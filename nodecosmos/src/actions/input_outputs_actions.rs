use crate::errors::NodecosmosError;
use crate::models::input_output::{InputOutput, UpdateDescriptionInputOutput, UpdateTitleInputOutput};
use crate::models::user::CurrentUser;

use crate::authorize::auth_workflow_update;
use actix_web::{delete, get, post, put, web, HttpResponse};
use charybdis::operations::{DeleteWithCallbacks, Find, InsertWithCallbacks, New, UpdateWithCallbacks};
use charybdis::types::Uuid;
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
pub async fn create_io(
    db_session: web::Data<CachingSession>,
    current_user: CurrentUser,
    mut input_output: web::Json<InputOutput>,
) -> Result<HttpResponse, NodecosmosError> {
    auth_workflow_update(&db_session, input_output.node_id, current_user).await?;

    input_output.insert_cb(&db_session).await?;

    Ok(HttpResponse::Ok().json(json!({
        "success": true,
        "inputOutput": input_output,
    })))
}

#[get("/{nodeId}/{workflowId}/{id}/description")]
pub async fn get_io_description(
    db_session: web::Data<CachingSession>,
    input_output: web::Path<UpdateDescriptionInputOutput>,
) -> Result<HttpResponse, NodecosmosError> {
    let input_output = input_output.find_by_primary_key(&db_session).await?;

    Ok(HttpResponse::Ok().json(json!({
        "success": true,
        "inputOutput": input_output,
    })))
}

#[put("/title")]
pub async fn update_io_title(
    db_session: web::Data<CachingSession>,
    current_user: CurrentUser,
    input_output: web::Json<UpdateTitleInputOutput>,
) -> Result<HttpResponse, NodecosmosError> {
    let mut input_output = input_output.into_inner();

    auth_workflow_update(&db_session, input_output.node_id, current_user).await?;

    input_output.update_cb(&db_session).await?;

    Ok(HttpResponse::Ok().json(json!({
        "success": true,
        "inputOutput": input_output,
    })))
}

#[put("/description")]
pub async fn update_io_description(
    db_session: web::Data<CachingSession>,
    current_user: CurrentUser,
    mut input_output: web::Json<UpdateDescriptionInputOutput>,
) -> Result<HttpResponse, NodecosmosError> {
    auth_workflow_update(&db_session, input_output.node_id, current_user).await?;

    input_output.update_cb(&db_session).await?;

    Ok(HttpResponse::Ok().json(json!({
        "success": true,
        "inputOutput": input_output,
    })))
}

#[delete("/{nodeId}/{workflowId}/{id}")]
pub async fn delete_io(
    db_session: web::Data<CachingSession>,
    input_output: web::Path<InputOutput>,
    current_user: CurrentUser,
) -> Result<HttpResponse, NodecosmosError> {
    let mut input_output = input_output.find_by_primary_key(&db_session).await?;

    auth_workflow_update(&db_session, input_output.node_id, current_user).await?;

    input_output.delete_cb(&db_session).await?;

    Ok(HttpResponse::Ok().json(json!({
        "success": true,
        "inputOutput": input_output,
    })))
}
