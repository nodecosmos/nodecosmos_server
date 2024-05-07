use actix_web::{delete, post, put, web, HttpResponse};
use charybdis::operations::{DeleteWithCallbacks, InsertWithCallbacks, UpdateWithCallbacks};

use crate::api::data::RequestData;
use crate::api::types::Response;
use crate::models::io::{Io, UpdateTitleIo};
use crate::models::node::AuthNode;

#[post("")]
pub async fn create_io(data: RequestData, mut io: web::Json<Io>) -> Response {
    AuthNode::auth_update(&data, io.branch_id, io.node_id, io.root_id).await?;

    io.insert_cb(&data).execute(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(io))
}

#[put("/title")]
pub async fn update_io_title(data: RequestData, mut io: web::Json<UpdateTitleIo>) -> Response {
    AuthNode::auth_update(&data, io.branch_id, io.node_id, io.root_id).await?;

    io.update_cb(&data).execute(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(io))
}

#[delete("/{rootId}/{nodeId}/{branchId}/{id}")]
pub async fn delete_io(data: RequestData, mut io: web::Path<Io>) -> Response {
    AuthNode::auth_update(&data, io.branch_id, io.node_id, io.root_id).await?;

    io.delete_cb(&data).execute(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(io.into_inner()))
}
