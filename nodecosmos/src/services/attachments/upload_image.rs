use crate::actions::client_session::CurrentUser;
use crate::errors::NodecosmosError;
use crate::models::attachment::Attachment;
use crate::services::aws::s3::upload_s3_object;
use crate::services::image::{
    compress_image, convert_image_to_rgb, decode_image, read_image_buffer, resize_image,
};
use actix_multipart::Multipart;
use charybdis::{InsertWithCallbacks, New, Uuid};
use futures::StreamExt;
use serde::Deserialize;

const TARGET_SIZE_IN_BYTES: usize = 15 * 1024;
const MAX_IMAGE_WIDTH: u32 = 852;

#[derive(Deserialize)]
pub struct ImageAttachmentParams {
    pub node_id: Uuid,
    pub object_id: Uuid,
}

pub async fn upload_image_attachment(
    params: &ImageAttachmentParams,
    nc_app: &crate::NodecosmosApp,
    s3_client: &aws_sdk_s3::Client,
    db_session: &charybdis::CachingSession,
    user: &CurrentUser,
    mut payload: Multipart,
) -> Result<Attachment, NodecosmosError> {
    if let Some(item) = payload.next().await {
        let mut field = item.map_err(|e| {
            NodecosmosError::InternalServerError(format!("Failed to read multipart field: {:?}", e))
        })?;

        let buffer = read_image_buffer(&mut field).await?;
        let decoded_img = decode_image(&buffer)?;

        let mut width = decoded_img.width();
        let mut height = decoded_img.height();

        if width > MAX_IMAGE_WIDTH {
            width = MAX_IMAGE_WIDTH;
            height =
                (width as f32 * (decoded_img.height() as f32 / decoded_img.width() as f32)) as u32;
        }

        let resized_image = resize_image(decoded_img, width, height)?;
        let rgb_image = convert_image_to_rgb(resized_image)?;
        let mut compressed = rgb_image.to_vec();

        if rgb_image.len() > TARGET_SIZE_IN_BYTES {
            compressed = compress_image(rgb_image)?;
        }

        let key = Attachment::build_s3_filename(params.object_id.to_string());
        let url = Attachment::build_s3_url(nc_app.bucket.clone(), key.clone());

        upload_s3_object(s3_client, compressed, &nc_app.bucket, &key).await?;

        let mut attachment = Attachment::new();
        attachment.node_id = params.node_id;
        attachment.object_id = params.object_id;
        attachment.key = key;
        attachment.url = Some(url);
        attachment.user_id = Some(user.id);

        attachment.insert_cb(db_session).await?;

        return Ok(attachment);
    }

    Err(NodecosmosError::InternalServerError(
        "No image found in multipart request".to_string(),
    ))
}
