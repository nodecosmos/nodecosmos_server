use crate::errors::NodecosmosError;
use futures::StreamExt;
use image::RgbImage;

pub async fn read_image_buffer(
    field: &mut actix_multipart::Field,
) -> Result<Vec<u8>, NodecosmosError> {
    let mut buffer: Vec<u8> = Vec::new();
    while let Some(chunk) = field.next().await {
        let data = chunk.map_err(|e| {
            NodecosmosError::InternalServerError(format!("Failed to read multipart field: {:?}", e))
        })?;
        buffer.extend_from_slice(&data);
    }
    Ok(buffer)
}

pub fn read_image_format(buffer: &[u8]) -> Result<image::ImageFormat, NodecosmosError> {
    let image_format = image::guess_format(buffer).map_err(|e| {
        NodecosmosError::InternalServerError(format!("Failed to guess image format: {:?}", e))
    })?;

    if image_format != image::ImageFormat::Png && image_format != image::ImageFormat::Jpeg {
        return Err(NodecosmosError::UnsupportedMediaType);
    }

    Ok(image_format)
}

pub fn decode_image(buffer: &[u8]) -> Result<image::DynamicImage, NodecosmosError> {
    let image_format = read_image_format(buffer)?;

    if image_format != image::ImageFormat::Png && image_format != image::ImageFormat::Jpeg {
        return Err(NodecosmosError::UnsupportedMediaType);
    }

    let img = image::load_from_memory(buffer).map_err(|e| {
        NodecosmosError::InternalServerError(format!("Failed to decode image: {:?}", e))
    })?;

    Ok(img)
}

pub fn resize_image(
    mut img: image::DynamicImage,
    width: u32,
    height: u32,
) -> Result<image::DynamicImage, NodecosmosError> {
    println!("img.width(): {}", img.width());
    println!("img.height(): {}", img.height());

    if img.width() > width || img.height() > height {
        println!("Resizing image");
        img = img.resize(width, height, image::imageops::FilterType::Lanczos3);
    }

    Ok(img)
}

pub fn convert_image_to_rgb(img: image::DynamicImage) -> Result<RgbImage, NodecosmosError> {
    let rgb_img = match img {
        image::DynamicImage::ImageRgb8(rgb_img) => rgb_img,
        _ => img.to_rgb8(),
    };

    Ok(rgb_img)
}

pub fn compress_image(image_src: RgbImage) -> Result<Vec<u8>, NodecosmosError> {
    let compressed: Vec<u8> = Vec::new();

    let mut compress = mozjpeg::Compress::new(mozjpeg::ColorSpace::JCS_RGB);
    compress.set_size(image_src.width() as usize, image_src.height() as usize);
    compress.set_quality(90f32);

    let mut compress = compress.start_compress(compressed).map_err(|e| {
        NodecosmosError::InternalServerError(format!("Failed to start compressing image: {:?}", e))
    })?;

    compress.write_scanlines(&image_src).map_err(|e| {
        NodecosmosError::InternalServerError(format!("Failed to compress image: {:?}", e))
    })?;

    let compressed = compress.finish().map_err(|e| {
        NodecosmosError::InternalServerError(format!("Failed to finish compressing image: {:?}", e))
    })?;

    Ok(compressed)
}
