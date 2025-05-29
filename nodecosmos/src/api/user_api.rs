use actix_multipart::Multipart;
use actix_session::Session;
use actix_web::{delete, get, post, put, web, HttpResponse};
use charybdis::model::AsNative;
use charybdis::operations::{DeleteWithCallbacks, Find, InsertWithCallbacks, UpdateWithCallbacks};
use charybdis::types::Uuid;
use futures::TryStreamExt;
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use scylla::client::caching_session::CachingSession;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::api::current_user::{refresh_current_user, remove_current_user, set_current_user, OptCurrentUser};
use crate::api::data::RequestData;
use crate::api::types::Response;
use crate::errors::NodecosmosError;
use crate::models::materialized_views::likes_by_user::LikesByUser;
use crate::models::materialized_views::nodes_by_owner::NodesByOwner;
use crate::models::node::{find_base_node, BaseNode};
use crate::models::token::{Token, TokenType};
use crate::models::traits::{Authorization, WhereInDoubleChunkedExec};
use crate::models::user::search::UserSearchQuery;
use crate::models::user::{
    ConfirmUser, CurrentUser, ShowUser, UpdateBioUser, UpdatePasswordUser, UpdateProfileImageUser, UpdateUsernameUser,
    User, UserContext, GOOGLE_LOGIN_PASSWORD,
};
use crate::models::user_counter::UserCounter;
use crate::models::utils::validate_recaptcha;
use crate::App;

#[derive(Deserialize)]
pub struct LoginForm {
    #[serde(rename = "usernameOrEmail")]
    pub username_or_email: String,
    pub password: String,

    /// Recaptcha token
    #[serde(rename = "rToken")]
    pub r_token: Option<String>,
}

#[post("/session/login")]
pub async fn login(
    client_session: Session,
    app: web::Data<App>,
    db_session: web::Data<CachingSession>,
    login_form: web::Json<LoginForm>,
) -> Response {
    validate_recaptcha(&app, &login_form.r_token).await?;

    let user;

    if let Some(user_by_username) = User::maybe_find_first_by_username(login_form.username_or_email.clone())
        .execute(&db_session)
        .await?
    {
        user = user_by_username;
    } else if let Some(user_by_email) = User::maybe_find_first_by_email(login_form.username_or_email.clone())
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

    let likes = LikesByUser {
        user_id: current_user.id,
        ..Default::default()
    }
    .find_by_partition_key()
    .execute(&db_session)
    .await?
    .try_collect()
    .await?;

    Ok(HttpResponse::Ok().json(json!({
        "user": current_user,
        "likes": likes
    })))
}

#[derive(Debug, Deserialize, Serialize)]
struct GoogleClaims {
    iss: String,
    sub: String,
    aud: String,
    exp: usize,
    iat: usize,
    email: String,
    name: String,
    email_verified: Option<bool>,
    family_name: String,
    given_name: String,
    picture: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct GoogleTokenResponse {
    access_token: String,
    expires_in: u64,
    scope: String,
    token_type: String,
    id_token: String,
    refresh_token: Option<String>,
}

#[derive(Deserialize)]
struct GoogleAuthRequest {
    code: String,
}

#[post("/google_auth")]
async fn handle_google_auth(
    body: web::Json<GoogleAuthRequest>,
    app: web::Data<App>,
    client_session: Session,
) -> Response {
    let code = body.code.clone();

    if let Some(google_cfg) = &app.google_cfg {
        // 1. Exchange the authorization code for tokens.
        let client = reqwest::Client::new();
        let redirect_uri = "postmessage".to_string();
        let grant_type = "authorization_code".to_string();
        let params = [
            ("client_id", &google_cfg.client_id),
            ("client_secret", &google_cfg.client_secret),
            ("code", &code),
            ("grant_type", &grant_type),
            ("redirect_uri", &redirect_uri),
        ];

        let token_resp = client
            .post("https://oauth2.googleapis.com/token")
            .form(&params)
            .send()
            .await
            .map_err(|e| {
                log::error!("Failed to exchange authorization code: {}", e);

                NodecosmosError::BadRequest("Failed to exchange authorization code".into())
            })?;

        if !token_resp.status().is_success() {
            log::error!("Failed to exchange authorization code {:?}", token_resp);

            // log body
            let body = token_resp.text().await.unwrap_or_default();
            log::error!("Response Body: {}", body);

            return Err(NodecosmosError::BadRequest(
                "Failed to exchange authorization code".into(),
            ));
        }

        let token_json: GoogleTokenResponse = token_resp.json().await.map_err(|e| {
            log::error!("Failed to parse token response: {:?}", e);

            NodecosmosError::InternalServerError(e.to_string())
        })?;

        // 2. Verify the ID token.
        let id_token = token_json.id_token;
        let header =
            decode_header(&id_token).map_err(|_| NodecosmosError::BadRequest("Invalid token header".into()))?;
        let kid = header
            .kid
            .ok_or(NodecosmosError::BadRequest("kid not found in token header".into()))?;

        // Fetch Google's public keys.
        let certs_resp = client
            .get("https://www.googleapis.com/oauth2/v3/certs")
            .send()
            .await
            .map_err(|_| NodecosmosError::BadRequest("Failed to fetch Google certs".into()))?;

        if !certs_resp.status().is_success() {
            return Err(NodecosmosError::BadRequest("Failed to fetch Google certs".into()));
        }

        let certs: serde_json::Value = certs_resp
            .json()
            .await
            .map_err(|e| NodecosmosError::InternalServerError(e.to_string()))?;
        let keys = certs["keys"]
            .as_array()
            .ok_or(NodecosmosError::BadRequest("keys not found".into()))?;

        let matching_key = keys
            .iter()
            .find(|key| key["kid"].as_str() == Some(&kid))
            .ok_or(NodecosmosError::BadRequest("Matching key not found".into()))?;
        let n = matching_key["n"]
            .as_str()
            .ok_or(NodecosmosError::BadRequest("n not found".into()))?;
        let e = matching_key["e"]
            .as_str()
            .ok_or(NodecosmosError::BadRequest("e not found".into()))?;
        let decoding_key = DecodingKey::from_rsa_components(n, e)
            .map_err(|_| NodecosmosError::BadRequest("Failed to create decoding key".into()))?;

        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_issuer(&["https://accounts.google.com", "accounts.google.com"]);
        validation.set_audience(&[&google_cfg.client_id]);

        let decoded = decode::<GoogleClaims>(&id_token, &decoding_key, &validation)
            .map_err(|_| NodecosmosError::BadRequest("Token verification failed".into()))?;

        if let Some(current_user) = CurrentUser::maybe_find_first_by_email(decoded.claims.email.clone())
            .execute(&app.db_session)
            .await?
        {
            set_current_user(&client_session, &current_user)?;

            let likes = LikesByUser {
                user_id: current_user.id,
                ..Default::default()
            }
            .find_by_partition_key()
            .execute(&app.db_session)
            .await?
            .try_collect()
            .await?;

            Ok(HttpResponse::Ok().json(json!({
                "isExistingUser": true,
                "user": current_user,
                "likes": likes
            })))
        } else {
            let id = Uuid::new_v4();
            let mut user = User {
                id,
                email: decoded.claims.email.clone(),
                username: id.to_string(),
                first_name: decoded.claims.given_name.clone(),
                last_name: decoded.claims.family_name.clone(),
                bio: None,
                address: None,
                profile_image_filename: None,
                profile_image_url: None,
                is_confirmed: true,
                is_blocked: false,
                editor_at_nodes: None,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                password: GOOGLE_LOGIN_PASSWORD.to_string(),
                ctx: Some(UserContext::GoogleSignUp),
                redirect: None,
            };

            user.insert_cb(&app).execute(&app.db_session).await?;

            let current_user = CurrentUser::from_user(user);

            set_current_user(&client_session, &current_user)?;

            Ok(HttpResponse::Ok().json(json!({
                "isExistingUser": false,
                "user": current_user,
                "likes": []
            })))
        }
    } else {
        Err(NodecosmosError::InternalServerError(
            "Google login not configured".into(),
        ))
    }
}

#[put("/update_username")]
pub async fn update_username(
    data: RequestData,
    client_session: Session,
    mut user: web::Json<UpdateUsernameUser>,
) -> Response {
    user.as_native().auth_update(&data).await?;

    user.update_cb(&data).execute(data.db_session()).await?;

    refresh_current_user(&client_session, data.db_session()).await?;

    Ok(HttpResponse::Ok().json(user))
}

#[get("/session/sync")]
pub async fn sync(data: RequestData, client_session: Session, db_session: web::Data<CachingSession>) -> Response {
    let current_user = data.current_user.find_by_primary_key().execute(data.db_session()).await;

    match current_user {
        Ok(user) => {
            set_current_user(&client_session, &user)?;

            let likes = LikesByUser {
                user_id: user.id,
                ..Default::default()
            }
            .find_by_partition_key()
            .execute(&db_session)
            .await?
            .try_collect()
            .await?;

            Ok(HttpResponse::Ok().json(json!({
                "user": user,
                "likes": likes
            })))
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

#[get("/search/user")]
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
pub async fn get_user_by_username(
    db_session: web::Data<CachingSession>,
    opt_cu: OptCurrentUser,
    username: web::Path<String>,
) -> Response {
    let user = ShowUser::find_first_by_username(username.into_inner())
        .execute(&db_session)
        .await?;
    let is_current_user = opt_cu.0.as_ref().is_some_and(|cu| cu.id == user.id);
    let root_nodes = NodesByOwner::root_nodes(&db_session, &opt_cu, user.id)
        .await?
        .try_filter_map(|node| async move {
            if node.branch_id == node.root_id && (node.is_public || is_current_user) {
                Ok(Some(node))
            } else {
                Ok(None)
            }
        })
        .try_collect::<Vec<NodesByOwner>>()
        .await?;

    let mut editor_at_nodes = vec![];
    if let Some(editor_at_node_ids) = &user.editor_at_nodes {
        editor_at_nodes = editor_at_node_ids
            .double_cond_where_in_chunked_query(&db_session, |branch_and_node_ids| {
                let (branch_ids, node_ids): (Vec<Uuid>, Vec<Uuid>) =
                    branch_and_node_ids.iter().map(|bn_ids| (bn_ids[0], bn_ids[1])).unzip();

                find_base_node!("branch_id in ? and id in ? ALLOW FILTERING", (branch_ids, node_ids))
            })
            .await
            .try_filter_map(|node| async move {
                if node.is_public || is_current_user {
                    Ok(Some(node))
                } else {
                    Ok(None)
                }
            })
            .try_collect()
            .await?;
    }

    Ok(HttpResponse::Ok().json(json!({
        "user": user,
        "rootNodes": root_nodes,
        "editorAtNodes": editor_at_nodes
    })))
}

#[derive(Deserialize)]
pub struct PostQ {
    pub token: Option<String>,

    /// Recaptcha token
    #[serde(rename = "rToken")]
    pub r_token: Option<String>,
}

#[post("")]
pub async fn create_user(
    app: web::Data<App>,
    client_session: Session,
    mut user: web::Json<User>,
    q: web::Query<PostQ>,
) -> Response {
    validate_recaptcha(&app, &q.r_token).await?;

    let token = if let Some(token) = &q.token {
        Token::find_by_id(token.clone()).execute(&app.db_session).await.ok()
    } else {
        None
    };

    if let Some(token) = &token {
        if token.email != user.email {
            return Err(NodecosmosError::Unauthorized("Email from token does not match"));
        }

        if token.token_type.parse::<TokenType>()? != TokenType::Invitation {
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

            Ok(HttpResponse::Ok().finish())
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

    Ok(HttpResponse::Ok().json(user))
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
        .send_confirm_user_email(&user.email, &user.username, &token.id, &user.redirect)
        .await?;

    Ok(HttpResponse::Ok().finish())
}

#[derive(Deserialize)]
pub struct ResetPassword {
    pub email: String,
}

#[post("/reset_password")]
pub async fn reset_password_email(app: web::Data<App>, data: web::Json<ResetPassword>) -> Response {
    let user_res = User::find_first_by_email(data.into_inner().email)
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
            Ok(HttpResponse::Ok().finish())
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
            "Token expired. Please request a new password.",
        ));
    }

    let mut user = UpdatePasswordUser::find_first_by_email(token.email)
        .execute(&app.db_session)
        .await?;

    user.password = pass_data.password;

    user.update_cb(&app).execute(&app.db_session).await?;

    Ok(HttpResponse::Ok().json(json!({"email": user.email })))
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
