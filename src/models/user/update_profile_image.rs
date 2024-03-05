use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::attachment::image::Image;
use crate::models::traits::s3::S3;
use crate::models::user::UpdateProfileImageUser;
use actix_multipart::Multipart;
use charybdis::operations::UpdateWithCallbacks;
use futures::StreamExt;
use log::error;

const IMG_WIDTH: u32 = 296;
const IMG_HEIGHT: u32 = 296;

impl UpdateProfileImageUser {
    pub async fn update_profile_image(
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

            if self.profile_image_filename.is_some() {
                self.delete_s3_object(data).await?;
            }

            // assign s3 key before url generation & upload
            self.profile_image_filename = Some(self.build_s3_key());
            self.profile_image_url = Some(self.s3_url(data));

            self.upload_s3_object(data, compressed).await?;

            self.update_cb(data).execute(data.db_session()).await?;

            return Ok(());
        }

        Err(NodecosmosError::InternalServerError(
            "Failed to read multipart field".to_string(),
        ))
    }

    pub async fn delete_profile_image(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.profile_image_url.is_some() {
            self.delete_s3_object(data).await?;
        }

        self.profile_image_url = None;
        self.profile_image_filename = None;

        self.update_cb(data).execute(data.db_session()).await?;

        Ok(())
    }
}
