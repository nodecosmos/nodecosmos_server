use crate::actions::client_session::CurrentUser;
use crate::authorize::auth_node_update_by_id;
use crate::errors::NodecosmosError;
use crate::services::attachments::upload_image::{upload_image_attachment, AttachmentParams};
use actix_multipart::Multipart;
use actix_web::{post, web, HttpResponse};
use scylla::CachingSession;

#[post("/{node_id}/{object_id}/upload_image")]
pub async fn upload_image(
    params: web::Path<AttachmentParams>,
    db_session: web::Data<CachingSession>,
    nc_app: web::Data<crate::NodecosmosApp>,
    s3_client: web::Data<aws_sdk_s3::Client>,
    payload: Multipart,
    current_user: CurrentUser,
) -> Result<HttpResponse, NodecosmosError> {
    let params = params.into_inner();

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
