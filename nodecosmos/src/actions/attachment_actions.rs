use crate::authorize::auth_node_update_by_id;
use crate::errors::NodecosmosError;
use crate::models::attachment::Attachment;
use crate::models::user::CurrentUser;
use crate::services::attachments::upload_image::{upload_image_attachment, ImageAttachmentParams};
use crate::services::aws::s3::get_s3_presigned_url;
use actix_multipart::Multipart;
use actix_web::{get, post, web, HttpResponse};
use charybdis::operations::InsertWithCallbacks;
use charybdis::types::Uuid;
use scylla::CachingSession;
use serde::Deserialize;
use serde_json::json;

#[post("/{node_id}/{object_id}/upload_image")]
pub async fn upload_image(
    params: web::Path<ImageAttachmentParams>,
    db_session: web::Data<CachingSession>,
    nc_app: web::Data<crate::NodecosmosApp>,
    s3_client: web::Data<aws_sdk_s3::Client>,
    payload: Multipart,
    current_user: CurrentUser,
) -> Result<HttpResponse, NodecosmosError> {
    auth_node_update_by_id(&params.node_id, &db_session, &current_user).await?;

    let attachment = upload_image_attachment(
        &params,
        &nc_app,
        &s3_client,
        &db_session,
        &current_user,
        payload,
    )
    .await?;

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
pub async fn get_presigned_url(
    params: web::Query<AttachmentParams>,
    db_session: web::Data<CachingSession>,
    nc_app: web::Data<crate::NodecosmosApp>,
    s3_client: web::Data<aws_sdk_s3::Client>,
    current_user: CurrentUser,
) -> Result<HttpResponse, NodecosmosError> {
    let bucket = &nc_app.bucket;

    auth_node_update_by_id(&params.node_id, &db_session, &current_user).await?;

    let url = get_s3_presigned_url(&s3_client, bucket, &params.filename).await?;

    Ok(HttpResponse::Ok().json(json!({
        "url": url,
        "filename": params.filename,
        "nodeId": params.node_id,
        "objectId": params.object_id,
    })))
}

#[post("")]
pub async fn create_attachment(
    mut attachment: web::Json<Attachment>,
    db_session: web::Data<CachingSession>,
    nc_app: web::Data<crate::NodecosmosApp>,
    current_user: CurrentUser,
) -> Result<HttpResponse, NodecosmosError> {
    let url = Attachment::build_s3_url(nc_app.bucket.clone(), attachment.key.clone());

    attachment.url = Some(url);
    attachment.user_id = Some(current_user.id);

    attachment.insert_cb(&db_session).await?;

    Ok(HttpResponse::Ok().json(attachment))
}
