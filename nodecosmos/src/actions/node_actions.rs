use crate::models::node::Node;
use actix_session::Session;

use crate::actions::client_session::get_current_user;
use crate::authorize::{auth_node_creation, auth_node_update};
use actix_web::{delete, get, post, put, web, HttpResponse, Responder};
use charybdis::prelude::{
    DeleteWithCallbacks, Find, InsertWithCallbacks, UpdateWithCallbacks, Uuid,
};
use scylla::CachingSession;
use serde_json::json;

const DEFAULT_PAGE_SIZE: i32 = 100;

#[get("/{root_id}/{id}")]
pub async fn get_node(
    db_session: web::Data<CachingSession>,
    root_id: web::Path<Uuid>,
    id: web::Path<Uuid>,
) -> impl Responder {
    let mut node = GetNode::new();
    node.root_id = root_id.into_inner();
    node.id = id.into_inner();

    let node = node.find_by_primary_key(&db_session).await;

    match node {
        Ok(node) => {
            let mut with_descendant_ids = node.descendant_ids.unwrap_or_default();
            with_descendant_ids.push(node.id);

            let descendants_q = find_node_query!("root_id = ?, id IN (?)");
            let descendants = Node::find_iter(
                &db_session,
                descendants_q,
                (node.root_id, with_descendant_ids),
                DEFAULT_PAGE_SIZE,
            )
            .await;

            match descendants {
                Ok(descendants) => {
                    let mut nodes = vec![node];
                    nodes.extend(descendants);

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
    client_session: Session,
    node: web::Json<Node>,
) -> impl Responder {
    let mut node = node.into_inner();
    let parent = node.get_descendable_parent(&db_session).await;
    let current_user = get_current_user(&client_session);

    match auth_node_creation(&node, &parent, &db_session, &current_user).await {
        Ok(_) => (),
        Err(e) => return HttpResponse::Unauthorized().body(e.to_string()),
    }

    let res = node.insert_cb(&db_session).await;

    match res {
        Ok(_) => HttpResponse::Ok().json(node),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

#[put("")]
pub async fn update_node(
    db_session: web::Data<CachingSession>,
    node: web::Json<Node>,
) -> impl Responder {
    let mut node = node.into_inner();
    let current_user = get_current_user(&client_session);

    match auth_node_update(&node, &db_session, &current_user).await {
        Ok(_) => (),
        Err(e) => return HttpResponse::Unauthorized().body(e.to_string()),
    }

    let res = node.update_cb(&db_session).await;

    match res {
        Ok(_) => HttpResponse::Ok().json(node),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

#[delete("")]
pub async fn delete_node(
    db_session: web::Data<CachingSession>,
    node: web::Json<Node>,
) -> impl Responder {
    let mut node = node.into_inner();
    let current_user = get_current_user(&client_session);

    match auth_node_update(&node, &db_session, &current_user).await {
        Ok(_) => (),
        Err(e) => return HttpResponse::Unauthorized().body(e.to_string()),
    }

    let res = node.delete_cb(&db_session).await;

    match res {
        Ok(_) => HttpResponse::Ok().json(node),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}
