use crate::api::authorization::{auth_input_output_creation, auth_workflow_update};
use crate::models::input_output::{DeleteIo, Io, UpdateDescriptionIo, UpdateTitleIo};
use crate::models::user::CurrentUser;

use crate::api::types::Response;
use actix_web::{delete, get, post, put, web, HttpResponse};
use charybdis::operations::{DeleteWithCallbacks, Find, InsertWithCallbacks, UpdateWithCallbacks};
use scylla::CachingSession;

#[post("")]
pub async fn create_io(
    db_session: web::Data<CachingSession>,
    current_user: CurrentUser,
    mut input_output: web::Json<Io>,
) -> Response {
    auth_input_output_creation(&db_session, &input_output, &current_user).await?;

    input_output.insert_cb(&db_session).await?;

    Ok(HttpResponse::Ok().json(input_output))
}

#[get("/{rootNodeId}/{nodeId}/{workflowId}/{id}/description")]
pub async fn get_io_description(
    db_session: web::Data<CachingSession>,
    input_output: web::Path<UpdateDescriptionIo>,
) -> Response {
    let input_output = input_output.find_by_primary_key(&db_session).await?;

    Ok(HttpResponse::Ok().json(input_output))
}

#[put("/title")]
pub async fn update_io_title(
    db_session: web::Data<CachingSession>,
    current_user: CurrentUser,
    mut input_output: web::Json<UpdateTitleIo>,
) -> Response {
    auth_workflow_update(&db_session, input_output.node_id, current_user).await?;

    input_output.update_cb(&db_session).await?;

    Ok(HttpResponse::Ok().json(input_output))
}

#[put("/description")]
pub async fn update_io_description(
    db_session: web::Data<CachingSession>,
    current_user: CurrentUser,
    mut input_output: web::Json<UpdateDescriptionIo>,
) -> Response {
    auth_workflow_update(&db_session, input_output.node_id, current_user).await?;

    input_output.update_cb(&db_session).await?;

    Ok(HttpResponse::Ok().json(input_output))
}

#[delete("/{rootNodeId}/{nodeId}/{workflowId}/{id}")]
pub async fn delete_io(
    db_session: web::Data<CachingSession>,
    input_output: web::Path<DeleteIo>,
    current_user: CurrentUser,
) -> Response {
    let mut input_output = input_output.find_by_primary_key(&db_session).await?;

    auth_workflow_update(&db_session, input_output.node_id, current_user).await?;

    input_output.delete_cb(&db_session).await?;

    Ok(HttpResponse::Ok().json(input_output))
}
