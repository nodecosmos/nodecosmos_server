use crate::api::ImageAttachmentParams;
use crate::errors::NodecosmosError;
use crate::models::user::CurrentUser;
use crate::models::utils::impl_default_callbacks;
use crate::services::aws::s3::upload_s3_object;
use crate::services::image::Image;
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
    global_secondary_indexes = [],
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

    #[serde(rename = "createdAt")]
    pub created_at: Option<Timestamp>,

    #[serde(rename = "updatedAt")]
    pub updated_at: Option<Timestamp>,
}

impl Attachment {
    pub fn build_s3_filename(object_id: String) -> String {
        let timestamp = chrono::Utc::now().timestamp();

        format!("{}-{}.jpeg", object_id, timestamp)
    }

    pub fn build_s3_url(bucket: String, key: String) -> String {
        format!("https://{}.s3.amazonaws.com/{}", bucket, key)
    }

    pub async fn create_image(
        params: &ImageAttachmentParams,
        app: &crate::App,
        user: &CurrentUser,
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

            let key = Attachment::build_s3_filename(params.object_id.to_string());
            let url = Attachment::build_s3_url(app.s3_bucket.clone(), key.clone());

            upload_s3_object(&app.s3_client, compressed, &app.s3_bucket, &key).await?;

            let mut attachment = Attachment::new();
            attachment.node_id = params.node_id;
            attachment.object_id = params.object_id;
            attachment.key = key;
            attachment.url = Some(url);
            attachment.user_id = Some(user.id);

            attachment.insert_cb(&app.db_session).await?;

            return Ok(attachment);
        }

        Err(NodecosmosError::InternalServerError(
            "No image found in multipart request".to_string(),
        ))
    }
}

impl_default_callbacks!(Attachment);
