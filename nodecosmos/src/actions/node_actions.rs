use crate::actions::client_session::*;
use crate::app::CbExtension;
use crate::authorize::{auth_node_access, auth_node_creation, auth_node_update};
use crate::errors::NodecosmosError;
use crate::models::node::*;
use crate::models::udts::{Owner, OwnerTypes};
use crate::services::nodes::reorder::{ReorderParams, Reorderer};
use crate::services::nodes::search::{NodeSearchQuery, NodeSearchService};
use crate::services::resource_locker::ResourceLocker;
use actix_web::{delete, get, post, put, web, HttpResponse};
use charybdis::{
    AsNative, DeleteWithExtCallbacks, Deserialize, Find, InsertWithExtCallbacks, New,
    UpdateWithExtCallbacks, Uuid,
};
use elasticsearch::Elasticsearch;
use scylla::CachingSession;
use serde_json::json;

#[derive(Debug, Deserialize)]
pub struct PrimaryKeyParams {
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

#[get("/{root_id}")]
pub async fn get_root_node(
    db_session: web::Data<CachingSession>,
    root_id: web::Path<Uuid>,
    opt_current_user: OptCurrentUser,
) -> Result<HttpResponse, NodecosmosError> {
    let mut node = BaseNode::new();
    node.root_id = root_id.into_inner();

    let nodes_iter = node.find_by_partition_key(&db_session).await?;

    let nodes: Vec<BaseNode> = nodes_iter.flatten().collect();

    if nodes.is_empty() {
        return Ok(HttpResponse::NotFound().json(json!({
            "success": false,
            "message": "Root node not found"
        })));
    }

    auth_node_access(&nodes[0], opt_current_user).await?;

    Ok(HttpResponse::Ok().json(nodes))
}

#[get("/{root_id}/{id}")]
pub async fn get_node(
    db_session: web::Data<CachingSession>,
    params: web::Path<PrimaryKeyParams>,
    opt_current_user: OptCurrentUser,
) -> Result<HttpResponse, NodecosmosError> {
    let params = params.into_inner();
    let mut node = BaseNode::new();

    node.root_id = params.root_id;
    node.id = params.id;

    let node = node.find_by_primary_key(&db_session).await?;

    auth_node_access(&node, opt_current_user).await?;

    let mut all_node_ids = node.descendant_ids.clone().unwrap_or_default();
    all_node_ids.push(node.id);

    let all_node_ids_chunks = all_node_ids.chunks(100).into_iter();

    let descendants_q = find_base_node_query!("root_id = ? AND id IN ?");
    let mut nodes = vec![];

    for node_ids_chunk in all_node_ids_chunks {
        let descendants =
            BaseNode::find(&db_session, descendants_q, (node.root_id, node_ids_chunk)).await?;

        for descendant in descendants.flatten() {
            nodes.push(descendant);
        }
    }

    Ok(HttpResponse::Ok().json(nodes))
}

#[get("/{root_id}/{id}/description")]
pub async fn get_node_description(
    db_session: web::Data<CachingSession>,
    params: web::Path<PrimaryKeyParams>,
) -> Result<HttpResponse, NodecosmosError> {
    let params = params.into_inner();
    let mut node = GetNodeDescription::new();

    node.root_id = params.root_id;
    node.id = params.id;

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
    node: web::Json<Node>,
    current_user: CurrentUser,
    resource_locker: web::Data<ResourceLocker>,
) -> Result<HttpResponse, NodecosmosError> {
    let mut node = node.into_inner();
    let parent = node.parent(&db_session).await;

    resource_locker.check_node_lock(&node).await?;
    auth_node_creation(&parent, &current_user).await?;

    node.set_owner(Owner {
        id: current_user.id,
        name: current_user.username,
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
    node: web::Json<UpdateNodeTitle>,
    db_session: web::Data<CachingSession>,
    cb_extension: web::Data<CbExtension>,
    current_user: CurrentUser,
) -> Result<HttpResponse, NodecosmosError> {
    let mut node = node.into_inner();
    let native_node = node.as_native().find_by_primary_key(&db_session).await?;

    auth_node_update(&native_node, &current_user).await?;
    node.update_cb(&db_session, &cb_extension).await?;

    Ok(HttpResponse::Ok().json(node))
}

#[put("/description")]
pub async fn update_node_description(
    db_session: web::Data<CachingSession>,
    node: web::Json<UpdateNodeDescription>,
    cb_extension: web::Data<CbExtension>,
    current_user: CurrentUser,
) -> Result<HttpResponse, NodecosmosError> {
    let mut node = node.into_inner();
    let native_node = node.as_native().find_by_primary_key(&db_session).await?;

    auth_node_update(&native_node, &current_user).await?;
    node.update_cb(&db_session, &cb_extension).await?;

    Ok(HttpResponse::Ok().json(node))
}

#[delete("/{root_id}/{id}")]
pub async fn delete_node(
    db_session: web::Data<CachingSession>,
    cb_extension: web::Data<CbExtension>,
    params: web::Path<PrimaryKeyParams>,
    current_user: CurrentUser,
    resource_locker: web::Data<ResourceLocker>,
) -> Result<HttpResponse, NodecosmosError> {
    let params = params.into_inner();
    let mut node = Node::new();

    node.root_id = params.root_id;
    node.id = params.id;

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
    let params = params.into_inner();

    let mut reorderer = Reorderer::new(db_session, params).await?;

    resource_locker.check_node_lock(&reorderer.node).await?;
    auth_node_update(&reorderer.node, &current_user).await?;
    reorderer.reorder(&resource_locker).await?;

    Ok(HttpResponse::Ok().finish())
}
