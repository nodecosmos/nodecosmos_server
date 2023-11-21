use crate::api::authorization::{auth_node_access, auth_node_creation, auth_node_update};
use crate::api::request::current_user::OptCurrentUser;
use crate::api::request::data::RequestData;
use crate::api::types::Response;
use crate::models::node::reorder::ReorderParams;
use crate::models::node::search::{NodeSearch, NodeSearchQuery};
use crate::models::node::*;
use actix_multipart::Multipart;
use actix_web::{delete, get, post, put, web, HttpResponse};
use charybdis::model::AsNative;
use charybdis::operations::{DeleteWithExtCallbacks, Find, InsertWithExtCallbacks, UpdateWithExtCallbacks};
use charybdis::types::Uuid;
use elasticsearch::Elasticsearch;
use scylla::CachingSession;
use serde_json::json;

#[get("")]
pub async fn get_nodes(elastic_client: web::Data<Elasticsearch>, query: web::Query<NodeSearchQuery>) -> Response {
    let nodes = NodeSearch::new(&elastic_client, &query).index().await?;
    Ok(HttpResponse::Ok().json(nodes))
}

#[get("/{id}")]
pub async fn get_node(db_session: web::Data<CachingSession>, id: web::Path<Uuid>, opt_cu: OptCurrentUser) -> Response {
    let node = BaseNode::find_by_id_and_branch_id(&db_session, *id, *id).await?;

    auth_node_access(&node, opt_cu).await?;

    let descendants = node.as_native().descendants(&db_session).await?.try_collect().await?;

    Ok(HttpResponse::Ok().json({
        json!({
            "success": true,
            "node": node,
            "descendants": descendants
        })
    }))
}

#[get("/{id}/{branchId}")]
pub async fn get_branched_node(
    db_session: web::Data<CachingSession>,
    node: web::Path<BaseNode>,
    opt_cu: OptCurrentUser,
) -> Response {
    let node = node.find_by_primary_key(&db_session).await?;

    auth_node_access(&node, opt_cu).await?;

    let descendants = node.as_native().branch_descendants(&db_session).await?;

    Ok(HttpResponse::Ok().json(json!({
        "success": true,
        "node": node,
        "descendants": descendants
    })))
}

#[get("/{id}/description")]
pub async fn get_node_description(db_session: web::Data<CachingSession>, id: web::Path<Uuid>) -> Response {
    let node = GetDescriptionNode::find_by_id_and_branch_id(&db_session, *id, *id).await?;

    Ok(HttpResponse::Ok().json(json!({
        "success": true,
        "node": node
    })))
}

#[get("/{id}/description_base64")]
pub async fn get_node_description_base64(db_session: web::Data<CachingSession>, id: web::Path<Uuid>) -> Response {
    let node = GetDescriptionBase64Node::find_by_id_and_branch_id(&db_session, *id, *id).await?;

    Ok(HttpResponse::Ok().json(json!({
        "success": true,
        "node": node
    })))
}

#[post("")]
pub async fn create_node(mut node: web::Json<Node>, data: RequestData) -> Response {
    auth_node_creation(data.db_session(), &mut node, &data.current_user).await?;

    data.resource_locker().check_node_lock(&node).await?;

    node.insert_cb(data.db_session(), &data).await?;

    Ok(HttpResponse::Ok().json(node))
}

#[put("/title")]
pub async fn update_node_title(mut node: web::Json<UpdateTitleNode>, data: RequestData) -> Response {
    let native_node = node.as_native().find_by_primary_key(data.db_session()).await?;

    auth_node_update(&native_node, &data.current_user).await?;

    data.resource_locker().check_node_lock(&native_node).await?;

    node.update_cb(data.db_session(), &data).await?;

    Ok(HttpResponse::Ok().json(node))
}

#[put("/description")]
pub async fn update_node_description(mut node: web::Json<UpdateDescriptionNode>, data: RequestData) -> Response {
    let native_node = node.as_native().find_by_primary_key(data.db_session()).await?;

    auth_node_update(&native_node, &data.current_user).await?;

    node.update_cb(data.db_session(), &data).await?;

    Ok(HttpResponse::Ok().json(node))
}

#[delete("/{id}/{branchId}")]
pub async fn delete_node(node: web::Path<DeleteNode>, data: RequestData) -> Response {
    let mut node = node.as_native().find_by_primary_key(data.db_session()).await?;

    auth_node_update(&node, &data.current_user).await?;

    data.resource_locker().check_node_lock(&node).await?;

    node.delete_cb(data.db_session(), &data).await?;

    Ok(HttpResponse::Ok().finish())
}

#[put("/reorder")]
pub async fn reorder_nodes(params: web::Json<ReorderParams>, data: RequestData) -> Response {
    let node = Node::find_by_id_and_branch_id(data.db_session(), params.node_id, params.branch_id).await?;

    auth_node_update(&node, &data.current_user).await?;

    node.reorder(&data, params.into_inner()).await?;

    Ok(HttpResponse::Ok().finish())
}

#[post("/{id}/upload_cover_image")]
async fn upload_cover_image(id: web::Path<Uuid>, data: RequestData, payload: Multipart) -> Response {
    let node = Node::find_by_id_and_branch_id(data.db_session(), *id, *id).await?;

    auth_node_update(&node, &data.current_user).await?;

    UpdateCoverImageNode::from_native(&node)
        .update_cover_image(payload, &data)
        .await?;

    Ok(HttpResponse::Ok().json(json!({
        "coverImageUrl": node.cover_image_url
    })))
}

#[delete("/{id}/delete_cover_image")]
async fn delete_cover_image(id: web::Path<Uuid>, data: RequestData) -> Response {
    let node = Node::find_by_id_and_branch_id(data.db_session(), *id, *id).await?;

    auth_node_update(&node, &data.current_user).await?;

    UpdateCoverImageNode::from_native(&node)
        .delete_cover_image(&data)
        .await?;

    Ok(HttpResponse::Ok().finish())
}
