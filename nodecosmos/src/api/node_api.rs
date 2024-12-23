use actix_multipart::Multipart;
use actix_web::{delete, get, post, put, web, HttpResponse};
use charybdis::batch::ModelBatch;
use charybdis::model::AsNative;
use charybdis::operations::{DeleteWithCallbacks, InsertWithCallbacks, UpdateWithCallbacks};
use charybdis::types::Uuid;
use futures::StreamExt;
use scylla::CachingSession;
use serde::Deserialize;
use serde_json::json;
use tokio_stream::wrappers::BroadcastStream; // This is crucial for handling streams

use crate::api::request::current_user::OptCurrentUser;
use crate::api::request::data::RequestData;
use crate::api::types::{ActionObject, ActionTypes, Response};
use crate::app::App;
use crate::errors::NodecosmosError;
use crate::models::invitation::Invitation;
use crate::models::node::import::Import;
use crate::models::node::reorder::ReorderParams;
use crate::models::node::search::{NodeSearch, NodeSearchQuery};
use crate::models::node::*;
use crate::models::traits::Authorization;
use crate::models::traits::Descendants;
use crate::models::user::ShowUser;
use crate::resources::resource_locker::ResourceLocker;

#[get("")]
pub async fn get_nodes(app: web::Data<App>, query: web::Query<NodeSearchQuery>) -> Response {
    let nodes = NodeSearch::new(&app.elastic_client, &query).index().await?;
    Ok(HttpResponse::Ok().json(nodes))
}

#[get("/{branchId}/{id}/original")]
pub async fn get_node(
    db_session: web::Data<CachingSession>,
    opt_cu: OptCurrentUser,
    params: web::Path<PrimaryKeyNode>,
) -> Response {
    let mut node = BaseNode::find_by_branch_id_and_id(params.branch_id, params.id)
        .execute(&db_session)
        .await?;

    node.auth_view(&db_session, &opt_cu).await?;

    let descendants = node.descendants(&db_session).await?.try_collect().await?;

    Ok(HttpResponse::Ok().json({
        json!({
            "node": node,
            "descendants": descendants
        })
    }))
}

#[derive(Deserialize)]
pub struct BranchedNodeQ {
    #[serde(rename = "originalId")]
    original_id: Option<Uuid>,
}

#[get("/{branchId}/{id}/branch")]
pub async fn get_branched_node(
    db_session: web::Data<CachingSession>,
    opt_cu: OptCurrentUser,
    pk: web::Path<PrimaryKeyNode>,
    q: web::Query<BranchedNodeQ>,
) -> Response {
    let mut node = if let Some(original_id) = q.original_id {
        match BaseNode::maybe_find_first_by_branch_id_and_id(pk.branch_id, pk.id)
            .execute(&db_session)
            .await?
        {
            Some(node) => node,
            None => {
                BaseNode::find_by_branch_id_and_id(original_id, pk.id)
                    .execute(&db_session)
                    .await?
            }
        }
    } else {
        BaseNode::find_by_branch_id_and_id(pk.branch_id, pk.id)
            .execute(&db_session)
            .await?
    };

    node.branch_id = pk.branch_id;
    node.auth_view(&db_session, &opt_cu).await?;

    let descendants = node.branch_descendants(&db_session).await?;

    Ok(HttpResponse::Ok().json(json!({
        "node": node,
        "descendants": descendants
    })))
}

#[post("")]
pub async fn create_node(node: web::Json<Node>, data: RequestData) -> Response {
    let mut node = node.into_inner();

    node.auth_creation(&data).await?;

    data.resource_locker()
        .lock_resource_actions(
            node.root_id,
            node.branch_id,
            &[ActionTypes::Reorder(ActionObject::Node), ActionTypes::Merge],
            ResourceLocker::TWO_SECONDS,
        )
        .await?;

    let insert = node.insert_cb(&data).execute(data.db_session()).await;

    data.resource_locker()
        .unlock_resource_actions(
            node.root_id,
            node.branch_id,
            &[ActionTypes::Reorder(ActionObject::Node), ActionTypes::Merge],
        )
        .await?;

    if let Err(e) = insert {
        return Err(e);
    }

    Ok(HttpResponse::Ok().json(node))
}

#[put("/title")]
pub async fn update_node_title(node: web::Json<UpdateTitleNode>, data: RequestData) -> Response {
    let mut node = node.into_inner();

    node.auth_update(&data).await?;

    // prevent reorder as we need to update `NodeDescendant` title for ancestors
    data.resource_locker()
        .lock_resource_actions(
            node.root_id,
            node.branch_id,
            &[ActionTypes::Reorder(ActionObject::Node), ActionTypes::Merge],
            ResourceLocker::TWO_SECONDS,
        )
        .await?;

    let res = node.update_cb(&data).execute(data.db_session()).await;

    data.resource_locker()
        .unlock_resource_actions(
            node.root_id,
            node.branch_id,
            &[ActionTypes::Reorder(ActionObject::Node), ActionTypes::Merge],
        )
        .await?;

    res?;

    Ok(HttpResponse::Ok().json(node))
}

#[delete("/{branchId}/{id}/{rootId}")]
pub async fn delete_node(node: web::Path<PrimaryKeyNode>, data: RequestData) -> Response {
    let mut node = node.as_native();

    node.auth_update(&data).await?;

    data.resource_locker()
        .lock_resource_actions(
            node.root_id,
            node.branch_id,
            &[ActionTypes::Reorder(ActionObject::Node), ActionTypes::Merge],
            ResourceLocker::ONE_HOUR,
        )
        .await?;

    node.delete_cb(&data).execute(data.db_session()).await?;

    data.resource_locker()
        .unlock_resource_actions(
            node.root_id,
            node.branch_id,
            &[ActionTypes::Reorder(ActionObject::Node), ActionTypes::Merge],
        )
        .await?;

    Ok(HttpResponse::Ok().json(node))
}

#[put("/reorder")]
pub async fn reorder_nodes(params: web::Json<ReorderParams>, data: RequestData) -> Response {
    AuthNode::auth_update(&data, params.branch_id, params.id, params.root_id).await?;

    // first lock the complete resource to avoid all kinds of race conditions
    data.resource_locker()
        .lock_resource(params.root_id, params.branch_id, ResourceLocker::ONE_HOUR)
        .await?;

    // validate that reorder is allowed
    if let Err(e) = data
        .resource_locker()
        .validate_resource_action_unlocked(
            ActionTypes::Reorder(ActionObject::Node),
            params.root_id,
            params.branch_id,
            true,
        )
        .await
    {
        // unlock complete resource as reorder is not allowed
        data.resource_locker()
            .unlock_resource(params.root_id, params.branch_id)
            .await?;

        // return reorder not allowed error
        return Err(e);
    }

    // execute reorder
    let res = Node::reorder(&data, &params).await;

    match res {
        Ok(_) => {
            // unlock complete resource
            data.resource_locker()
                .unlock_resource(params.root_id, params.branch_id)
                .await?;
            Ok(HttpResponse::Ok().finish())
        }
        Err(e) => {
            match e {
                // unlock complete resource in case of validation errors
                NodecosmosError::Forbidden(_) | NodecosmosError::Conflict(_) => {
                    data.resource_locker()
                        .unlock_resource(params.root_id, params.branch_id)
                        .await?;
                }
                _ => {}
            }
            Err(e)
        }
    }
}

#[post("/{branchId}/{id}/{rootId}/upload_cover_image")]
async fn upload_cover_image(
    mut node: web::Path<UpdateCoverImageNode>,
    data: RequestData,
    payload: Multipart,
) -> Response {
    AuthNode::auth_update(&data, node.branch_id, node.id, node.root_id).await?;

    node.update_cover_image(&data, payload).await?;

    Ok(HttpResponse::Ok().json(json!({
        "url": node.cover_image_url
    })))
}

#[delete("/{branchId}/{id}/{rootId}/delete_cover_image")]
async fn delete_cover_image(mut node: web::Path<UpdateCoverImageNode>, data: RequestData) -> Response {
    AuthNode::auth_update(&data, node.branch_id, node.id, node.root_id).await?;

    node.delete_cover_image(&data).await?;

    Ok(HttpResponse::Ok().finish())
}

#[get("/{root_id}/events/listen")]
pub async fn listen_node_events(root_id: web::Path<Uuid>, data: RequestData) -> Response {
    let root_id = *root_id;
    let broadcaster = data.sse_broadcast();
    let receiver = broadcaster.build_receiver(root_id);
    let stream = BroadcastStream::new(receiver).map(|msg| match msg {
        Ok(data) => Ok::<_, actix_web::Error>(data),
        Err(_) => Err(NodecosmosError::InternalServerError("Failed to send event".to_string()).into()),
    });

    Ok(HttpResponse::Ok()
        .content_type("text/event-stream")
        .insert_header(("Cache-Control", "no-cache"))
        .insert_header(("Connection", "keep-alive"))
        .streaming(stream))
}

#[get("/{branchId}/{id}/editors")]
pub async fn get_node_editors(db_session: web::Data<CachingSession>, pk: web::Path<PrimaryKeyNode>) -> Response {
    let node = AuthNode::find_by_branch_id_and_id(pk.branch_id, pk.id)
        .execute(&db_session)
        .await?;
    let user_ids = node.editor_ids.unwrap_or_default();
    let users = ShowUser::find_by_ids(&db_session, user_ids).await?;

    Ok(HttpResponse::Ok().json(users))
}

#[delete("/{branchId}/{id}/editors/{editorId}")]
pub async fn delete_node_editor(
    db_session: web::Data<CachingSession>,
    data: RequestData,
    params: web::Path<(Uuid, Uuid, Uuid)>,
) -> Response {
    let (branch_id, id, editor_id) = params.into_inner();

    let node = Node::find_by_branch_id_and_id(branch_id, id)
        .execute(&db_session)
        .await?;

    if node.owner_id != data.current_user.id {
        return Err(NodecosmosError::Forbidden(
            "Only the owner can remove editors".to_string(),
        ));
    }

    let mut descendants = node.descendants(data.db_session()).await?;

    let mut statement_vals = vec![(vec![editor_id], node.branch_id, node.id)];

    while let Some(descendant) = descendants.next().await {
        let descendant = descendant?;

        statement_vals.push((vec![editor_id], descendant.branch_id, descendant.id));
    }

    Node::statement_batch()
        .chunked_statements(&db_session, Node::PULL_EDITOR_IDS_QUERY, statement_vals.clone(), 100)
        .await?;

    Node::statement_batch()
        .chunked_statements(&db_session, Node::PULL_VIEWER_IDS_QUERY, statement_vals, 100)
        .await?;

    Invitation::delete_by_editor_id(&db_session, branch_id, id, editor_id).await?;

    Ok(HttpResponse::Ok().finish())
}

#[put("/{branchId}/{id}/import_nodes")]
pub async fn import_nodes(data: RequestData, json_file: Multipart, params: web::Path<(Uuid, Uuid)>) -> Response {
    let (branch_id, id) = params.into_inner();

    let mut current_root = Node::find_by_branch_id_and_id(branch_id, id)
        .execute(data.db_session())
        .await?;

    let root_id = current_root.root_id;

    current_root.auth_update(&data).await?;

    data.resource_locker()
        .lock_resource_actions(
            current_root.root_id,
            current_root.branch_id,
            &[ActionTypes::Reorder(ActionObject::Node), ActionTypes::Merge],
            ResourceLocker::TWO_SECONDS,
        )
        .await?;

    let import_run = Import::new(current_root, json_file).await?.run(&data).await;

    data.resource_locker()
        .unlock_resource_actions(
            root_id,
            branch_id,
            &[ActionTypes::Reorder(ActionObject::Node), ActionTypes::Merge],
        )
        .await?;

    if let Err(e) = import_run {
        return Err(e);
    }

    Ok(HttpResponse::Ok().finish())
}
