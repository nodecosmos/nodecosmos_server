use crate::models::user::*;

use actix_web::{delete, get, post, put, web, HttpResponse, Responder};
use charybdis::prelude::*;
use scylla::CachingSession;
use serde_json::json;

#[get("/{id}")]
pub async fn get_user(session: web::Data<CachingSession>, id: web::Path<Uuid>) -> impl Responder {
    let user = GetUser {
        id: Some(id.into_inner()),
        ..Default::default()
    };

    let user = user.find_by_primary_key(&session).await;

    match user {
        Ok(user) => HttpResponse::Ok().json(user),
        Err(e) => HttpResponse::NotFound().body(e.to_string()),
    }
}

#[post("")]
pub async fn create_user(
    session: web::Data<CachingSession>,
    user: web::Json<User>,
) -> impl Responder {
    let mut user = user.into_inner();
    let res = user.insert_cb(&session).await;

    match res {
        Ok(_) => HttpResponse::Ok().json(json!({"message": "User created"})),
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
    session: web::Data<CachingSession>,
    user: web::Json<UpdateUser>,
) -> impl Responder {
    let mut user = user.into_inner();
    let res = user.update_cb(&session).await;

    match res {
        Ok(_) => HttpResponse::Ok().json(json!({"message": "User updated"})),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

#[delete("/{id}")]
pub async fn delete_user(
    session: web::Data<CachingSession>,
    id: web::Path<Uuid>,
) -> impl Responder {
    let user = DeleteUser {
        id: Some(id.into_inner()),
    };

    let res = user.delete(&session).await;

    match res {
        Ok(_) => HttpResponse::Ok().json(json!({"message": "User deleted"})),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}
