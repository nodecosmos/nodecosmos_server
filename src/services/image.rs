use crate::errors::NodecosmosError;
use futures::StreamExt;

const TARGET_SIZE_IN_BYTES: usize = 30 * 1024;

pub struct Image {
    inner: image::DynamicImage,
    pub width: u32,
    pub height: u32,
}

impl Image {
    pub async fn from_field(field: &mut actix_multipart::Field) -> Result<Self, NodecosmosError> {
        let buffer = read_image_buffer(field).await?;
        let format = read_image_format(&buffer)?;
        let image = decode_image(format, buffer)?;
        let width = image.width();
        let height = image.height();

        Ok(Self {
            inner: image,
            width,
            height,
        })
    }

    pub fn resize_image(&mut self, width: u32, height: u32) -> &Self {
        if self.inner.width() > width || self.inner.height() > height {
            self.inner = self.inner.resize(width, height, image::imageops::FilterType::Lanczos3);
        }

        self
    }

    pub fn compressed(&mut self) -> Result<Vec<u8>, NodecosmosError> {
        let img = &self.inner;
        let image_src = match img {
            image::DynamicImage::ImageRgb8(rgb_img) => rgb_img.clone(),
            _ => {
                let img = img.to_rgb8();
                img
            }
        };

        if image_src.len() <= TARGET_SIZE_IN_BYTES {
            return Ok(image_src.to_vec());
        }

        let compressed: Vec<u8> = Vec::new();

        let mut compress = mozjpeg::Compress::new(mozjpeg::ColorSpace::JCS_RGB);
        compress.set_size(image_src.width() as usize, image_src.height() as usize);
        compress.set_quality(90f32);

        let mut compress = compress
            .start_compress(compressed)
            .map_err(|e| NodecosmosError::InternalServerError(format!("Failed to start compressing image: {:?}", e)))?;

        compress
            .write_scanlines(&image_src)
            .map_err(|e| NodecosmosError::InternalServerError(format!("Failed to compress image: {:?}", e)))?;

        let compressed = compress.finish().map_err(|e| {
            NodecosmosError::InternalServerError(format!("Failed to finish compressing image: {:?}", e))
        })?;

        Ok(compressed)
    }
}

async fn read_image_buffer(field: &mut actix_multipart::Field) -> Result<Vec<u8>, NodecosmosError> {
    let mut buffer: Vec<u8> = Vec::new();
    while let Some(chunk) = field.next().await {
        let data = chunk
            .map_err(|e| NodecosmosError::InternalServerError(format!("Failed to read multipart field: {:?}", e)))?;
        buffer.extend_from_slice(&data);
    }
    Ok(buffer)
}

fn read_image_format(buffer: &[u8]) -> Result<image::ImageFormat, NodecosmosError> {
    let image_format = image::guess_format(buffer)
        .map_err(|e| NodecosmosError::InternalServerError(format!("Failed to guess image format: {:?}", e)))?;

    if image_format != image::ImageFormat::Png && image_format != image::ImageFormat::Jpeg {
        return Err(NodecosmosError::UnsupportedMediaType);
    }

    Ok(image_format)
}

fn decode_image(format: image::ImageFormat, buffer: Vec<u8>) -> Result<image::DynamicImage, NodecosmosError> {
    if format != image::ImageFormat::Png && format != image::ImageFormat::Jpeg {
        return Err(NodecosmosError::UnsupportedMediaType);
    }

    let img = image::load_from_memory(&buffer)
        .map_err(|e| NodecosmosError::InternalServerError(format!("Failed to decode image: {:?}", e)))?;

    Ok(img)
}
