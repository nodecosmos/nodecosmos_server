use crate::errors::NodecosmosError;
use aws_sdk_s3::primitives::ByteStream;

pub async fn upload_s3_object(
    s3_client: &aws_sdk_s3::Client,
    bytes: Vec<u8>,
    bucket: &str,
    key: &str,
) -> Result<(), NodecosmosError> {
    let put_object = s3_client.put_object().key(key).bucket(bucket);
    let body = ByteStream::from(bytes);

    put_object.body(body).send().await.map_err(|e| {
        NodecosmosError::InternalServerError(format!("Failed to upload to S3: {:?}", e))
    })?;

    Ok(())
}

pub async fn delete_s3_object(
    s3_client: &aws_sdk_s3::Client,
    bucket: &str,
    key: &str,
) -> Result<(), NodecosmosError> {
    let delete_object = s3_client.delete_object().key(key).bucket(bucket);

    delete_object.send().await.map_err(|e| {
        NodecosmosError::InternalServerError(format!("Failed to delete from S3: {:?}", e))
    })?;

    Ok(())
}
