use crate::api::authorization::{auth_node_access, auth_node_creation, auth_node_update};
use crate::api::request::current_user::OptCurrentUser;
use crate::api::request::data::RequestData;
use crate::api::types::{ActionObject, ActionTypes, Response};
use crate::errors::NodecosmosError;
use crate::models::node::cover_image_uploader::CoverImageUploader;
use crate::models::node::reorder::{Reorder, ReorderParams};
use crate::models::node::search::{NodeSearch, NodeSearchQuery};
use crate::models::node::*;
use crate::models::node_descendant::NodeDescendant;
use crate::services::aws::s3::delete_s3_object;
use crate::utils::logger::log_error;
use actix_multipart::Multipart;
use actix_web::{delete, get, post, put, web, HttpResponse};
use charybdis::model::AsNative;
use charybdis::operations::{DeleteWithExtCallbacks, Find, InsertWithExtCallbacks, New, UpdateWithExtCallbacks};
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
    let mut node = BaseNode::new();
    node.id = *id;

    let node = node.find_by_primary_key(&db_session).await?;

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
    let node = GetDescriptionNode::find_by_primary_key_value(&db_session, (*id,)).await?;

    Ok(HttpResponse::Ok().json(json!({
        "success": true,
        "node": node
    })))
}

#[get("/{id}/description_base64")]
pub async fn get_node_description_base64(db_session: web::Data<CachingSession>, id: web::Path<Uuid>) -> Response {
    let node = GetDescriptionBase64Node::find_by_primary_key_value(&db_session, (*id,)).await?;

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
pub async fn update_node_title(mut utn: web::Json<UpdateTitleNode>, data: RequestData) -> Response {
    let db_session = &data.app.db_session;
    let current_user = &data.current_user;
    let resource_locker = &data.app.resource_locker;

    let native_node = utn.as_native().find_by_primary_key(&db_session).await?;

    auth_node_update(&native_node, current_user).await?;
    resource_locker.check_node_lock(&native_node).await?;
    utn.update_cb(db_session, &data).await?;

    Ok(HttpResponse::Ok().json(utn))
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

    let mut node = Node::new();
    node.id = *id;

    resource_locker.check_node_lock(&node).await?;

    let mut node = node.find_by_primary_key(db_session).await?;

    auth_node_update(&node, current_user).await?;
    node.delete_cb(db_session, &data).await?;

    Ok(HttpResponse::Ok().finish())
}

#[put("/reorder")]
pub async fn reorder_nodes(params: web::Json<ReorderParams>, data: RequestData) -> Response {
    let db_session = &data.app.db_session;
    let current_user = &data.current_user;
    let resource_locker = &data.app.resource_locker;

    let mut node = Node::new();
    node.id = params.node_id;

    let node = node.find_by_primary_key(db_session).await?;
    auth_node_update(&node, &current_user).await?;

    resource_locker.check_node_lock(&node).await?;
    resource_locker
        .check_node_action_lock(ActionTypes::Reorder(ActionObject::Node), &node)
        .await?;

    let mut reorder = Reorder::new(db_session, params.into_inner()).await?;

    reorder.run(resource_locker).await?;

    Ok(HttpResponse::Ok().finish())
}

#[post("/{id}/upload_cover_image")]
async fn upload_cover_image(id: web::Path<Uuid>, data: RequestData, payload: Multipart) -> Response {
    let db_session = &data.app.db_session;
    let current_user = &data.current_user;

    let mut node = Node::new();
    node.id = *id;

    let node = node.find_by_primary_key(db_session).await?;
    auth_node_update(&node, current_user).await?;

    let mut uploader = CoverImageUploader {
        data: &data,
        payload,
        node: &node,
    };

    let image_url = uploader.run().await?;

    Ok(HttpResponse::Ok().json(json!({
        "coverImageUrl": image_url
    })))
}

#[delete("/{id}/delete_cover_image")]
async fn delete_cover_image(id: web::Path<Uuid>, data: RequestData) -> Response {
    let db_session = &data.app.db_session;
    let current_user = &data.current_user;

    let mut node = UpdateCoverImageNode::new();
    node.id = *id;

    let native_node = node.as_native().find_by_primary_key(db_session).await?;

    auth_node_update(&native_node, &current_user).await?;

    if native_node.cover_image_url.is_some() {
        let key = native_node
            .cover_image_filename
            .clone()
            .ok_or_else(|| NodecosmosError::InternalServerError("Missing cover image key".to_string()))?;

        let bucket = data.app.s3_bucket.clone();
        let key = key.clone();
        let s3_client = data.app.s3_client.clone();

        tokio::spawn(async move {
            let _ = delete_s3_object(&s3_client, &bucket, &key)
                .await
                .map_err(|e| log_error(format!("Failed to delete cover image from S3: {:?}", e)));
        });
    }

    node.cover_image_url = None;
    node.cover_image_filename = None;

    node.update_cb(db_session, &data).await?;

    Ok(HttpResponse::Ok().finish())
}
