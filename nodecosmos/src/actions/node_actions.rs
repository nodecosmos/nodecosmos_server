use crate::actions::client_session::*;
use crate::authorize::{auth_node_creation, auth_node_update};
use crate::errors::NodecosmosError;
use crate::models::node::*;
use actix_web::{delete, get, post, put, web, HttpResponse, Responder};
use charybdis::prelude::{
    AsNative, DeleteWithCallbacks, Find, InsertWithCallbacks, New, UpdateWithCallbacks, Uuid,
};
use futures::StreamExt;
use scylla::CachingSession;
const DEFAULT_PAGE_SIZE: i32 = 100;

#[get("/{root_id}/{id}")]
pub async fn get_node(
    db_session: web::Data<CachingSession>,
    root_id: web::Path<Uuid>,
    id: web::Path<Uuid>,
) -> impl Responder {
    let mut node = Node::new();
    node.root_id = root_id.into_inner();
    node.id = id.into_inner();

    let node = node.find_by_primary_key(&db_session).await;

    match node {
        Ok(node) => {
            let mut all_node_ids = node.descendant_ids.clone().unwrap_or_else(|| vec![]);
            all_node_ids.push(node.id);

            let descendants_q = find_node_query!("root_id = ?, id IN (?)");
            let descendants = Node::find_iter(
                &db_session,
                descendants_q,
                (node.root_id, all_node_ids),
                DEFAULT_PAGE_SIZE,
            )
            .await;

            match descendants {
                Ok(mut descendants) => {
                    let mut nodes = vec![node];

                    while let Some(descendant) = descendants.next().await {
                        if let Ok(descendant) = descendant {
                            nodes.push(descendant);
                        }
                    }

                    HttpResponse::Ok().json(nodes)
                }
                Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
            }
        }
        Err(e) => HttpResponse::NotFound().body(e.to_string()),
    }
}

#[post("")]
pub async fn create_node(
    db_session: web::Data<CachingSession>,
    node: web::Json<Node>,
    current_user: CurrentUser,
) -> Result<HttpResponse, NodecosmosError> {
    let mut node = node.into_inner();
    let parent = node.parent(&db_session).await;

    auth_node_creation(&parent, &current_user).await?;

    node.set_owner_id(current_user.id);

    match parent {
        Some(parent) => {
            node.set_editor_ids(parent.editor_ids);
        }
        None => {
            node.set_editor_ids(Some(vec![current_user.id]));
        }
    }

    let res = node.insert_cb(&db_session).await;

    match res {
        Ok(_) => Ok(HttpResponse::Ok().json(node)),
        Err(e) => Ok(HttpResponse::InternalServerError().body(e.to_string())),
    }
}

#[put("/title")]
pub async fn update_node_title(
    node: web::Json<UpdateNodeTitle>,
    db_session: web::Data<CachingSession>,
    current_user: CurrentUser,
) -> Result<HttpResponse, NodecosmosError> {
    let mut node = node.into_inner();
    let native_node = node.as_native();

    auth_node_update(&native_node, &current_user).await?;

    let res = node.update_cb(&db_session).await;

    match res {
        Ok(_) => Ok(HttpResponse::Ok().json(node)),
        Err(e) => Ok(HttpResponse::InternalServerError().body(e.to_string())),
    }
}

#[put("/description")]
pub async fn update_node_description(
    db_session: web::Data<CachingSession>,
    node: web::Json<UpdateNodeDescription>,
    current_user: CurrentUser,
) -> Result<HttpResponse, NodecosmosError> {
    let mut node = node.into_inner();
    let native_node = node.as_native();

    auth_node_update(&native_node, &current_user).await?;

    let res = node.update_cb(&db_session).await;

    match res {
        Ok(_) => Ok(HttpResponse::Ok().json(node)),
        Err(e) => Ok(HttpResponse::InternalServerError().body(e.to_string())),
    }
}

#[delete("")]
pub async fn delete_node(
    db_session: web::Data<CachingSession>,
    node: web::Json<Node>,
    current_user: CurrentUser,
) -> Result<HttpResponse, NodecosmosError> {
    let mut node = node.into_inner();

    auth_node_update(&node, &current_user).await?;

    let res = node.delete_cb(&db_session).await;

    match res {
        Ok(_) => Ok(HttpResponse::Ok().finish()),
        Err(e) => Ok(HttpResponse::InternalServerError().body(e.to_string())),
    }
}
