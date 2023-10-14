use crate::client_session::{get_current_user, set_current_user};
use crate::errors::NodecosmosError;
use crate::models::user::User;
use actix_session::Session;
use actix_web::{delete, get, post, web, HttpResponse, Responder};
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
) -> Result<HttpResponse, NodecosmosError> {
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
        return Ok(HttpResponse::NotFound().finish());
    }

    if !user.verify_password(&login_form.password).await? {
        return Ok(HttpResponse::NotFound().finish());
    }

    let current_user = set_current_user(&client_session, &user)?;

    Ok(HttpResponse::Ok().json(json!({"success": true, "user": current_user})))
}

#[get("/sync")]
pub async fn sync(client_session: Session) -> impl Responder {
    let current_user = get_current_user(&client_session);

    match current_user {
        Some(current_user) => {
            HttpResponse::Ok().json(json!({"success": true, "user": current_user}))
        }
        None => HttpResponse::Ok().json(json!({"success": false})),
    }
}

#[delete("/logout")]
pub async fn logout(client_session: Session) -> impl Responder {
    client_session.clear();
    HttpResponse::Ok().json(json!({"success": true}))
}
