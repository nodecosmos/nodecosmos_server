use crate::errors::NodecosmosError;
use aws_sdk_s3::primitives::ByteStream;
use std::time::Duration;

pub async fn upload_s3_object(
    s3_client: &aws_sdk_s3::Client,
    bytes: Vec<u8>,
    bucket: &str,
    key: &str,
) -> Result<(), NodecosmosError> {
    let put_object = s3_client.put_object().key(key).bucket(bucket);
    let body = ByteStream::from(bytes);

    put_object
        .body(body)
        .send()
        .await
        .map_err(|e| NodecosmosError::InternalServerError(format!("Failed to upload to S3: {:?}", e)))?;

    Ok(())
}

pub async fn delete_s3_object(s3_client: &aws_sdk_s3::Client, bucket: &str, key: &str) -> Result<(), NodecosmosError> {
    let delete_object = s3_client.delete_object().key(key).bucket(bucket);

    delete_object
        .send()
        .await
        .map_err(|e| NodecosmosError::InternalServerError(format!("Failed to delete from S3: {:?}", e)))?;

    Ok(())
}

pub async fn get_s3_presigned_url(app: &crate::App, key: &str) -> Result<String, NodecosmosError> {
    let put_object = app.s3_client.put_object().key(key).bucket(app.s3_bucket.clone());
    let presigned_config = aws_sdk_s3::presigning::PresigningConfig::expires_in(Duration::from_secs(300))
        .map_err(|e| NodecosmosError::InternalServerError(format!("Failed to set presigned config: {:?}", e)))?;

    let presigned_req = put_object
        .presigned(presigned_config)
        .await
        .map_err(|e| NodecosmosError::InternalServerError(format!("Failed to send presigned req: {:?}", e)))?;

    Ok(presigned_req.uri().to_string())
}
