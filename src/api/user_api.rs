use crate::api::current_user::set_current_user;
use crate::api::data::RequestData;
use crate::api::types::Response;
use crate::errors::NodecosmosError;
use crate::models::user::{GetUser, UpdateUser, User};
use crate::App;
use actix_session::Session;
use actix_web::{delete, get, post, put, web, HttpResponse};
use charybdis::operations::{DeleteWithExtCallbacks, InsertWithExtCallbacks, UpdateWithExtCallbacks};
use charybdis::types::Uuid;
use scylla::CachingSession;
use serde_json::json;

#[get("/{id}")]
pub async fn get_user(db_session: web::Data<CachingSession>, id: web::Path<Uuid>) -> Response {
    let user = GetUser::find_by_id(&db_session, *id).await?;

    Ok(HttpResponse::Ok().json(user))
}

#[post("")]
pub async fn create_user(app: web::Data<App>, client_session: Session, mut user: web::Json<User>) -> Response {
    user.insert_cb(&app.db_session, &app).await?;

    let current_user = set_current_user(&client_session, &user)?;

    Ok(HttpResponse::Ok().json(json!({ "message": "User created", "user": current_user })))
}

#[put("")]
pub async fn update_user(data: RequestData, mut user: web::Json<UpdateUser>) -> Response {
    if user.id != data.current_user.id {
        return Err(NodecosmosError::Unauthorized(json!({
            "error": "Unauthorized",
            "message": "You are not allowed to perform this action!",
        })));
    }

    user.update_cb(data.db_session(), &data.app).await?;

    Ok(HttpResponse::Ok().json(user))
}

#[delete("/{id}")]
pub async fn delete_user(data: RequestData, mut user: web::Path<User>) -> Response {
    if user.id != data.current_user.id {
        return Err(NodecosmosError::Unauthorized(json!({
            "error": "Unauthorized",
            "message": "You are not allowed to perform this action!",
        })));
    }

    user.delete_cb(data.db_session(), &data.app).await?;

    Ok(HttpResponse::Ok().finish())
}
