use crate::models::helpers::impl_default_callbacks;
use charybdis::{Text, Timestamp, Uuid};
use charybdis_macros::{charybdis_model, partial_model_generator};
use serde::{Deserialize, Serialize};

#[partial_model_generator]
#[charybdis_model(
    table_name = attachments,
    partition_keys = [node_id],
    clustering_keys = [object_id, id],
    secondary_indexes = [],
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
}

impl_default_callbacks!(Attachment);
