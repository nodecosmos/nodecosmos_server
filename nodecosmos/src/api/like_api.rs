use actix_web::{delete, get, post, web, HttpResponse};
use charybdis::operations::{DeleteWithCallbacks, Find, InsertWithCallbacks};
use scylla::client::caching_session::CachingSession;
use serde_json::json;

use crate::api::data::RequestData;
use crate::api::types::Response;
use crate::models::like::{Like, PkLike};
use crate::models::materialized_views::likes_by_user::LikesByUser;
use crate::models::user::CurrentUser;

#[get("/{objectId}/{branchId}")]
pub async fn get_like_count(db_session: web::Data<CachingSession>, like: web::Path<PkLike>) -> Response {
    let first_like = Like::maybe_find_first_by_object_id_and_branch_id(like.object_id, like.branch_id)
        .execute(&db_session)
        .await?;

    if let Some(first_like) = first_like {
        let like_count = first_like.like_count(&db_session).await?;

        return Ok(HttpResponse::Ok().json(json!({
            "id": like.object_id,
            "branchId": like.branch_id,
            "likeCount": like_count,
        })));
    }

    Ok(HttpResponse::Ok().json(json!({
        "id": like.object_id,
        "branchId": like.branch_id,
        "likeCount": 0,
    })))
}

#[post("")]
pub async fn create_like(data: RequestData, mut like: web::Json<Like>) -> Response {
    like.insert_cb(&data).execute(data.db_session()).await?;

    Ok(HttpResponse::Ok().finish())
}

#[delete("/{objectId}/{branchId}/{userId}")]
pub async fn delete_like(data: RequestData, like: web::Path<PkLike>) -> Response {
    let mut like = Like::find_by_object_id_and_branch_id_and_user_id(like.object_id, like.branch_id, like.user_id)
        .execute(data.db_session())
        .await?;

    if like.user_id != data.current_user.id {
        return Ok(HttpResponse::Forbidden().finish());
    }

    like.delete_cb(&data).execute(data.db_session()).await?;

    Ok(HttpResponse::Ok().finish())
}

#[get("/user_likes")]
pub async fn user_likes(db_session: web::Data<CachingSession>, current_user: CurrentUser) -> Response {
    let user_likes = LikesByUser {
        user_id: current_user.id,
        ..Default::default()
    }
    .find_by_partition_key()
    .execute(&db_session)
    .await?
    .try_collect()
    .await?;

    Ok(HttpResponse::Ok().json(user_likes))
}
