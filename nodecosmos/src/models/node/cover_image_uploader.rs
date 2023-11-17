use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::node::{Node, UpdateCoverImageNode};
use crate::services::aws::s3::{delete_s3_object, upload_s3_object};
use crate::services::image::Image;
use actix_multipart::Multipart;
use charybdis::operations::{New, UpdateWithExtCallbacks};
use futures::StreamExt;

const IMG_WIDTH: u32 = 850;
const IMG_HEIGHT: u32 = 375;

pub struct CoverImageUploader<'a> {
    pub data: &'a RequestData,
    pub payload: Multipart,
    pub node: &'a Node,
}

impl<'a> CoverImageUploader<'a> {
    pub async fn run(&mut self) -> Result<String, NodecosmosError> {
        if let Some(item) = self.payload.next().await {
            let mut field = item.map_err(|e| {
                NodecosmosError::InternalServerError(format!("Failed to read multipart field: {:?}", e))
            })?;

            let mut image = Image::from_field(&mut field).await?;
            image.resize_image(IMG_WIDTH, IMG_HEIGHT);
            let compressed = image.compressed()?;

            let timestamp = chrono::Utc::now().timestamp();

            if let Some(cover_image_filename) = &self.node.cover_image_filename {
                // delete s3 object asynchronously
                let bucket = self.data.app.s3_bucket.clone();
                let key = cover_image_filename.clone();
                let s3_client = self.data.app.s3_client.clone();
                tokio::spawn(async move {
                    let _ = delete_s3_object(&s3_client, &bucket, &key).await.map_err(|e| {
                        println!("Failed to delete existing cover image from S3: {:?}", e);
                    });
                });
            }

            let new_cover_image_filename = format!("{}/{}-cover.jpeg", self.node.id, timestamp);
            let url = format!(
                "https://{}.s3.amazonaws.com/{}",
                self.data.app.s3_bucket, new_cover_image_filename
            );

            upload_s3_object(
                &self.data.app.s3_client,
                compressed,
                &self.data.app.s3_bucket,
                &new_cover_image_filename,
            )
            .await?;

            let mut update_node_cover_img = UpdateCoverImageNode::new();
            update_node_cover_img.id = self.node.id;
            update_node_cover_img.cover_image_url = Some(url.clone());
            update_node_cover_img.cover_image_filename = Some(new_cover_image_filename);

            update_node_cover_img
                .update_cb(&self.data.app.db_session, self.data)
                .await?;

            return Ok(url);
        }

        Err(NodecosmosError::InternalServerError(
            "Failed to read multipart field".to_string(),
        ))
    }
}
