use crate::client_session::{CurrentUser, OptCurrentUser};
use crate::errors::NodecosmosError;
use crate::models::like::{Like, ObjectTypes};
use crate::models::likes_count::LikesCount;
use crate::models::user::LikedObjectIdsUser;
use crate::CbExtension;
use actix_web::{delete, get, post, web, HttpResponse};
use charybdis::{DeleteWithExtCallbacks, Find, InsertWithExtCallbacks, New, Uuid};
use scylla::CachingSession;
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Deserialize)]
pub struct LikeParams {
    object_type: ObjectTypes,
    object_id: Uuid,
}

#[get("/{id}")]
pub async fn get_likes_count(
    db_session: web::Data<CachingSession>,
    object_id: web::Path<Uuid>,
    opt_current_user: OptCurrentUser,
) -> Result<HttpResponse, NodecosmosError> {
    let object_id = object_id.into_inner();

    let mut liked_by_current_user = false;

    if let Some(opt_current_user) = opt_current_user.0 {
        let mut cu_like = Like::new();
        cu_like.object_id = object_id;
        cu_like.user_id = opt_current_user.id;

        if cu_like
            .find_by_primary_key(&db_session)
            .await
            .ok()
            .is_some()
        {
            liked_by_current_user = true;
        }
    }

    let likes_count = LikesCount {
        object_id,
        ..Default::default()
    };

    let likes_count = likes_count.find_by_primary_key(&db_session).await.ok();

    match likes_count {
        Some(likes_count) => Ok(HttpResponse::Ok().json(json!({
            "id": object_id,
            "likesCount": likes_count.count,
            "likedByCurrentUser": liked_by_current_user,
        }))),
        None => Ok(HttpResponse::Ok().json(json!({
            "id": object_id,
            "likesCount": 0,
            "likedByCurrentUser": liked_by_current_user,
        }))),
    }
}

#[post("")]
pub async fn create_like(
    db_session: web::Data<CachingSession>,
    cb_extension: web::Data<CbExtension>,
    params: web::Json<LikeParams>,
    current_user: CurrentUser,
) -> Result<HttpResponse, NodecosmosError> {
    let mut like = Like {
        object_id: params.object_id,
        object_type: params.object_type.to_string(),
        user_id: current_user.id,
        username: current_user.username,
        ..Default::default()
    };

    like.insert_cb(&db_session, &cb_extension).await?;

    let likes_count = LikesCount {
        object_id: params.object_id,
        ..Default::default()
    };

    let likes_count = likes_count.find_by_primary_key(&db_session).await?;

    Ok(HttpResponse::Ok().json(json!({
        "id": params.object_id,
        "likedByCurrentUser": true,
        "likesCount": likes_count.count,
    })))
}

#[delete("/{id}")]
pub async fn delete_like(
    db_session: web::Data<CachingSession>,
    cb_extension: web::Data<CbExtension>,
    id: web::Path<Uuid>,
    current_user: CurrentUser,
) -> Result<HttpResponse, NodecosmosError> {
    let object_id = id.into_inner();

    let like: Like = Like {
        object_id,
        user_id: current_user.id,
        ..Default::default()
    };

    let mut like = like.find_by_primary_key(&db_session).await?;

    like.delete_cb(&db_session, &cb_extension).await?;

    let likes_count = LikesCount {
        object_id,
        ..Default::default()
    };
    let likes_count = likes_count.find_by_primary_key(&db_session).await?;

    Ok(HttpResponse::Ok().json(json!({
        "id": object_id,
        "likedByCurrentUser": false,
        "likesCount": likes_count.count,
    })))
}

#[get("/liked_object_ids")]
pub async fn liked_object_ids(
    db_session: web::Data<CachingSession>,
    current_user: CurrentUser,
) -> Result<HttpResponse, NodecosmosError> {
    let mut user = LikedObjectIdsUser::new();
    user.id = current_user.id;

    let user = user.find_by_primary_key(&db_session).await?;
    let liked_object_ids = user.liked_object_ids.unwrap_or(vec![]);

    Ok(HttpResponse::Ok().json(json!({
        "success": true,
        "likedObjectIds": liked_object_ids,
    })))
}
