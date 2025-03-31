use crate::api::current_user::OptCurrentUser;
use actix_web::{delete, get, post, put, web, HttpResponse};
use charybdis::model::AsNative;
use charybdis::operations::{DeleteWithCallbacks, InsertWithCallbacks, UpdateWithCallbacks};
use charybdis::types::Uuid;
use scylla::client::caching_session::CachingSession;
use serde::Deserialize;
use serde_json::json;

use crate::api::data::RequestData;
use crate::api::types::Response;
use crate::models::comment::{Comment, DeleteComment, UpdateContentComment};
use crate::models::comment_thread::CommentThread;
use crate::models::node::AuthNode;
use crate::models::traits::Authorization;

#[get("/threads/{root_id}/{branch_id}/{object_id}")]
pub async fn get_threads(
    db_session: web::Data<CachingSession>,
    opt_cu: OptCurrentUser,
    pk: web::Path<(Uuid, Uuid, Uuid)>,
) -> Response {
    let (root_id, branch_id, object_id) = pk.into_inner();
    AuthNode::auth_view(&db_session, &opt_cu, branch_id, object_id, root_id).await?;

    let threads = CommentThread::find_by_branch_id_and_object_id(branch_id, object_id)
        .execute(&db_session)
        .await?
        .try_collect()
        .await?;

    Ok(HttpResponse::Ok().json(threads))
}

#[get("/{root_id}/{branch_id}/{object_id}/{thread_id}")]
pub async fn get_thread_comments(
    db_session: web::Data<CachingSession>,
    opt_cu: OptCurrentUser,
    pk: web::Path<(Uuid, Uuid, Uuid, Uuid)>,
) -> Response {
    let (root_id, branch_id, object_id, thread_id) = pk.into_inner();
    AuthNode::auth_view(&db_session, &opt_cu, branch_id, object_id, root_id).await?;

    let comments = Comment::find_by_branch_id_and_thread_id(branch_id, thread_id)
        .execute(&db_session)
        .await?
        .try_collect()
        .await?;
    let thread = CommentThread::find_by_branch_id_and_object_id_and_id(branch_id, object_id, thread_id)
        .execute(&db_session)
        .await?;

    Ok(HttpResponse::Ok().json(json! {
        {
            "comments": comments,
            "thread": thread,
        }
    }))
}

#[derive(Deserialize)]
pub struct CreateCommentPayload {
    #[serde(rename = "newThread")]
    pub new_thread: Option<CommentThread>,
    pub comment: Comment,
}

#[post("")]
pub async fn create_comment(data: RequestData, payload: web::Json<CreateCommentPayload>) -> Response {
    let payload = payload.into_inner();
    let mut comment = payload.comment;

    if let Some(mut thread) = payload.new_thread {
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

#[delete("/thread/{branch_id}/{object_id}/{thread_id}")]
pub async fn delete_thread(data: RequestData, pk: web::Path<(Uuid, Uuid, Uuid)>) -> Response {
    let (branch_id, object_id, thread_id) = pk.into_inner();

    let mut thread = CommentThread::find_by_branch_id_and_object_id_and_id(branch_id, object_id, thread_id)
        .execute(data.db_session())
        .await?;

    thread.auth_update(&data).await?;

    thread.delete_cb(&data).execute(data.db_session()).await?;

    Ok(HttpResponse::NoContent().finish())
}

#[delete("/{branchId}/{threadId}/{objectId}/{id}")]
pub async fn delete_comment(data: RequestData, mut comment: web::Path<DeleteComment>) -> Response {
    comment.as_native().auth_update(&data).await?;

    comment.delete_cb(&data).execute(data.db_session()).await?;

    Ok(HttpResponse::NoContent().finish())
}
