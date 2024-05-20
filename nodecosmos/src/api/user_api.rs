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
use crate::models::token::{Token, TokenType};
use crate::models::traits::Authorization;
use crate::models::user::search::UserSearchQuery;
use crate::models::user::{
    ConfirmUser, CurrentUser, ShowUser, UpdateBioUser, UpdateProfileImageUser, User, UserContext,
};
use crate::models::user_counter::UserCounter;
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
    let current_user = data.current_user.find_by_primary_key().execute(data.db_session()).await;

    match current_user {
        Ok(user) => {
            set_current_user(&client_session, &user)?;
            Ok(HttpResponse::Ok().json(user))
        }
        Err(_) => {
            remove_current_user(&client_session);
            Ok(HttpResponse::Unauthorized().finish())
        }
    }
}

#[delete("/session/logout")]
pub async fn logout(client_session: Session) -> Response {
    client_session.clear();
    Ok(HttpResponse::Ok().finish())
}

#[get("/search")]
pub async fn search_users(data: RequestData, query: web::Query<UserSearchQuery>) -> Response {
    let users = User::search(data, &query).await?;

    Ok(HttpResponse::Ok().json(users))
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

#[derive(Deserialize)]
pub struct PostQ {
    pub token: Option<String>,
}

#[post("")]
pub async fn create_user(
    app: web::Data<App>,
    client_session: Session,
    mut user: web::Json<User>,
    q: web::Query<PostQ>,
) -> Response {
    let token = if let Some(token) = &q.token {
        Token::find_by_id(token.clone()).execute(&app.db_session).await.ok()
    } else {
        None
    };

    if let Some(token) = &token {
        if token.email != user.email {
            return Err(NodecosmosError::Unauthorized("Email from token does not match"));
        }

        if TokenType::from(token.token_type.parse()?) != TokenType::Invitation {
            return Err(NodecosmosError::Unauthorized("Invalid token type"));
        }

        // token is created within the last 24 hours so we can safely confirm the user
        if token.created_at >= (chrono::Utc::now() - chrono::Duration::days(1)) {
            user.ctx = Some(UserContext::ConfirmInvitationTokenValid);
        }
    }

    let res = user.insert_cb(&app).execute(&app.db_session).await;

    match res {
        Ok(_) => {
            if user.is_confirmed {
                let current_user = CurrentUser::from_user(user.into_inner());

                // we can set the current user session as user is created from the email invitation
                set_current_user(&client_session, &current_user)?;

                return Ok(HttpResponse::Ok().json(current_user));
            }

            return Ok(HttpResponse::Ok().finish());
        }
        Err(e) => {
            if let NodecosmosError::EmailAlreadyExists = e {
                // for security reasons, we don't want to leak the fact that the email is already taken, we just
                // print that they can continue with the login from the email
                Ok(HttpResponse::Ok().finish())
            } else {
                Err(e)
            }
        }
    }
}

#[post("/confirm_email/{token}")]
pub async fn confirm_user_email(token: web::Path<String>, app: web::Data<App>) -> Response {
    let token = Token::find_by_id(token.into_inner()).execute(&app.db_session).await?;

    if token.expires_at < chrono::Utc::now() {
        return Err(NodecosmosError::Unauthorized("Token expired"));
    }

    let mut user = ConfirmUser::find_first_by_email(token.email)
        .execute(&app.db_session)
        .await?;

    user.is_confirmed = true;

    user.update_cb(&app).execute(&app.db_session).await?;

    Ok(HttpResponse::Ok().finish())
}

#[post("/resend_confirmation_email")]
pub async fn resend_confirmation_email(data: RequestData) -> Response {
    let user = User::find_first_by_email(data.current_user.email.clone())
        .execute(data.db_session())
        .await?;

    if user.is_confirmed {
        return Err(NodecosmosError::Unauthorized("User is already confirmed"));
    }

    if user.resend_token_count(data.db_session()).await? > 5 {
        return Err(NodecosmosError::Unauthorized(
            "Resend Token limit reached. Please contact support: support@nodecosmos.com",
        ));
    }

    UserCounter {
        id: user.id,
        ..Default::default()
    }
    .increment_resend_token_count(1)
    .execute(data.db_session())
    .await?;

    let token = user.generate_confirmation_token(data.db_session()).await?;

    data.mailer()
        .send_confirm_user_email(user.email, user.username, token.id)
        .await?;

    Ok(HttpResponse::Ok().finish())
}

#[post("/reset_password_email/{email}")]
pub async fn reset_password_email(app: web::Data<App>, email: web::Path<String>) -> Response {
    let user_res = User::find_first_by_email(email.into_inner())
        .execute(&app.db_session)
        .await
        .map_err(|e| {
            log::warn!("Error finding user by email for pass reset: {:?}", e);

            e
        });

    match user_res {
        Ok(user) => {
            user.reset_password_token(&app).await?;

            Ok(HttpResponse::Ok().finish())
        }
        Err(_) => {
            // we don't want to leak the fact that the email is not found, we just
            // print that if the email is found, an email will be sent
            return Ok(HttpResponse::Ok().finish());
        }
    }
}

#[derive(Deserialize)]
pub struct UpdatePassword {
    pub password: String,
    pub token: String,
}

#[put("/update_password")]
pub async fn update_password(app: web::Data<App>, pass: web::Json<UpdatePassword>) -> Response {
    let pass_data = pass.into_inner();
    let token = Token::find_by_id(pass_data.token).execute(&app.db_session).await?;

    if token.expires_at < chrono::Utc::now() {
        return Err(NodecosmosError::Unauthorized(
            "Token expired! Please request a new one.",
        ));
    }

    let mut user = User::find_first_by_email(token.email).execute(&app.db_session).await?;

    user.password = pass_data.password;
    user.hash_password()?;

    user.update_cb(&app).execute(&app.db_session).await?;

    Ok(HttpResponse::Ok().finish())
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
