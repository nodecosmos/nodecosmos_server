use crate::api::data::RequestData;
use crate::api::types::Response;
use crate::models::comment::{Comment, DeleteComment, PkComment, UpdateContentComment};
use crate::models::comment_thread::CommentThread;
use crate::models::traits::Authorization;
use actix_web::{delete, get, post, put, web, HttpResponse};
use charybdis::model::AsNative;
use charybdis::operations::{Delete, DeleteWithCallbacks, Find, Insert, InsertWithCallbacks, UpdateWithCallbacks};
use scylla::CachingSession;
use serde::Deserialize;
use serde_json::json;

#[get("/{objectId}")]
pub async fn get_comments(db_session: web::Data<CachingSession>, pk: web::Path<PkComment>) -> Response {
    let comments = Comment::find_by_partition_key_value(&(pk.object_id,))
        .execute(&db_session)
        .await?
        .try_collect()
        .await?;
    let threads = CommentThread::find_by_partition_key_value(&(pk.object_id,))
        .execute(&db_session)
        .await?
        .try_collect()
        .await?;

    Ok(HttpResponse::Ok().json(json! {
        {
            "comments": comments,
            "threads": threads,
        }
    }))
}

#[get("/{objectId}/{threadId}")]
pub async fn get_thread_comments(db_session: web::Data<CachingSession>, pk: web::Path<PkComment>) -> Response {
    let comments = Comment::find_by_object_id_and_thread_id(pk.object_id, pk.thread_id)
        .execute(&db_session)
        .await?
        .try_collect()
        .await?;

    Ok(HttpResponse::Ok().json(comments))
}

#[derive(Deserialize)]
pub struct CreateCommentPayload {
    pub thread: Option<CommentThread>,
    pub comment: Comment,
}

#[post("")]
pub async fn create_comment(data: RequestData, payload: web::Json<CreateCommentPayload>) -> Response {
    let payload = payload.into_inner();
    let mut comment = payload.comment;

    if let Some(mut thread) = payload.thread {
        thread.auth_creation(&data).await?;
        thread.insert_cb(&data).execute(data.db_session()).await?;
        comment.assign_thread(thread.clone());
    } else {
        comment.auth_creation(&data).await?;
    }

    comment.insert_cb(&data).execute(data.db_session()).await?;

    Ok(HttpResponse::Created().json(json! {
        {
            "comment": comment,
            "thread": comment.thread,
        }
    }))
}

#[put("/content")]
pub async fn update_comment_content(data: RequestData, mut comment: web::Json<UpdateContentComment>) -> Response {
    comment.as_native().auth_update(&data).await?;

    comment.update_cb(&None).execute(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(comment))
}

#[delete("/{objectId}/{threadId}/{id}")]
pub async fn delete_comment(data: RequestData, mut comment: web::Path<DeleteComment>) -> Response {
    comment.as_native().auth_update(&data).await?;

    comment.delete_cb(&data).execute(data.db_session()).await?;

    Ok(HttpResponse::NoContent().finish())
}
