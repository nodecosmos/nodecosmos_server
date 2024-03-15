use crate::api::data::RequestData;
use crate::api::types::Response;
use crate::models::like::Like;
use crate::models::materialized_views::likes_by_user::LikesByUser;
use crate::models::user::CurrentUser;
use actix_web::{delete, get, post, web, HttpResponse};
use charybdis::operations::{DeleteWithCallbacks, Find, InsertWithCallbacks};
use scylla::CachingSession;
use serde_json::json;

#[get("/{objectId}/{branchId}")]
pub async fn get_like_count(db_session: web::Data<CachingSession>, like: web::Path<Like>) -> Response {
    let like_count = match like.find_by_primary_key().execute(&db_session).await.ok() {
        Some(mut like) => like.like_count(&db_session).await?,
        None => 0,
    };

    Ok(HttpResponse::Ok().json(json!({
        "id": like.object_id,
        "branchId": like.branch_id,
        "likesCount": like_count,
    })))
}

#[post("")]
pub async fn create_like(data: RequestData, mut like: web::Json<Like>) -> Response {
    like.user_id = data.current_user.id;
    like.username = data.current_user.username.clone();

    like.insert_cb(&data).execute(data.db_session()).await?;

    let like_count = like.like_count(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(json!({
        "id": like.object_id,
        "branchId": like.branch_id,
        "likesCount": like_count,
    })))
}

#[delete("/{objectId}/{branchId}")]
pub async fn delete_like(data: RequestData, mut like: web::Path<Like>) -> Response {
    like.user_id = data.current_user_id();
    let mut like = like.find_by_primary_key().execute(data.db_session()).await?;

    like.delete_cb(&data).execute(data.db_session()).await?;

    let like_count = like.like_count(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(json!({
        "id": like.object_id,
        "branchId": like.branch_id,
        "likesCount": like_count,
    })))
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
