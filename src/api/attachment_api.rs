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
    pub node_id: Uuid,
    pub object_id: Uuid,
}

#[post("/{node_id}/{object_id}/upload_image")]
pub async fn upload_image(params: web::Path<ImageAttachmentParams>, data: RequestData, payload: Multipart) -> Response {
    AuthNode::auth_update(&data, params.node_id, params.node_id).await?;

    let attachment = Attachment::create_image(&params, &data, payload).await?;

    Ok(HttpResponse::Ok().json(attachment))
}

#[derive(Deserialize)]
pub struct AttachmentParams {
    #[serde(rename = "nodeId")]
    node_id: Uuid,

    #[serde(rename = "objectId")]
    object_id: Uuid,

    filename: String,
}

#[get("/presigned_url")]
pub async fn get_presigned_url(params: web::Query<AttachmentParams>, data: RequestData) -> Response {
    AuthNode::auth_update(&data, params.node_id, params.node_id).await?;

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
    AuthNode::auth_update(&data, attachment.node_id, attachment.node_id).await?;

    attachment.user_id = Some(data.current_user_id());

    attachment.insert_cb(&None).execute(data.db_session()).await?;

    Ok(HttpResponse::Ok().json(attachment))
}
