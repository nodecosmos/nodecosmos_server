use actix_multipart::Multipart;
use actix_session::Session;
use actix_web::{delete, get, post, put, web, HttpResponse};
use charybdis::model::AsNative;
use charybdis::operations::{DeleteWithCallbacks, Find, InsertWithCallbacks, UpdateWithCallbacks};
use charybdis::types::Uuid;
use scylla::CachingSession;
use serde::Deserialize;
use serde_json::json;

use crate::api::current_user::{refresh_current_user, remove_current_user, set_current_user};
use crate::api::data::RequestData;
use crate::api::types::Response;
use crate::errors::NodecosmosError;
use crate::models::traits::Authorization;
use crate::models::user::{CurrentUser, ShowUser, UpdateBioUser, UpdateProfileImageUser, User};
use crate::App;

#[derive(Deserialize)]
pub struct LoginForm {
    #[serde(rename = "usernameOrEmail")]
    pub username_or_email: String,
    pub password: String,
}

#[post("/session/login")]
pub async fn login(
    client_session: Session,
    db_session: web::Data<CachingSession>,
    login_form: web::Json<LoginForm>,
) -> Response {
    let user;

    if let Some(user_by_username) = User::maybe_find_first_by_username(login_form.username_or_email.clone())
        .execute(&db_session)
        .await?
    {
        user = user_by_username;
    } else if let Some(user_by_email) = User::maybe_find_first_by_username(login_form.username_or_email.clone())
        .execute(&db_session)
        .await?
    {
        user = user_by_email;
    } else {
        return Err(NodecosmosError::NotFound("Not found".to_string()));
    }

    if !user.verify_password(&login_form.password).await? {
        return Err(NodecosmosError::NotFound("Not found".to_string()));
    }

    let current_user = CurrentUser::from_user(user);

    set_current_user(&client_session, &current_user)?;

    Ok(HttpResponse::Ok().json(current_user))
}

#[get("/session/sync")]
pub async fn sync(data: RequestData, client_session: Session) -> Response {
    let current_user = data
        .current_user
        .find_by_primary_key()
        .execute(data.db_session())
        .await?;

    set_current_user(&client_session, &current_user)?;

    Ok(HttpResponse::Ok().json(current_user))
}

#[delete("/session/logout")]
pub async fn logout(client_session: Session) -> Response {
    client_session.clear();
    Ok(HttpResponse::Ok().finish())
}

#[get("/{id}")]
pub async fn get_user(db_session: web::Data<CachingSession>, id: web::Path<Uuid>) -> Response {
    let user = ShowUser::find_by_id(*id).execute(&db_session).await?;

    Ok(HttpResponse::Ok().json(user))
}

#[get("/{username}/username")]
pub async fn get_user_by_username(db_session: web::Data<CachingSession>, username: web::Path<String>) -> Response {
    let user = ShowUser::find_first_by_username(username.into_inner())
        .execute(&db_session)
        .await?;

    Ok(HttpResponse::Ok().json(user))
}

#[post("")]
pub async fn create_user(app: web::Data<App>, client_session: Session, mut user: web::Json<User>) -> Response {
    user.insert_cb(&app).execute(&app.db_session).await?;

    let current_user = CurrentUser::from_user(user.into_inner());

    set_current_user(&client_session, &current_user)?;

    Ok(HttpResponse::Ok().json(json!({ "message": "User created", "user": current_user })))
}

#[put("/bio")]
pub async fn update_bio(data: RequestData, client_session: Session, mut user: web::Json<UpdateBioUser>) -> Response {
    user.as_native().auth_update(&data).await?;

    user.update_cb(&data).execute(data.db_session()).await?;

    refresh_current_user(&client_session, data.db_session()).await?;

    Ok(HttpResponse::Ok().json(user))
}

#[delete("/{id}")]
pub async fn delete_user(data: RequestData, mut user: web::Path<User>, client_session: Session) -> Response {
    user.auth_update(&data).await?;

    user.delete_cb(&data.app).execute(data.db_session()).await?;

    remove_current_user(&client_session);

    Ok(HttpResponse::Ok().finish())
}

#[post("/{id}/update_profile_image")]
pub async fn update_profile_image(
    data: RequestData,
    client_session: Session,
    mut user: web::Path<UpdateProfileImageUser>,
    payload: Multipart,
) -> Response {
    user.as_native().auth_update(&data).await?;

    user.update_profile_image(&data, payload).await?;

    refresh_current_user(&client_session, data.db_session()).await?;

    Ok(HttpResponse::Ok().json(json!({
        "url": user.profile_image_url
    })))
}

#[delete("/{id}/delete_profile_image")]
pub async fn delete_profile_image(data: RequestData, mut user: web::Path<UpdateProfileImageUser>) -> Response {
    user.as_native().auth_update(&data).await?;

    user.delete_profile_image(&data).await?;

    Ok(HttpResponse::Ok().json(json!({
        "url": user.profile_image_url
    })))
}
