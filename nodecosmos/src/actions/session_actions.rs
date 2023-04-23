use crate::client_session::{get_current_user, set_current_user};
use crate::models::user::*;

use actix_session::Session;
use actix_web::{delete, get, post, web, HttpResponse, Responder};
use charybdis::prelude::CharybdisError;
use scylla::CachingSession;
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize, Debug)]
pub struct LoginForm {
    pub username_or_email: String,
    pub password: String,
}

#[post("/login")]
pub async fn login(
    client_session: Session,
    db_session: web::Data<CachingSession>,
    login_form: web::Json<LoginForm>,
) -> impl Responder {
    let login_form = login_form.into_inner();

    let mut user = User {
        username: login_form.username_or_email.clone(),
        email: login_form.username_or_email.clone(),
        ..Default::default()
    };

    if let Some(user_by_username) = user.find_by_username(&db_session).await {
        user = user_by_username;
    } else if let Some(user_by_email) = user.find_by_email(&db_session).await {
        user = user_by_email;
    } else {
        return HttpResponse::NotFound()
            .json(json!({"error": {"username_or_email": "is not found"}}));
    }

    if let Err(_e) = user.verify_password(&login_form.password).await {
        return HttpResponse::NotFound().json(json!({"error": {"password": "is incorrect"}}));
    }

    match set_current_user(&client_session, user) {
        Ok(current_user) => HttpResponse::Ok().json(json!({"success": true, "user": current_user})),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

#[get("/sync")]
pub async fn sync(client_session: Session) -> impl Responder {
    let current_user: Result<CurrentUser, CharybdisError> = get_current_user(&client_session);

    match current_user {
        Ok(current_user) => HttpResponse::Ok().json(json!({"success": true, "user": current_user})),
        Err(e) => match e {
            CharybdisError::SessionError(_) => {
                HttpResponse::Ok().json(json!({"success": false, "user": null}))
            }
            _ => HttpResponse::InternalServerError().body(e.to_string()),
        },
    }
}

#[delete("/logout")]
pub async fn logout(client_session: Session) -> impl Responder {
    client_session.clear();
    HttpResponse::Ok().json(json!({"success": true}))
}
