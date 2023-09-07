use crate::app::CbExtension;
use crate::errors::NodecosmosError;
use crate::models::node::{Node, UpdateNodeCoverImage};
use crate::services::aws::s3::upload_to_s3;
use crate::services::image::*;
use actix_multipart::Multipart;
use charybdis::UpdateWithExtCallbacks;
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
) -> Result<Vec<u8>, NodecosmosError> {
    if let Some(item) = payload.next().await {
        let mut field = item.map_err(|e| {
            NodecosmosError::InternalServerError(format!("Failed to read multipart field: {:?}", e))
        })?;

        let buffer = read_image_buffer(&mut field).await?;
        let decoded_img = decode_image(&buffer)?;
        let resized_image = resize_image(decoded_img, IMG_WIDTH, IMG_HEIGHT)?;
        let rgb_image = convert_image_to_rgb(resized_image)?;
        let mut compressed = rgb_image.to_vec();

        if rgb_image.len() > TARGET_SIZE_IN_BYTES {
            compressed = compress_image(rgb_image)?;
        }

        let bucket = nc_app.config["aws"]["bucket"]
            .as_str()
            .expect("Missing bucket");
        let file_name = format!("{}/cover.jpeg", node.id);

        let url = format!("https://{}.s3.amazonaws.com/{}", bucket, file_name);

        upload_to_s3(
            s3_client,
            compressed.clone(),
            nc_app.config["aws"]["bucket"]
                .as_str()
                .expect("Missing bucket"),
            &file_name,
        )
        .await?;

        let mut update_node_cover_img = UpdateNodeCoverImage {
            id: node.id,
            root_id: node.root_id,
            cover_image: Some(url),
            updated_at: None,
        };

        update_node_cover_img
            .update_cb(db_session, cb_extension)
            .await?;

        return Ok(compressed);
    }

    Err(NodecosmosError::InternalServerError(
        "Failed to read multipart field".to_string(),
    ))
}
