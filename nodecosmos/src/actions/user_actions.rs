use crate::authorize::auth_user_update;
use crate::client_session::set_current_user;
use crate::client_session::CurrentUser;
use crate::errors::NodecosmosError;
use crate::models::user::{GetUser, UpdateUser, User};
use crate::CbExtension;
use actix_session::Session;
use actix_web::{delete, get, post, put, web, HttpResponse};
use charybdis::{
    AsNative, DeleteWithExtCallbacks, Find, InsertWithExtCallbacks, UpdateWithExtCallbacks, Uuid,
};
use scylla::CachingSession;
use serde_json::json;

#[get("/{id}")]
pub async fn get_user(
    db_session: web::Data<CachingSession>,
    id: web::Path<Uuid>,
) -> Result<HttpResponse, NodecosmosError> {
    let user = GetUser {
        id: id.into_inner(),
        ..Default::default()
    };

    let user = user.find_by_primary_key(&db_session).await?;

    Ok(HttpResponse::Ok().json(user))
}

#[post("")]
pub async fn create_user(
    db_session: web::Data<CachingSession>,
    cb_extension: web::Data<CbExtension>,
    client_session: Session,
    mut user: web::Json<User>,
) -> Result<HttpResponse, NodecosmosError> {
    user.insert_cb(&db_session, &cb_extension).await?;

    let current_user = set_current_user(&client_session, &user)?;

    Ok(HttpResponse::Ok().json(json!({ "message": "User created", "user": current_user })))
}

#[put("")]
pub async fn update_user(
    db_session: web::Data<CachingSession>,
    cb_extension: web::Data<CbExtension>,
    mut user: web::Json<UpdateUser>,
    current_user: CurrentUser,
) -> Result<HttpResponse, NodecosmosError> {
    let native_user = user.as_native();

    auth_user_update(&native_user, &current_user).await?;

    user.update_cb(&db_session, &cb_extension).await?;

    Ok(HttpResponse::Ok().json(user))
}

#[delete("/{id}")]
pub async fn delete_user(
    db_session: web::Data<CachingSession>,
    cb_extension: web::Data<CbExtension>,
    current_user: CurrentUser,
    id: web::Path<Uuid>,
) -> Result<HttpResponse, NodecosmosError> {
    let mut user = User {
        id: id.into_inner(),
        ..Default::default()
    };

    auth_user_update(&user, &current_user).await?;

    user.delete_cb(&db_session, &cb_extension).await?;

    Ok(HttpResponse::Ok().finish())
}
