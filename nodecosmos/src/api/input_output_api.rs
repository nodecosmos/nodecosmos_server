use actix_web::{delete, post, put, web, HttpResponse};
use charybdis::operations::{DeleteWithCallbacks, InsertWithCallbacks, UpdateWithCallbacks};
use serde::Deserialize;

use crate::api::data::RequestData;
use crate::api::types::Response;
use crate::models::io::{BaseIo, Io, UpdateTitleIo};
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

#[derive(Deserialize)]
pub struct DeleteDanglingQ {
    #[serde(default, rename = "deleteDangling")]
    delete_dangling: Option<bool>,
}

#[delete("/{rootId}/{nodeId}/{branchId}/{id}")]
pub async fn delete_io(data: RequestData, io: web::Path<BaseIo>, query: web::Query<DeleteDanglingQ>) -> Response {
    AuthNode::auth_update(&data, io.branch_id, io.node_id, io.root_id).await?;

    let mut io = Io::find_branched_or_original(data.db_session(), io.root_id, io.branch_id, io.id).await?;

    io.delete_dangling = query.delete_dangling;
    io.delete_cb(&data).execute(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(io))
}
