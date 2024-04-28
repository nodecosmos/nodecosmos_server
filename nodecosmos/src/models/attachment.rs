use actix_multipart::Multipart;
use charybdis::batch::ModelBatch;
use charybdis::macros::charybdis_model;
use charybdis::operations::{InsertWithCallbacks, New};
use charybdis::types::{Text, Timestamp, Uuid};
use futures::StreamExt;
use scylla::CachingSession;
use serde::{Deserialize, Serialize};

use crate::api::data::RequestData;
use crate::api::ImageAttachmentParams;
use crate::errors::NodecosmosError;
use crate::models::traits::s3::S3;
use crate::models::utils::{impl_default_callbacks, Image};

const MAX_IMAGE_WIDTH: u32 = 852;

#[charybdis_model(
    table_name = attachments,
    partition_keys = [branch_id],
    clustering_keys = [node_id, object_id, id],
    static_columns = [],
)]
#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    pub node_id: Uuid,
    pub branch_id: Uuid,
    pub object_id: Uuid,

    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,

    pub key: Text,
    pub url: Option<Text>,
    pub user_id: Option<Uuid>,

    #[serde(default = "chrono::Utc::now")]
    pub created_at: Timestamp,

    #[serde(default = "chrono::Utc::now")]
    pub updated_at: Timestamp,
}

impl_default_callbacks!(Attachment);

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
            attachment.branch_id = params.branch_id;
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

    pub async fn find_by_node_ids(
        db_session: &CachingSession,
        ids: &[Uuid],
    ) -> Result<Vec<Attachment>, NodecosmosError> {
        find_attachment!("node_id IN ? AND branch_id IN ?", (ids, ids))
            .execute(db_session)
            .await?
            .try_collect()
            .await
            .map_err(NodecosmosError::from)
    }

    pub async fn find_by_branch_id_and_node_ids(
        db_session: &CachingSession,
        ids: &[Uuid],
        branch_id: Uuid,
    ) -> Result<Vec<Attachment>, NodecosmosError> {
        find_attachment!("node_id = ? AND branch_id = ?", (ids, branch_id))
            .execute(db_session)
            .await?
            .try_collect()
            .await
            .map_err(NodecosmosError::from)
    }
}

pub trait AttachmentsDelete {
    fn delete_all(self, data: &RequestData) -> Result<(), NodecosmosError>;
}

impl AttachmentsDelete for Vec<Attachment> {
    fn delete_all(self, data: &RequestData) -> Result<(), NodecosmosError> {
        let data = data.clone();

        tokio::spawn(async move {
            Attachment::delete_s3_objects(&data, &self).await;

            let _ = Attachment::batch()
                .chunked_delete_by_partition_key(data.db_session(), &self, crate::constants::BATCH_CHUNK_SIZE)
                .await
                .map_err(|e| log::error!("Error deleting attachments: {}", e));
        });

        Ok(())
    }
}
