use std::collections::HashSet;
use std::time::Duration;

use aws_sdk_s3::types::{Delete, ObjectIdentifier};
use aws_sdk_s3::{presigning::PresigningConfig, primitives::ByteStream};
use charybdis::types::Uuid;

use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::attachment::Attachment;
use crate::models::node::UpdateCoverImageNode;
use crate::models::user::UpdateProfileImageUser;

pub trait S3: Sized {
    fn s3_key(&self) -> &String;

    fn s3_object_id(&self) -> String;

    fn s3_url(&self, data: &RequestData) -> String {
        format!("https://{}.s3.amazonaws.com/{}", data.s3_bucket(), self.s3_key())
    }

    fn build_s3_key(&self, purpose: &str, ext: &str) -> String {
        let timestamp = chrono::Utc::now().timestamp();

        format!("{}/{}-{}.{}", self.s3_object_id(), timestamp, purpose, ext)
    }

    async fn upload_s3_object(&self, data: &RequestData, bytes: Vec<u8>) -> Result<(), NodecosmosError> {
        let put_object = data
            .s3_client()
            .put_object()
            .key(self.s3_key())
            .bucket(data.s3_bucket());

        let body = ByteStream::from(bytes);

        put_object
            .body(body)
            .send()
            .await
            .map_err(|e| NodecosmosError::InternalServerError(format!("Failed to upload to S3: {:?}", e)))?;

        Ok(())
    }

    async fn delete_s3_object(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        let s3_key = self.s3_key().clone();
        let s3_bucket = data.s3_bucket().clone();
        let s3_client = data.s3_client_arc();

        tokio::spawn(async move {
            let delete_s3_object = s3_client.delete_object().key(s3_key).bucket(s3_bucket);

            let _ = delete_s3_object
                .send()
                .await
                .map_err(|e| log::error!("Failed to delete from S3: {:?}", e));
        });

        Ok(())
    }

    async fn get_presigned_url(
        data: &RequestData,
        object_id: &Uuid,
        filename: &str,
    ) -> Result<String, NodecosmosError> {
        // scope keys to object_id
        let key = format!("{}/{}-{}", object_id, chrono::Utc::now().timestamp(), filename);
        let put_object = data.s3_client().put_object().key(key).bucket(data.s3_bucket());
        let presigned_config = PresigningConfig::expires_in(Duration::from_secs(300))
            .map_err(|e| NodecosmosError::InternalServerError(format!("Failed to set presigned config: {:?}", e)))?;
        let presigned_req = put_object
            .presigned(presigned_config)
            .await
            .map_err(|e| NodecosmosError::InternalServerError(format!("Failed to send presigned req: {:?}", e)))?;

        Ok(presigned_req.uri().to_string())
    }

    async fn delete_s3_objects(data: &RequestData, s3_objects: &[Self]) {
        let s3_object_ids = s3_objects
            .iter()
            .map(|object| object.s3_object_id())
            .collect::<HashSet<String>>();
        let mut keys: Vec<String> = vec![];

        for s3_object_id in s3_object_ids {
            let resp = data
                .s3_client()
                .list_objects_v2()
                .bucket(data.s3_bucket())
                .prefix(s3_object_id) // The directory name acts as a prefix
                .send()
                .await;

            match resp {
                Ok(resp) => {
                    let current_keys = resp
                        .contents
                        .unwrap_or_default()
                        .into_iter()
                        .filter_map(|object| match object.key {
                            Some(key) => Some(key),
                            None => {
                                log::error!("Failed to get key from S3 object");
                                None
                            }
                        })
                        .collect::<Vec<String>>();

                    keys.extend(current_keys);
                }
                Err(e) => {
                    log::error!("Failed to list objects in S3: {:?}", e);
                    return;
                }
            }
        }

        if keys.is_empty() {
            return;
        }

        let objects = keys
            .into_iter()
            .filter_map(|key| match ObjectIdentifier::builder().key(key).build() {
                Ok(object) => Some(object),
                Err(e) => {
                    log::error!("Failed to build object identifier: {:?}", e);
                    None
                }
            })
            .collect::<Vec<_>>();

        let delete = Delete::builder().set_objects(Some(objects)).build();

        match delete {
            Ok(delete) => {
                let _ = data
                    .s3_client()
                    .delete_objects()
                    .bucket(data.s3_bucket())
                    .delete(delete)
                    .send()
                    .await
                    .map_err(|e| {
                        log::error!("Failed to delete from S3: {:?}", e);
                    });
            }
            Err(e) => {
                log::error!("Failed to delete from S3: {:?}", e);
            }
        }
    }
}

impl S3 for Attachment {
    fn s3_key(&self) -> &String {
        &self.key
    }

    fn s3_object_id(&self) -> String {
        self.object_id.to_string()
    }
}

impl S3 for UpdateCoverImageNode {
    fn s3_key(&self) -> &String {
        self.cover_image_filename
            .as_ref()
            .expect("cover_image_filename should be set")
    }

    fn s3_object_id(&self) -> String {
        self.id.to_string()
    }
}

impl S3 for UpdateProfileImageUser {
    fn s3_key(&self) -> &String {
        self.profile_image_filename
            .as_ref()
            .expect("profile_image_filename should be set")
    }

    fn s3_object_id(&self) -> String {
        self.id.to_string()
    }
}
