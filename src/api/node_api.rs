use crate::api::request::current_user::OptCurrentUser;
use crate::api::request::data::RequestData;
use crate::api::types::Response;
use crate::app::App;
use crate::models::node::reorder::ReorderParams;
use crate::models::node::search::{NodeSearch, NodeSearchQuery};
use crate::models::node::*;
use crate::models::traits::{Authorization, Branchable, MergeDescription};
use actix_multipart::Multipart;
use actix_web::{delete, get, post, put, web, HttpResponse};
use charybdis::model::AsNative;
use charybdis::operations::{DeleteWithCallbacks, Find, InsertWithCallbacks, UpdateWithCallbacks};
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
pub async fn get_node(app: web::Data<App>, id: web::Path<Uuid>, opt_cu: OptCurrentUser) -> Response {
    let node = BaseNode::find_by_id_and_branch_id(*id, *id)
        .execute(&app.db_session)
        .await?;

    let mut native_node = node.as_native();

    native_node.auth_view(&app, opt_cu).await?;

    let descendants = native_node
        .descendants(&app.db_session, None)
        .await?
        .try_collect()
        .await?;

    Ok(HttpResponse::Ok().json({
        json!({
            "node": node,
            "descendants": descendants
        })
    }))
}

#[get("/{id}/{branchId}")]
pub async fn get_branched_node(app: web::Data<App>, pk: web::Path<PrimaryKeyNode>, opt_cu: OptCurrentUser) -> Response {
    let node = BaseNode::find_by_id_and_branch_id(pk.id, pk.branch_id)
        .execute(&app.db_session)
        .await?;

    let mut native_node = node.as_native();

    native_node.auth_view(&app, opt_cu).await?;

    let descendants = native_node.branch_descendants(&app.db_session, None).await?;

    Ok(HttpResponse::Ok().json(json!({
        "node": node,
        "descendants": descendants
    })))
}

#[get("/{id}/{branchId}/description")]
pub async fn get_node_description(
    db_session: web::Data<CachingSession>,
    mut node: web::Path<GetDescriptionNode>,
) -> Response {
    node.find_branched(&db_session).await?;

    Ok(HttpResponse::Ok().json(node.into_inner()))
}

#[get("/{id}/{branchId}/description_base64")]
pub async fn get_node_description_base64(
    db_session: web::Data<CachingSession>,
    node: web::Path<GetDescriptionBase64Node>,
) -> Response {
    let mut node = node.into_inner();
    node.find_branched(&db_session).await?;

    // we always return merged description as we want to keep branched description in sync with original
    if node.is_branched() && node.description_base64.is_some() {
        let original = UpdateDescriptionNode::find_by_id_and_branch_id(node.id, node.id)
            .execute(&db_session)
            .await;

        if let Ok(mut original) = original {
            original.description_base64 = node.description_base64;
            original.merge_description(&db_session).await?;

            node.description = original.description;
            node.description_markdown = original.description_markdown;
            node.description_base64 = original.description_base64;
        }
    }

    Ok(HttpResponse::Ok().json(node))
}

#[get("/{id}/original/description_base64")]
pub async fn get_original_node_description_base64(
    db_session: web::Data<CachingSession>,
    mut node: web::Path<GetDescriptionBase64Node>,
) -> Response {
    node.branch_id = node.id;
    let node = node.find_by_primary_key().execute(&db_session).await?;

    Ok(HttpResponse::Ok().json(node))
}

#[post("")]
pub async fn create_node(mut node: web::Json<Node>, data: RequestData) -> Response {
    node.auth_creation(&data).await?;

    data.resource_locker().validate_node_root_unlocked(&node, true).await?;

    node.insert_cb(&data).execute(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(node))
}

#[put("/title")]
pub async fn update_node_title(mut node: web::Json<UpdateTitleNode>, data: RequestData) -> Response {
    let mut native_node = node.as_native();

    native_node.auth_update(&data).await?;

    data.resource_locker()
        .validate_node_root_unlocked(&native_node, true)
        .await?;

    node.update_cb(&data).execute(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(node))
}

#[put("/description")]
pub async fn update_node_description(mut node: web::Json<UpdateDescriptionNode>, data: RequestData) -> Response {
    let mut native_node = node.as_native();

    native_node.auth_update(&data).await?;

    node.update_cb(&data).execute(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(node))
}

#[delete("/{id}/{branchId}")]
pub async fn delete_node(node: web::Path<PrimaryKeyNode>, data: RequestData) -> Response {
    let mut node = node.as_native();

    node.transform_to_branched(data.db_session()).await?;

    node.auth_update(&data).await?;

    data.resource_locker().validate_node_root_unlocked(&node, true).await?;

    node.delete_cb(&data).execute(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(node))
}

#[put("/reorder")]
pub async fn reorder_nodes(params: web::Json<ReorderParams>, data: RequestData) -> Response {
    let mut node = Node::find_branched_or_original(data.db_session(), params.id, params.branch_id, None).await?;

    node.auth_update(&data).await?;

    node.reorder(&data, params.into_inner()).await?;

    Ok(HttpResponse::Ok().finish())
}

#[post("/{id}/{branchId}/upload_cover_image")]
async fn upload_cover_image(node: web::Path<UpdateCoverImageNode>, data: RequestData, payload: Multipart) -> Response {
    let mut node = node.find_by_primary_key().execute(data.db_session()).await?;

    node.as_native().auth_update(&data).await?;

    node.update_cover_image(&data, payload).await?;

    Ok(HttpResponse::Ok().json(json!({
        "url": node.cover_image_url
    })))
}

#[delete("/{id}/{branchId}/delete_cover_image")]
async fn delete_cover_image(node: web::Path<UpdateCoverImageNode>, data: RequestData) -> Response {
    let mut node = node.find_by_primary_key().execute(data.db_session()).await?;

    node.as_native().auth_update(&data).await?;

    node.delete_cover_image(&data).await?;

    Ok(HttpResponse::Ok().finish())
}
