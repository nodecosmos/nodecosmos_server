use crate::api::authorization::{auth_node_access, auth_node_creation, auth_node_update};
use crate::api::request::current_user::OptCurrentUser;
use crate::api::request::data::RequestData;
use crate::api::types::Response;
use crate::models::node::search::{NodeSearch, NodeSearchQuery};
use crate::models::node::*;
use crate::models::node_descendant::NodeDescendant;
use actix_multipart::Multipart;
use actix_web::{delete, get, post, put, web, HttpResponse};
use charybdis::model::AsNative;
use charybdis::operations::{DeleteWithExtCallbacks, Find, InsertWithExtCallbacks, UpdateWithExtCallbacks};
use charybdis::types::Uuid;
use elasticsearch::Elasticsearch;
use scylla::CachingSession;
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
pub struct NodePrimaryKeyParams {
    pub root_id: Uuid,
    pub id: Uuid,
}

#[get("")]
pub async fn get_nodes(elastic_client: web::Data<Elasticsearch>, query: web::Query<NodeSearchQuery>) -> Response {
    let nodes = NodeSearch::new(&elastic_client, &query).index().await?;
    Ok(HttpResponse::Ok().json(nodes))
}

#[get("/{id}")]
pub async fn get_node(
    db_session: web::Data<CachingSession>,
    id: web::Path<Uuid>,
    opt_current_user: OptCurrentUser,
) -> Response {
    let node = BaseNode::find_by_id(&db_session, *id).await?;

    auth_node_access(&node, opt_current_user).await?;

    let descendants: Vec<NodeDescendant> = node.as_native().descendants(&db_session).await?.try_collect().await?;

    Ok(HttpResponse::Ok().json({
        json!({
            "success": true,
            "node": node,
            "descendants": descendants
        })
    }))
}

#[get("/{id}/description")]
pub async fn get_node_description(db_session: web::Data<CachingSession>, id: web::Path<Uuid>) -> Response {
    let node = GetDescriptionNode::find_by_id(&db_session, *id).await?;

    Ok(HttpResponse::Ok().json(json!({
        "success": true,
        "node": node
    })))
}

#[get("/{id}/description_base64")]
pub async fn get_node_description_base64(db_session: web::Data<CachingSession>, id: web::Path<Uuid>) -> Response {
    let node = GetDescriptionBase64Node::find_by_id(&db_session, *id).await?;

    Ok(HttpResponse::Ok().json(json!({
        "success": true,
        "node": node
    })))
}

#[post("")]
pub async fn create_node(mut node: web::Json<Node>, data: RequestData) -> Response {
    let db_session = &data.app.db_session;
    let current_user = &data.current_user;
    let resource_locker = &data.app.resource_locker;

    auth_node_creation(&db_session, &mut node, current_user).await?;

    resource_locker.check_node_lock(&node).await?;
    node.set_owner(current_user);

    node.insert_cb(db_session, &data).await?;

    Ok(HttpResponse::Ok().json(node))
}

#[put("/title")]
pub async fn update_node_title(mut node: web::Json<UpdateTitleNode>, data: RequestData) -> Response {
    let db_session = &data.app.db_session;
    let current_user = &data.current_user;
    let resource_locker = &data.app.resource_locker;

    let native_node = node.as_native().find_by_primary_key(&db_session).await?;

    auth_node_update(&native_node, current_user).await?;
    resource_locker.check_node_lock(&native_node).await?;
    node.update_cb(db_session, &data).await?;

    Ok(HttpResponse::Ok().json(node))
}

#[put("/description")]
pub async fn update_node_description(mut node: web::Json<UpdateDescriptionNode>, data: RequestData) -> Response {
    let db_session = &data.app.db_session;
    let current_user = &data.current_user;

    let native_node = node.as_native().find_by_primary_key(db_session).await?;

    auth_node_update(&native_node, current_user).await?;
    node.update_cb(db_session, &data).await?;

    Ok(HttpResponse::Ok().json(node))
}

#[delete("/{id}")]
pub async fn delete_node(id: web::Path<Uuid>, data: RequestData) -> Response {
    let db_session = &data.app.db_session;
    let current_user = &data.current_user;
    let resource_locker = &data.app.resource_locker;

    let mut node = Node::find_by_id(db_session, *id).await?;

    resource_locker.check_node_lock(&node).await?;

    auth_node_update(&node, current_user).await?;
    node.delete_cb(db_session, &data).await?;

    Ok(HttpResponse::Ok().finish())
}

#[derive(Deserialize)]
pub struct ReorderParams {
    #[serde(rename = "nodeId")]
    pub node_id: Uuid,

    #[serde(rename = "newParentId")]
    pub new_parent_id: Uuid,

    #[serde(rename = "newUpperSiblingId")]
    pub new_upper_sibling_id: Option<Uuid>,

    #[serde(rename = "newBottomSiblingId")]
    pub new_bottom_sibling_id: Option<Uuid>,
}

#[put("/reorder")]
pub async fn reorder_nodes(params: web::Json<ReorderParams>, data: RequestData) -> Response {
    let db_session = &data.app.db_session;
    let current_user = &data.current_user;

    let node = Node::find_by_id(db_session, params.node_id).await?;
    auth_node_update(&node, &current_user).await?;

    node.reorder(&data, params.into_inner()).await?;

    Ok(HttpResponse::Ok().finish())
}

#[post("/{id}/upload_cover_image")]
async fn upload_cover_image(id: web::Path<Uuid>, data: RequestData, payload: Multipart) -> Response {
    let db_session = &data.app.db_session;
    let current_user = &data.current_user;

    let node = Node::find_by_id(db_session, *id).await?;

    auth_node_update(&node, current_user).await?;

    UpdateCoverImageNode::from_native(&node)
        .update_cover_image(payload, &data)
        .await?;

    Ok(HttpResponse::Ok().json(json!({
        "coverImageUrl": node.cover_image_url
    })))
}

#[delete("/{id}/delete_cover_image")]
async fn delete_cover_image(id: web::Path<Uuid>, data: RequestData) -> Response {
    let db_session = &data.app.db_session;
    let current_user = &data.current_user;

    let node = Node::find_by_id(db_session, *id).await?;

    auth_node_update(&node, &current_user).await?;

    UpdateCoverImageNode::from_native(&node)
        .delete_cover_image(&data)
        .await?;

    Ok(HttpResponse::Ok().finish())
}
