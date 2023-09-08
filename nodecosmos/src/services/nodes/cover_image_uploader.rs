use crate::app::CbExtension;
use crate::errors::NodecosmosError;
use crate::models::node::{Node, UpdateNodeCoverImage};
use crate::services::aws::s3::{delete_s3_object, upload_s3_object};
use crate::services::image::*;
use actix_multipart::Multipart;
use charybdis::{New, UpdateWithExtCallbacks};
use futures::StreamExt;

const IMG_WIDTH: u32 = 850;
const IMG_HEIGHT: u32 = 375;
const TARGET_SIZE_IN_BYTES: usize = 15 * 1024;

pub async fn handle_cover_image_upload(
    mut payload: Multipart,
    s3_client: &aws_sdk_s3::Client,
    nc_app: &crate::NodecosmosApp,
    node: Node,
    db_session: &charybdis::CachingSession,
    cb_extension: &CbExtension,
) -> Result<String, NodecosmosError> {
    if let Some(item) = payload.next().await {
        let mut field = item.map_err(|e| {
            NodecosmosError::InternalServerError(format!("Failed to read multipart field: {:?}", e))
        })?;

        println!("Field name: {}", field.name());

        let buffer = read_image_buffer(&mut field).await?;
        let decoded_img = decode_image(&buffer)?;
        let resized_image = resize_image(decoded_img, IMG_WIDTH, IMG_HEIGHT)?;
        let rgb_image = convert_image_to_rgb(resized_image)?;
        let mut compressed = rgb_image.to_vec();

        if rgb_image.len() > TARGET_SIZE_IN_BYTES {
            compressed = compress_image(rgb_image)?;
        }

        let timestamp = chrono::Utc::now().timestamp();

        if node.cover_image_url.is_some() {
            let key = node.cover_image_filename.ok_or_else(|| {
                NodecosmosError::InternalServerError("Missing cover image key".to_string())
            })?;

            delete_s3_object(s3_client, &nc_app.bucket, &key).await?;
        }

        let new_cover_image_filename = format!("{}/{}-cover.jpeg", node.id, timestamp);
        let url = format!(
            "https://{}.s3.amazonaws.com/{}",
            nc_app.bucket, new_cover_image_filename
        );

        upload_s3_object(
            s3_client,
            compressed.clone(),
            &nc_app.bucket,
            &new_cover_image_filename,
        )
        .await?;

        let mut update_node_cover_img = UpdateNodeCoverImage::new();
        update_node_cover_img.id = node.id;
        update_node_cover_img.root_id = node.root_id;
        update_node_cover_img.cover_image_url = Some(url.clone());
        update_node_cover_img.cover_image_filename = Some(new_cover_image_filename);

        update_node_cover_img
            .update_cb(db_session, cb_extension)
            .await?;

        return Ok(url);
    }

    Err(NodecosmosError::InternalServerError(
        "Failed to read multipart field".to_string(),
    ))
}
