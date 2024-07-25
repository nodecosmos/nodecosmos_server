use crate::api::current_user::OptCurrentUser;
use crate::api::data::RequestData;
use crate::api::types::Response;
use crate::models::attachment::Attachment;
use crate::models::node::AuthNode;
use crate::models::traits::s3::S3;
use actix_multipart::Multipart;
use actix_web::{get, post, web, HttpResponse};
use charybdis::operations::InsertWithCallbacks;
use charybdis::types::Uuid;
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
pub struct ImageAttachmentParams {
    pub branch_id: Uuid,
    pub node_id: Uuid,
    pub object_id: Uuid,
    pub root_id: Uuid,
}

#[post("/{branch_id}/{node_id}/{root_id}/{object_id}/upload_image")]
pub async fn upload_image(params: web::Path<ImageAttachmentParams>, data: RequestData, payload: Multipart) -> Response {
    // we authorize view as image can be uploaded in comment editor
    AuthNode::auth_view(
        data.db_session(),
        &OptCurrentUser(Some(data.current_user.clone())),
        params.branch_id,
        params.node_id,
        params.root_id,
    )
    .await?;

    let attachment = Attachment::create_image(&params, &data, payload).await?;

    Ok(HttpResponse::Ok().json(attachment))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentParams {
    branch_id: Uuid,
    node_id: Uuid,
    object_id: Uuid,
    root_id: Uuid,
    filename: String,
}

#[get("/presigned_url")]
pub async fn get_presigned_url(params: web::Query<AttachmentParams>, data: RequestData) -> Response {
    // we authorize view as image can be uploaded in comment editor
    AuthNode::auth_view(
        data.db_session(),
        &OptCurrentUser(Some(data.current_user.clone())),
        params.branch_id,
        params.node_id,
        params.root_id,
    )
    .await?;

    let url = Attachment::get_presigned_url(&data, &params.object_id, &params.filename).await?;

    Ok(HttpResponse::Ok().json(json!({
        "url": url,
        "filename": params.filename,
        "nodeId": params.node_id,
        "objectId": params.object_id,
    })))
}

#[post("")]
pub async fn create_attachment(mut attachment: web::Json<Attachment>, data: RequestData) -> Response {
    AuthNode::auth_update(&data, attachment.branch_id, attachment.node_id, attachment.root_id).await?;

    attachment.user_id = Some(data.current_user.id);

    attachment.insert_cb(&None).execute(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(attachment))
}
