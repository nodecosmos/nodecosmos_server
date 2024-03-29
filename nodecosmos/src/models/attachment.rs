use crate::api::data::RequestData;
use crate::api::ImageAttachmentParams;
use crate::errors::NodecosmosError;
use crate::models::traits::s3::S3;
use crate::models::utils::{impl_default_callbacks, Image};
use actix_multipart::Multipart;
use charybdis::macros::charybdis_model;
use charybdis::operations::{InsertWithCallbacks, New};
use charybdis::types::{Text, Timestamp, Uuid};
use futures::StreamExt;
use serde::{Deserialize, Serialize};

const MAX_IMAGE_WIDTH: u32 = 852;

#[charybdis_model(
    table_name = attachments,
    partition_keys = [node_id],
    clustering_keys = [object_id, id],
)]
#[derive(Serialize, Deserialize, Default)]
pub struct Attachment {
    #[serde(rename = "nodeId")]
    pub node_id: Uuid,

    #[serde(rename = "objectId")]
    pub object_id: Uuid,

    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,

    pub key: Text,
    pub url: Option<Text>,

    #[serde(rename = "userId")]
    pub user_id: Option<Uuid>,

    #[serde(rename = "createdAt", default = "chrono::Utc::now")]
    pub created_at: Timestamp,

    #[serde(rename = "updatedAt", default = "chrono::Utc::now")]
    pub updated_at: Timestamp,
}

impl Attachment {
    pub async fn create_image(
        params: &ImageAttachmentParams,
        data: &RequestData,
        mut payload: Multipart,
    ) -> Result<Attachment, NodecosmosError> {
        if let Some(item) = payload.next().await {
            let mut field = item.map_err(|e| {
                NodecosmosError::InternalServerError(format!("Failed to read multipart field: {:?}", e))
            })?;

            let mut image = Image::from_field(&mut field).await?;
            let mut width = image.width;
            let mut height = image.height;

            if image.width > MAX_IMAGE_WIDTH {
                width = MAX_IMAGE_WIDTH;
                height = (image.width as f32 * (image.height as f32 / image.width as f32)) as u32;
            }

            image.resize_image(width, height);
            let compressed = image.compressed()?;

            let mut attachment = Attachment::new();
            attachment.node_id = params.node_id;
            attachment.object_id = params.object_id;
            attachment.user_id = Some(data.current_user.id);

            // assign s3 key before url generation & upload
            attachment.key = attachment.build_s3_key("attachment", "jpg");
            attachment.url = Some(attachment.s3_url(data));

            attachment.upload_s3_object(data, compressed).await?;

            attachment.insert_cb(&None).execute(data.db_session()).await?;

            return Ok(attachment);
        }

        Err(NodecosmosError::InternalServerError(
            "No image found in multipart request".to_string(),
        ))
    }
}

impl_default_callbacks!(Attachment);
