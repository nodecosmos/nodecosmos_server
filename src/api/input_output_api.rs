use crate::api::authorization::Authorization;
use crate::api::data::RequestData;
use crate::api::types::Response;
use crate::models::input_output::{DeleteIo, GetDescriptionIo, Io, UpdateDescriptionIo, UpdateTitleIo};
use crate::models::node::AuthNode;
use actix_web::{delete, get, post, put, web, HttpResponse};
use charybdis::operations::{DeleteWithCallbacks, Find, InsertWithCallbacks, UpdateWithCallbacks};

#[post("")]
pub async fn create_io(data: RequestData, mut input_output: web::Json<Io>) -> Response {
    let node = input_output.node(data.db_session()).await?;

    node.auth_update(&data).await?;

    input_output.insert_cb(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(input_output))
}

#[get("/{rootNodeId}/{nodeId}/{workflowId}/{id}/description")]
pub async fn get_io_description(data: RequestData, input_output: web::Path<GetDescriptionIo>) -> Response {
    let input_output = input_output.find_by_primary_key(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(input_output))
}

#[put("/title")]
pub async fn update_io_title(data: RequestData, mut input_output: web::Json<UpdateTitleIo>) -> Response {
    AuthNode::auth_update(&data, input_output.node_id, input_output.node_id).await?;

    input_output.update_cb(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(input_output))
}

#[put("/description")]
pub async fn update_io_description(data: RequestData, mut input_output: web::Json<UpdateDescriptionIo>) -> Response {
    AuthNode::auth_update(&data, input_output.node_id, input_output.node_id).await?;

    input_output.update_cb(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(input_output))
}

#[delete("/{rootNodeId}/{nodeId}/{workflowId}/{id}")]
pub async fn delete_io(data: RequestData, input_output: web::Path<DeleteIo>) -> Response {
    let mut input_output = input_output.find_by_primary_key(data.db_session()).await?;

    AuthNode::auth_update(&data, input_output.node_id, input_output.node_id).await?;

    input_output.delete_cb(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(input_output))
}
