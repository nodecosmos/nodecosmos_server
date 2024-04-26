use actix_web::{delete, post, put, web, HttpResponse};
use charybdis::operations::{DeleteWithCallbacks, InsertWithCallbacks, UpdateWithCallbacks};

use crate::api::data::RequestData;
use crate::api::types::Response;
use crate::models::io::{Io, UpdateTitleIo};
use crate::models::node::AuthNode;
use crate::models::traits::Branchable;

#[post("")]
pub async fn create_io(data: RequestData, mut input_output: web::Json<Io>) -> Response {
    if input_output.is_original() {
        AuthNode::auth_update(&data, input_output.node_id, input_output.node_id).await?;
    } else {
        AuthNode::auth_update(&data, input_output.node_id, input_output.branch_id).await?;
    }

    input_output.insert_cb(&data).execute(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(input_output))
}

#[put("/title")]
pub async fn update_io_title(data: RequestData, mut input_output: web::Json<UpdateTitleIo>) -> Response {
    if input_output.is_original() {
        AuthNode::auth_update(&data, input_output.node_id, input_output.node_id).await?;
    } else {
        AuthNode::auth_update(&data, input_output.node_id, input_output.branch_id).await?;
    }

    input_output.update_cb(&data).execute(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(input_output))
}

#[delete("/{rootId}/{nodeId}/{branchId}/{id}")]
pub async fn delete_io(data: RequestData, mut input_output: web::Path<Io>) -> Response {
    if input_output.is_original() {
        AuthNode::auth_update(&data, input_output.node_id, input_output.node_id).await?;
    } else {
        AuthNode::auth_update(&data, input_output.node_id, input_output.branch_id).await?;
    }

    input_output.delete_cb(&data).execute(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(input_output.into_inner()))
}
