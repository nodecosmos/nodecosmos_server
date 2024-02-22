use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::attachment::image::Image;
use crate::models::node::UpdateCoverImageNode;
use crate::services::aws::s3::{delete_s3_object, upload_s3_object};
use actix_multipart::Multipart;
use charybdis::operations::UpdateWithCallbacks;
use futures::StreamExt;
use log::error;

const IMG_WIDTH: u32 = 850;
const IMG_HEIGHT: u32 = 375;

impl UpdateCoverImageNode {
    pub async fn update_cover_image(
        &mut self,
        data: &RequestData,
        mut payload: Multipart,
    ) -> Result<(), NodecosmosError> {
        if let Some(item) = payload.next().await {
            let mut field = item.map_err(|e| {
                NodecosmosError::InternalServerError(format!("Failed to read multipart field: {:?}", e))
            })?;

            let mut image = Image::from_field(&mut field).await?;
            image.resize_image(IMG_WIDTH, IMG_HEIGHT);
            let compressed = image.compressed()?;

            let timestamp = chrono::Utc::now().timestamp();

            if let Some(cover_image_filename) = &self.cover_image_filename {
                // delete s3 object asynchronously
                let bucket = data.app.s3_bucket.clone();
                let key = cover_image_filename.clone();
                let s3_client = data.app.s3_client.clone();
                tokio::spawn(async move {
                    let _ = delete_s3_object(&s3_client, &bucket, &key).await.map_err(|e| {
                        println!("Failed to delete existing cover image from S3: {:?}", e);
                    });
                });
            }

            let new_cover_image_filename = format!("{}/{}-cover.jpeg", self.id, timestamp);
            let url = format!(
                "https://{}.s3.amazonaws.com/{}",
                data.app.s3_bucket, new_cover_image_filename
            );

            upload_s3_object(
                &data.s3_client(),
                &data.s3_bucket(),
                compressed,
                &new_cover_image_filename,
            )
            .await?;

            self.cover_image_url = Some(url.clone());
            self.cover_image_filename = Some(new_cover_image_filename);

            self.update_cb(data).execute(data.db_session()).await?;

            return Ok(());
        }

        Err(NodecosmosError::InternalServerError(
            "Failed to read multipart field".to_string(),
        ))
    }

    pub async fn delete_cover_image(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.cover_image_url.is_some() {
            let key = self
                .cover_image_filename
                .clone()
                .ok_or_else(|| NodecosmosError::InternalServerError("Missing cover image key".to_string()))?;

            let bucket = data.app.s3_bucket.clone();
            let key = key.clone();
            let s3_client = data.app.s3_client.clone();

            tokio::spawn(async move {
                let _ = delete_s3_object(&s3_client, &bucket, &key)
                    .await
                    .map_err(|e| error!("Failed to delete cover image from S3: {:?}", e));
            });
        }

        self.cover_image_url = None;
        self.cover_image_filename = None;

        self.update_cb(data).execute(data.db_session()).await?;

        Ok(())
    }
}
