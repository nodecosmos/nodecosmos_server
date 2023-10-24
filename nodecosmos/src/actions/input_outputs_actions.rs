use crate::authorize::{auth_input_output_creation, auth_workflow_update};
use crate::errors::NodecosmosError;
use crate::models::input_output::{InputOutput, UpdateDescriptionInputOutput, UpdateTitleInputOutput};
use crate::models::user::CurrentUser;

use actix_web::{delete, get, post, put, web, HttpResponse};
use charybdis::operations::{DeleteWithCallbacks, Find, InsertWithCallbacks, UpdateWithCallbacks};
use scylla::CachingSession;
use serde_json::json;

#[post("")]
pub async fn create_io(
    db_session: web::Data<CachingSession>,
    current_user: CurrentUser,
    mut input_output: web::Json<InputOutput>,
) -> Result<HttpResponse, NodecosmosError> {
    auth_input_output_creation(&db_session, &input_output, &current_user).await?;

    input_output.insert_cb(&db_session).await?;

    Ok(HttpResponse::Ok().json(json!({
        "inputOutput": input_output,
    })))
}

#[get("/{rootNodeId}/{nodeId}/{id}/description")]
pub async fn get_io_description(
    db_session: web::Data<CachingSession>,
    input_output: web::Path<UpdateDescriptionInputOutput>,
) -> Result<HttpResponse, NodecosmosError> {
    let input_output = input_output.find_by_primary_key(&db_session).await?;

    Ok(HttpResponse::Ok().json(json!({
        "inputOutput": input_output,
    })))
}

#[put("/title")]
pub async fn update_io_title(
    db_session: web::Data<CachingSession>,
    current_user: CurrentUser,
    input_output: web::Json<UpdateTitleInputOutput>,
) -> Result<HttpResponse, NodecosmosError> {
    let mut input_output = input_output.find_by_primary_key(&db_session).await?;

    auth_workflow_update(&db_session, input_output.node_id, current_user).await?;

    input_output.update_cb(&db_session).await?;

    Ok(HttpResponse::Ok().json(json!({
        "inputOutput": input_output,
    })))
}

#[put("/description")]
pub async fn update_io_description(
    db_session: web::Data<CachingSession>,
    current_user: CurrentUser,
    input_output: web::Json<UpdateDescriptionInputOutput>,
) -> Result<HttpResponse, NodecosmosError> {
    let mut input_output = input_output.find_by_primary_key(&db_session).await?;

    auth_workflow_update(&db_session, input_output.node_id, current_user).await?;

    input_output.update_cb(&db_session).await?;

    Ok(HttpResponse::Ok().json(json!({
        "inputOutput": input_output,
    })))
}

#[delete("/{rootNodeId}/{nodeId}/{workflowId}/{id}")]
pub async fn delete_io(
    db_session: web::Data<CachingSession>,
    input_output: web::Path<InputOutput>,
    current_user: CurrentUser,
) -> Result<HttpResponse, NodecosmosError> {
    let mut input_output = input_output.find_by_primary_key(&db_session).await?;

    auth_workflow_update(&db_session, input_output.node_id, current_user).await?;

    input_output.delete_cb(&db_session).await?;

    Ok(HttpResponse::Ok().json(json!({
        "inputOutput": input_output,
    })))
}
