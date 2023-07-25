use crate::actions::client_session::*;
use crate::app::CbExtension;
use crate::authorize::{auth_node_creation, auth_node_update};
use crate::errors::NodecosmosError;
use crate::models::node::*;
use crate::models::udts::{Owner, OwnerTypes};
use crate::services::nodes::search::{NodeSearchQuery, NodeSearchService};
use actix_web::{delete, get, post, put, web, HttpResponse};
use charybdis::{
    AsNative, DeleteWithExtCallbacks, Deserialize, Find, InsertWithExtCallbacks, New,
    UpdateWithExtCallbacks, Uuid,
};
use elasticsearch::Elasticsearch;
use futures::StreamExt;
use scylla::CachingSession;
use serde_json::json;

const DEFAULT_PAGE_SIZE: i32 = 5;

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
) -> Result<HttpResponse, NodecosmosError> {
    let mut node = BaseNode::new();
    node.root_id = root_id.into_inner();

    let nodes_iter = node.find_by_partition_key(&db_session).await?;

    let nodes: Vec<BaseNode> = nodes_iter.flatten().collect();

    Ok(HttpResponse::Ok().json(nodes))
}

#[get("/{root_id}/{id}")]
pub async fn get_node(
    db_session: web::Data<CachingSession>,
    params: web::Path<PrimaryKeyParams>,
) -> Result<HttpResponse, NodecosmosError> {
    let params = params.into_inner();
    let mut node = BaseNode::new();

    node.root_id = params.root_id;
    node.id = params.id;

    let node = node.find_by_primary_key(&db_session).await?;

    let mut all_node_ids = node.descendant_ids.clone().unwrap_or_default();
    all_node_ids.push(node.id);

    let descendants_q = find_base_node_query!("root_id = ? AND id IN ?");

    let mut descendants = BaseNode::find_iter(
        &db_session,
        descendants_q,
        (node.root_id, all_node_ids),
        DEFAULT_PAGE_SIZE,
    )
    .await?;

    let mut nodes = vec![];

    while let Some(descendant) = descendants.next().await {
        if let Ok(descendant) = descendant {
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
) -> Result<HttpResponse, NodecosmosError> {
    let mut node = node.into_inner();
    let parent = node.parent(&db_session).await;

    auth_node_creation(&parent, &current_user).await?;

    node.set_owner(Owner {
        id: current_user.id,
        name: current_user.username,
        owner_type: OwnerTypes::User.into(),
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
) -> Result<HttpResponse, NodecosmosError> {
    let params = params.into_inner();
    let mut node = Node::new();

    node.root_id = params.root_id;
    node.id = params.id;

    let mut node = node.find_by_primary_key(&db_session).await?;

    auth_node_update(&node, &current_user).await?;
    node.delete_cb(&db_session, &cb_extension).await?;

    Ok(HttpResponse::Ok().finish())
}
