use futures::StreamExt;

use crate::errors::NodecosmosError;

const TARGET_SIZE_IN_KB: usize = 50;
const DEFAULT_TARGET_SIZE_IN_BYTES: usize = TARGET_SIZE_IN_KB * 1024;

pub struct Image {
    inner: image::DynamicImage,
    buffer: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub file_size: usize,
    pub extension: &'static str,
}

impl Image {
    async fn read_image_buffer(field: &mut actix_multipart::Field) -> Result<Vec<u8>, NodecosmosError> {
        let mut buffer: Vec<u8> = Vec::new();
        while let Some(chunk) = field.next().await {
            let data = chunk.map_err(|e| {
                NodecosmosError::InternalServerError(format!("Failed to read multipart field: {:?}", e))
            })?;
            buffer.extend_from_slice(&data);
        }
        Ok(buffer)
    }

    fn read_image_format(buffer: &[u8]) -> Result<image::ImageFormat, NodecosmosError> {
        let image_format = image::guess_format(buffer)
            .map_err(|e| NodecosmosError::InternalServerError(format!("Failed to guess image format: {:?}", e)))?;

        if image_format != image::ImageFormat::Png
            && image_format != image::ImageFormat::Jpeg
            && image_format != image::ImageFormat::WebP
        {
            return Err(NodecosmosError::UnsupportedMediaType);
        }

        Ok(image_format)
    }

    fn decode_image(format: image::ImageFormat, buffer: &[u8]) -> Result<image::DynamicImage, NodecosmosError> {
        if format != image::ImageFormat::Png && format != image::ImageFormat::Jpeg && format != image::ImageFormat::WebP
        {
            return Err(NodecosmosError::UnsupportedMediaType);
        }

        let img = image::load_from_memory(buffer)
            .map_err(|e| NodecosmosError::InternalServerError(format!("Failed to decode image: {:?}", e)))?;

        Ok(img)
    }

    pub async fn from_field(field: &mut actix_multipart::Field) -> Result<Self, NodecosmosError> {
        let buffer = Self::read_image_buffer(field).await?;
        let file_size = buffer.len();
        let format = Self::read_image_format(&buffer)?;
        let image = Self::decode_image(format, &buffer)?;
        let width = image.width();
        let height = image.height();
        let extension = match format {
            image::ImageFormat::Png => "png",
            image::ImageFormat::Jpeg => "jpg",
            image::ImageFormat::WebP => "webp",
            _ => return Err(NodecosmosError::UnsupportedMediaType),
        };

        Ok(Self {
            inner: image,
            buffer,
            width,
            height,
            file_size,
            extension,
        })
    }

    pub fn resize_image(&mut self, width: u32, height: u32) -> &Self {
        if self.inner.width() > width || self.inner.height() > height {
            self.inner = self.inner.resize(width, height, image::imageops::FilterType::Lanczos3);
        }

        self
    }

    pub fn compressed(self, target_size_in_bytes: Option<usize>) -> Result<Vec<u8>, NodecosmosError> {
        let img = self.inner;

        println!("file size: {}", self.file_size);

        let target_size_in_bytes = target_size_in_bytes.unwrap_or(DEFAULT_TARGET_SIZE_IN_BYTES);

        if self.file_size <= target_size_in_bytes {
            return Ok(self.buffer);
        }

        let image_src = match img {
            image::DynamicImage::ImageRgb8(rgb_img) => rgb_img,
            _ => img.to_rgb8(),
        };

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
