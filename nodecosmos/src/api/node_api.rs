use actix_multipart::Multipart;
use actix_web::{delete, get, post, put, web, HttpResponse};
use charybdis::model::AsNative;
use charybdis::operations::{DeleteWithCallbacks, InsertWithCallbacks, UpdateWithCallbacks};
use charybdis::types::Uuid;
use scylla::CachingSession;
use serde_json::json;
use tokio_stream::wrappers::BroadcastStream;

use crate::api::request::current_user::OptCurrentUser;
use crate::api::request::data::RequestData;
use crate::api::types::{ActionObject, ActionTypes, Response};
use crate::app::App;
use crate::models::node::reorder::ReorderParams;
use crate::models::node::search::{NodeSearch, NodeSearchQuery};
use crate::models::node::*;
use crate::models::traits::Descendants;
use crate::models::traits::{Authorization, Branchable};
use crate::resources::resource_locker::ResourceLocker;

#[get("")]
pub async fn get_nodes(app: web::Data<App>, query: web::Query<NodeSearchQuery>) -> Response {
    let nodes = NodeSearch::new(&app.elastic_client, &query).index().await?;
    Ok(HttpResponse::Ok().json(nodes))
}

#[get("/{id}")]
pub async fn get_node(db_session: web::Data<CachingSession>, opt_cu: OptCurrentUser, id: web::Path<Uuid>) -> Response {
    let mut node = BaseNode::find_by_id_and_branch_id(*id, *id)
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

#[get("/{id}/{branchId}")]
pub async fn get_branched_node(
    db_session: web::Data<CachingSession>,
    opt_cu: OptCurrentUser,
    pk: web::Path<PrimaryKeyNode>,
) -> Response {
    let mut node = BaseNode::find_by_id_and_branch_id(pk.id, pk.branch_id)
        .execute(&db_session)
        .await?;

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
            node.branchise_id(node.root_id),
            vec![ActionTypes::Reorder(ActionObject::Node), ActionTypes::Merge],
            ResourceLocker::TWO_SECONDS,
        )
        .await?;

    node.insert_cb(&data).execute(data.db_session()).await?;

    data.resource_locker()
        .unlock_resource_actions(
            node.root_id,
            node.branchise_id(node.root_id),
            vec![ActionTypes::Reorder(ActionObject::Node), ActionTypes::Merge],
        )
        .await?;

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
            node.branchise_id(node.root_id),
            vec![ActionTypes::Reorder(ActionObject::Node), ActionTypes::Merge],
            ResourceLocker::TWO_SECONDS,
        )
        .await?;

    node.update_cb(&data).execute(data.db_session()).await?;

    data.resource_locker()
        .unlock_resource_actions(
            node.root_id,
            node.branchise_id(node.root_id),
            vec![ActionTypes::Reorder(ActionObject::Node), ActionTypes::Merge],
        )
        .await?;

    Ok(HttpResponse::Ok().json(node))
}

#[delete("/{id}/{branchId}")]
pub async fn delete_node(node: web::Path<PrimaryKeyNode>, data: RequestData) -> Response {
    let mut node = node.as_native();

    node.auth_update(&data).await?;

    data.resource_locker()
        .lock_resource_actions(
            node.root_id,
            node.branchise_id(node.root_id),
            vec![ActionTypes::Reorder(ActionObject::Node), ActionTypes::Merge],
            ResourceLocker::ONE_HOUR,
        )
        .await?;

    node.delete_cb(&data).execute(data.db_session()).await?;

    data.resource_locker()
        .unlock_resource_actions(
            node.root_id,
            node.branchise_id(node.root_id),
            vec![ActionTypes::Reorder(ActionObject::Node), ActionTypes::Merge],
        )
        .await?;

    Ok(HttpResponse::Ok().json(node))
}

#[put("/reorder")]
pub async fn reorder_nodes(params: web::Json<ReorderParams>, data: RequestData) -> Response {
    let mut node = Node::find_by_id_and_branch_id(params.id, params.branch_id)
        .execute(data.db_session())
        .await?;

    node.auth_update(&data).await?;

    // first lock the complete resource to avoid all types of race conditions
    data.resource_locker()
        .lock_resource(node.root_id, node.branchise_id(node.root_id), ResourceLocker::ONE_HOUR)
        .await?;

    // validate that reorder is allowed
    if let Err(e) = data
        .resource_locker()
        .validate_resource_action_unlocked(
            ActionTypes::Reorder(ActionObject::Node),
            node.root_id,
            node.branchise_id(node.root_id),
        )
        .await
    {
        // unlock complete resource as reorder is not allowed
        data.resource_locker()
            .unlock_resource(node.root_id, node.branchise_id(node.root_id))
            .await?;

        // return reorder not allowed error
        return Err(e);
    }

    // execute reorder
    node.reorder(&data, params.into_inner()).await?;

    // unlock complete resource
    data.resource_locker()
        .unlock_resource(node.root_id, node.branchise_id(node.root_id))
        .await?;

    Ok(HttpResponse::Ok().finish())
}

#[post("/{id}/{branchId}/upload_cover_image")]
async fn upload_cover_image(
    mut node: web::Path<UpdateCoverImageNode>,
    data: RequestData,
    payload: Multipart,
) -> Response {
    AuthNode::auth_update(&data, node.id, node.branch_id).await?;

    node.update_cover_image(&data, payload).await?;

    Ok(HttpResponse::Ok().json(json!({
        "url": node.cover_image_url
    })))
}

#[delete("/{id}/{branchId}/delete_cover_image")]
async fn delete_cover_image(mut node: web::Path<UpdateCoverImageNode>, data: RequestData) -> Response {
    AuthNode::auth_update(&data, node.id, node.branch_id).await?;

    node.delete_cover_image(&data).await?;

    Ok(HttpResponse::Ok().finish())
}

#[get("/{id}/events/listen")]
pub async fn listen_node_events(id: web::Path<Uuid>, data: RequestData) -> Response {
    let sender = data.sse_broadcast().get_or_create_room(id.into_inner());
    let receiver = sender.subscribe();
    let stream = BroadcastStream::new(receiver);

    Ok(HttpResponse::Ok().content_type("text/event-stream").streaming(stream))
}
