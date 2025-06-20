use actix_multipart::Multipart;
use charybdis::operations::{InsertWithCallbacks, UpdateWithCallbacks};
use futures::StreamExt;
use uuid::Uuid;

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::attachment::Attachment;
use crate::models::node::UpdateCoverImageNode;
use crate::models::traits::s3::S3;
use crate::models::utils::Image;

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
            let extension = image.extension;
            let compressed = image.compressed(None)?;

            if self.cover_image_filename.is_some() {
                self.delete_s3_object(data).await?;
            }

            // assign s3 key before url generation & upload
            self.cover_image_filename = Some(self.build_s3_key("cover", extension));
            self.cover_image_url = Some(self.s3_url(data));

            self.upload_s3_object(data, compressed).await?;

            self.update_cb(data).execute(data.db_session()).await?;

            // create attachment
            Attachment {
                node_id: self.id,
                branch_id: self.branch_id,
                root_id: self.root_id,
                object_id: self.id,
                id: Uuid::new_v4(),
                key: self.cover_image_filename.clone().unwrap(),
                url: self.cover_image_url.clone(),
                user_id: Some(data.current_user.id),
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            }
            .insert_cb(&None)
            .execute(data.db_session())
            .await?;

            return Ok(());
        }

        Err(NodecosmosError::InternalServerError(
            "Failed to read multipart field".to_string(),
        ))
    }

    pub async fn delete_cover_image(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.cover_image_url.is_some() {
            self.delete_s3_object(data).await?;
        }

        self.cover_image_filename = None;
        self.cover_image_url = None;

        self.update_cb(data).execute(data.db_session()).await?;

        Ok(())
    }
}
