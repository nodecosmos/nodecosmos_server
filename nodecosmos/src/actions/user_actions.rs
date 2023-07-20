use super::client_session::set_current_user;

use crate::elastic::{add_document, delete_document, update_document};
use crate::models::user::{DeleteUser, GetUser, UpdateUser, User};
use actix_session::Session;
use actix_web::{delete, get, post, put, web, HttpResponse, Responder};
use charybdis::{CharybdisError, Delete, Find, InsertWithCallbacks, UpdateWithCallbacks, Uuid};
use elasticsearch::Elasticsearch;
use scylla::CachingSession;
use serde_json::json;

#[get("/{id}")]
pub async fn get_user(
    db_session: web::Data<CachingSession>,
    id: web::Path<Uuid>,
) -> impl Responder {
    let user = GetUser {
        id: id.into_inner(),
        ..Default::default()
    };

    let user = user.find_by_primary_key(&db_session).await;

    match user {
        Ok(user) => HttpResponse::Ok().json(user),
        Err(e) => HttpResponse::NotFound().body(e.to_string()),
    }
}

#[post("")]
pub async fn create_user(
    db_session: web::Data<CachingSession>,
    elastic_client: web::Data<Elasticsearch>,
    client_session: Session,
    user: web::Json<User>,
) -> impl Responder {
    let mut user = user.into_inner();

    let res = user.insert_cb(&db_session).await;

    match res {
        Ok(_) => {
            let current_user = set_current_user(&client_session, &user);
            add_document(
                &elastic_client,
                User::ELASTIC_IDX_NAME,
                &user,
                user.id.to_string(),
            )
            .await;

            match current_user {
                Ok(current_user) => HttpResponse::Ok()
                    .json(json!({ "message": "User created", "user": current_user })),
                Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
            }
        }
        Err(e) => match e {
            CharybdisError::ValidationError((field, message)) => {
                HttpResponse::Conflict().json(json!({ "error": {field: message} }))
            }
            _ => HttpResponse::InternalServerError().body(e.to_string()),
        },
    }
}

#[put("")]
pub async fn update_user(
    db_session: web::Data<CachingSession>,
    elastic_client: web::Data<Elasticsearch>,
    user: web::Json<UpdateUser>,
) -> impl Responder {
    // TODO: authorize update
    let mut user = user.into_inner();
    let res = user.update_cb(&db_session).await;

    update_document(
        &elastic_client,
        User::ELASTIC_IDX_NAME,
        &user,
        user.id.to_string(),
    )
    .await;

    match res {
        Ok(_) => HttpResponse::Ok().json(json!({"message": "User updated"})),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

#[delete("/{id}")]
pub async fn delete_user(
    db_session: web::Data<CachingSession>,
    elastic_client: web::Data<Elasticsearch>,
    id: web::Path<Uuid>,
) -> impl Responder {
    // TODO: authorize deletion

    let user = DeleteUser {
        id: id.into_inner(),
    };

    delete_document(&elastic_client, User::ELASTIC_IDX_NAME, user.id.to_string()).await;

    let res = user.delete(&db_session).await;

    match res {
        Ok(_) => HttpResponse::Ok().json(json!({"message": "User deleted"})),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}
