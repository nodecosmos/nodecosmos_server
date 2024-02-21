use crate::api::data::RequestData;
use crate::api::types::Response;
use crate::models::comment::{Comment, PkComment};
use crate::models::traits::Authorization;
use actix_web::{get, post, web, HttpResponse};
use charybdis::model::AsNative;
use charybdis::operations::{Delete, DeleteWithCallbacks, Find, InsertWithCallbacks};
use scylla::CachingSession;

#[get("/{object_id}/{branchId}")]
pub async fn get_comments(db_session: web::Data<CachingSession>, pk: web::Path<PkComment>) -> Response {
    let comments = Comment::find_by_object_id_and_branch_id(pk.object_id, pk.branch_id)
        .execute(&db_session)
        .await?
        .try_collect()
        .await?;

    Ok(HttpResponse::Ok().json(comments))
}

#[post("")]
pub async fn create_comment(data: web::Data<RequestData>, mut comment: web::Json<Comment>) -> Response {
    comment.auth_creation(&data).await?;

    comment.insert_cb(&None).execute(data.db_session()).await?;

    Ok(HttpResponse::Created().json(comment))
}

#[get("/{object_id}/{branch_id}/{id}")]
pub async fn delete_comment(data: web::Data<RequestData>, comment: web::Path<PkComment>) -> Response {
    comment.as_native().auth_update(&data).await?;

    comment.delete().execute(data.db_session()).await?;

    Ok(HttpResponse::NoContent().finish())
}
