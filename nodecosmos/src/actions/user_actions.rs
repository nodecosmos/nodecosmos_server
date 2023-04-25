use super::client_session::set_current_user;

use crate::models::user::{DeleteUser, GetUser, UpdateUser, User};
use actix_session::Session;
use actix_web::{delete, get, post, put, web, HttpResponse, Responder};
use charybdis::prelude::*;
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
    client_session: Session,
    user: web::Json<User>,
) -> impl Responder {
    let mut user = user.into_inner();

    let res = user.insert_cb(&db_session).await;

    match res {
        Ok(_) => {
            let current_user = set_current_user(&client_session, user);

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
    user: web::Json<UpdateUser>,
) -> impl Responder {
    let mut user = user.into_inner();
    let res = user.update_cb(&db_session).await;

    match res {
        Ok(_) => HttpResponse::Ok().json(json!({"message": "User updated"})),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

#[delete("/{id}")]
pub async fn delete_user(
    db_session: web::Data<CachingSession>,
    id: web::Path<Uuid>,
) -> impl Responder {
    let user = DeleteUser {
        id: id.into_inner(),
    };

    let res = user.delete(&db_session).await;

    match res {
        Ok(_) => HttpResponse::Ok().json(json!({"message": "User deleted"})),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}
