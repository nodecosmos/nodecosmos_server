use crate::actions::client_session::*;
use crate::app::CbExtension;
use crate::authorize::{auth_node_access, auth_node_creation, auth_node_update};
use crate::errors::NodecosmosError;
use crate::models::node::*;
use crate::models::node_descendant::NodeDescendant;
use crate::models::udts::{Owner, OwnerTypes};
use crate::services::aws::s3::delete_s3_object;
use crate::services::nodes::cover_image_uploader::handle_cover_image_upload;
use crate::services::nodes::reorder::{ReorderParams, Reorderer};
use crate::services::nodes::search::{NodeSearchQuery, NodeSearchService};
use crate::services::resource_locker::ResourceLocker;
use actix_multipart::Multipart;
use actix_web::{delete, get, post, put, web, HttpResponse};
use charybdis::{
    AsNative, DeleteWithExtCallbacks, Deserialize, Find, InsertWithExtCallbacks, New,
    UpdateWithExtCallbacks, Uuid,
};
use elasticsearch::Elasticsearch;
use scylla::CachingSession;
use serde_json::json;

#[derive(Debug, Deserialize)]
pub struct NodePrimaryKeyParams {
    pub root_id: Uuid,
    pub id: Uuid,
}

#[get("")]
pub async fn get_nodes(
    elastic_client: web::Data<Elasticsearch>,
    query: web::Query<NodeSearchQuery>,
) -> Result<HttpResponse, NodecosmosError> {
    let nodes = NodeSearchService::new(&elastic_client, &query)
        .index()
        .await?;
    Ok(HttpResponse::Ok().json(nodes))
}

#[get("/{id}")]
pub async fn get_node(
    db_session: web::Data<CachingSession>,
    id: web::Path<Uuid>,
    opt_current_user: OptCurrentUser,
) -> Result<HttpResponse, NodecosmosError> {
    let mut node = BaseNode::new();
    node.id = *id;

    let node = node.find_by_primary_key(&db_session).await?;

    auth_node_access(&node, opt_current_user).await?;

    let descendants = NodeDescendant {
        root_id: node.id,
        ..Default::default()
    }
    .find_by_partition_key(&db_session)
    .await?;

    Ok(HttpResponse::Ok().json({
        json!({
            "success": true,
            "node": node,
            "descendants": descendants
        })
    }))
}

#[get("/{id}/description")]
pub async fn get_node_description(
    db_session: web::Data<CachingSession>,
    id: web::Path<Uuid>,
) -> Result<HttpResponse, NodecosmosError> {
    let mut node = GetNodeDescription::new();
    node.id = *id;

    let node = node.find_by_primary_key(&db_session).await?;

    Ok(HttpResponse::Ok().json(json!({
        "success": true,
        "node": node
    })))
}

#[get("/{id}/description_base64")]
pub async fn get_node_description_base64(
    db_session: web::Data<CachingSession>,
    id: web::Path<Uuid>,
) -> Result<HttpResponse, NodecosmosError> {
    let mut node = GetNodedescriptionBase64::new();
    node.id = *id;

    let node = node.find_by_primary_key(&db_session).await?;

    Ok(HttpResponse::Ok().json(json!({
        "success": true,
        "node": node
    })))
}

#[post("")]
pub async fn create_node(
    db_session: web::Data<CachingSession>,
    cb_extension: web::Data<CbExtension>,
    mut node: web::Json<Node>,
    current_user: CurrentUser,
    resource_locker: web::Data<ResourceLocker>,
) -> Result<HttpResponse, NodecosmosError> {
    let parent = node.parent(&db_session).await?;

    resource_locker.check_node_lock(&node).await?;
    auth_node_creation(&parent, &current_user).await?;

    node.set_owner(Owner {
        id: current_user.id,
        name: current_user.full_name(),
        username: Some(current_user.username),
        owner_type: OwnerTypes::User.into(),
        profile_image_url: None,
    });

    if let Some(parent) = parent {
        node.editor_ids = parent.editor_ids;
        node.is_public = parent.is_public;

        let mut ancestor_ids = parent.ancestor_ids.unwrap_or_default();
        ancestor_ids.push(parent.id);

        node.ancestor_ids = Some(ancestor_ids);
    }

    node.insert_cb(&db_session, &cb_extension).await?;

    Ok(HttpResponse::Ok().json(node))
}

#[put("/title")]
pub async fn update_node_title(
    mut node: web::Json<UpdateNodeTitle>,
    db_session: web::Data<CachingSession>,
    cb_extension: web::Data<CbExtension>,
    current_user: CurrentUser,
) -> Result<HttpResponse, NodecosmosError> {
    let native_node = node.as_native().find_by_primary_key(&db_session).await?;

    auth_node_update(&native_node, &current_user).await?;

    node.order_index = native_node.order_index;
    node.ancestor_ids = native_node.ancestor_ids;

    node.update_cb(&db_session, &cb_extension).await?;

    Ok(HttpResponse::Ok().json(node))
}

#[put("/description")]
pub async fn update_node_description(
    db_session: web::Data<CachingSession>,
    mut node: web::Json<UpdateNodeDescription>,
    cb_extension: web::Data<CbExtension>,
    current_user: CurrentUser,
) -> Result<HttpResponse, NodecosmosError> {
    let native_node = node.as_native().find_by_primary_key(&db_session).await?;

    auth_node_update(&native_node, &current_user).await?;
    node.update_cb(&db_session, &cb_extension).await?;

    Ok(HttpResponse::Ok().json(node))
}

#[delete("/{id}")]
pub async fn delete_node(
    id: web::Path<Uuid>,
    db_session: web::Data<CachingSession>,
    cb_extension: web::Data<CbExtension>,
    current_user: CurrentUser,
    resource_locker: web::Data<ResourceLocker>,
) -> Result<HttpResponse, NodecosmosError> {
    let mut node = Node::new();
    node.id = *id;

    resource_locker.check_node_lock(&node).await?;

    let mut node = node.find_by_primary_key(&db_session).await?;

    auth_node_update(&node, &current_user).await?;
    node.delete_cb(&db_session, &cb_extension).await?;

    Ok(HttpResponse::Ok().finish())
}

#[put("/reorder")]
pub async fn reorder_nodes(
    db_session: web::Data<CachingSession>,
    params: web::Json<ReorderParams>,
    current_user: CurrentUser,
    resource_locker: web::Data<ResourceLocker>,
) -> Result<HttpResponse, NodecosmosError> {
    let mut reorderer = Reorderer::new(db_session, params.into_inner()).await?;

    resource_locker.check_node_lock(&reorderer.node).await?;
    auth_node_update(&reorderer.node, &current_user).await?;
    reorderer.reorder(&resource_locker).await?;

    Ok(HttpResponse::Ok().finish())
}

#[post("/{id}/upload_cover_image")]
async fn upload_cover_image(
    id: web::Path<Uuid>,
    db_session: web::Data<CachingSession>,
    cb_extension: web::Data<CbExtension>,
    s3_client: web::Data<aws_sdk_s3::Client>,
    nc_app: web::Data<crate::NodecosmosApp>,
    current_user: CurrentUser,
    payload: Multipart,
) -> Result<HttpResponse, NodecosmosError> {
    let mut node = Node::new();
    node.id = *id;

    let node = node.find_by_primary_key(&db_session).await?;
    auth_node_update(&node, &current_user).await?;

    let image_url = handle_cover_image_upload(
        payload,
        &s3_client,
        &nc_app,
        node,
        &db_session,
        &cb_extension,
    )
    .await?;

    Ok(HttpResponse::Ok().json(json!({
        "success": true,
        "coverImageUrl": image_url
    })))
}

#[delete("/{id}/delete_cover_image")]
async fn delete_cover_image(
    id: web::Path<Uuid>,
    db_session: web::Data<CachingSession>,
    cb_extension: web::Data<CbExtension>,
    s3_client: web::Data<aws_sdk_s3::Client>,
    nc_app: web::Data<crate::NodecosmosApp>,
    current_user: CurrentUser,
) -> Result<HttpResponse, NodecosmosError> {
    let mut node = UpdateNodeCoverImage::new();
    node.id = *id;

    let mut node = node.find_by_primary_key(&db_session).await?;
    let native_node = node.as_native().find_by_primary_key(&db_session).await?;

    auth_node_update(&native_node, &current_user).await?;

    if node.cover_image_url.is_some() {
        let key = node.cover_image_filename.clone().ok_or_else(|| {
            NodecosmosError::InternalServerError("Missing cover image key".to_string())
        })?;

        println!("Deleting cover image from S3: {}", key);

        let bucket = nc_app.bucket.clone();
        let key = key.clone();
        let s3_client = s3_client.clone();

        tokio::spawn(async move {
            let _ = delete_s3_object(&s3_client, &bucket, &key)
                .await
                .map_err(|e| {
                    println!("Failed to delete cover image from S3: {:?}", e);
                });
        });
    }

    node.cover_image_url = None;
    node.cover_image_filename = None;

    node.update_cb(&db_session, &cb_extension).await?;

    Ok(HttpResponse::Ok().finish())
}
