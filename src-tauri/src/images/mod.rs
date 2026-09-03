use std::io::Cursor;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use friendolls_common::{InteractionContent, MAX_IMAGE_B64_SIZE, MAX_IMAGE_DIMENSION};
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, ImageReader, Limits, Rgb, RgbImage};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use crate::network::Network;

const MAX_SOURCE_BYTES: usize = 25 * 1024 * 1024;
const MAX_SOURCE_DIMENSION: u32 = 16_384;
const MAX_DECODE_ALLOC: u64 = 128 * 1024 * 1024;
const SCALES: [f32; 3] = [1.0, 0.8, 0.65];
const JPEG_QUALITIES: [u8; 4] = [50, 38, 26, 18];

#[derive(Debug, Clone)]
struct EncodedImage {
    media_type: String,
    data: String,
}

#[tauri::command]
#[specta::specta]
pub async fn pick_and_send_image(
    recipient_id: String,
    app: AppHandle,
    network: State<'_, Network>,
) -> Result<bool, String> {
    let Some(file) = app
        .dialog()
        .file()
        .add_filter(
            crate::settings::text(&app, crate::settings::NativeText::Images),
            &["png", "jpg", "jpeg", "gif", "webp"],
        )
        .blocking_pick_file()
    else {
        return Ok(false);
    };
    let path = file
        .as_path()
        .ok_or_else(|| "The selected image is not a local file".to_owned())?
        .to_owned();
    let encoded = tauri::async_runtime::spawn_blocking(move || compress_image_file(&path))
        .await
        .map_err(|error| format!("Image compression task failed: {error}"))??;

    send_encoded_image(&network, recipient_id, encoded).await?;
    Ok(true)
}

#[tauri::command]
#[specta::specta]
pub async fn send_image_bytes(
    recipient_id: String,
    bytes: Vec<u8>,
    network: State<'_, Network>,
) -> Result<(), String> {
    let encoded = tauri::async_runtime::spawn_blocking(move || compress_image_bytes(&bytes))
        .await
        .map_err(|error| format!("Image compression task failed: {error}"))??;
    send_encoded_image(&network, recipient_id, encoded).await
}

async fn send_encoded_image(
    network: &Network,
    recipient_id: String,
    encoded: EncodedImage,
) -> Result<(), String> {
    network
        .send_interaction(
            recipient_id,
            InteractionContent::Image {
                media_type: encoded.media_type,
                data: encoded.data,
            },
        )
        .await
}

fn compress_image_file(path: &std::path::Path) -> Result<EncodedImage, String> {
    let metadata =
        std::fs::metadata(path).map_err(|_| "The selected image could not be read".to_owned())?;
    if metadata.len() > MAX_SOURCE_BYTES as u64 {
        return Err(format!(
            "The selected image must be at most {} MiB",
            MAX_SOURCE_BYTES / 1024 / 1024
        ));
    }
    let bytes =
        std::fs::read(path).map_err(|_| "The selected image could not be read".to_owned())?;
    compress_image_bytes(&bytes)
}

fn compress_image_bytes(bytes: &[u8]) -> Result<EncodedImage, String> {
    if bytes.is_empty() {
        return Err("Choose an image file".to_owned());
    }
    if bytes.len() > MAX_SOURCE_BYTES {
        return Err(format!(
            "The selected image must be at most {} MiB",
            MAX_SOURCE_BYTES / 1024 / 1024
        ));
    }

    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| "The selected image could not be decoded".to_owned())?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_SOURCE_DIMENSION);
    limits.max_image_height = Some(MAX_SOURCE_DIMENSION);
    limits.max_alloc = Some(MAX_DECODE_ALLOC);
    reader.limits(limits);
    let source = reader
        .decode()
        .map_err(|_| "The selected image could not be decoded".to_owned())?;
    let source = composite_onto_white(source);
    let (width, height) = scaled_dimensions(source.width(), source.height());

    for scale in SCALES {
        let width = ((width as f32 * scale).round() as u32).max(1);
        let height = ((height as f32 * scale).round() as u32).max(1);
        let resized = image::imageops::resize(&source, width, height, FilterType::Triangle);

        for quality in JPEG_QUALITIES {
            let encoded =
                webp::Encoder::from_rgb(resized.as_raw(), width, height).encode(quality as f32);
            let data = STANDARD.encode(&*encoded);
            if data.len() <= MAX_IMAGE_B64_SIZE {
                return Ok(EncodedImage {
                    media_type: "image/webp".to_owned(),
                    data,
                });
            }
        }

        for quality in JPEG_QUALITIES {
            let mut encoded = Vec::new();
            JpegEncoder::new_with_quality(&mut encoded, quality)
                .encode_image(&resized)
                .map_err(|_| "Image compression failed".to_owned())?;
            let data = STANDARD.encode(encoded);
            if data.len() <= MAX_IMAGE_B64_SIZE {
                return Ok(EncodedImage {
                    media_type: "image/jpeg".to_owned(),
                    data,
                });
            }
        }
    }

    Err("Image is still too detailed after compression".to_owned())
}

fn scaled_dimensions(width: u32, height: u32) -> (u32, u32) {
    let largest = width.max(height);
    if largest <= MAX_IMAGE_DIMENSION {
        return (width, height);
    }

    let scale = MAX_IMAGE_DIMENSION as f64 / largest as f64;
    (
        ((width as f64 * scale).round() as u32).max(1),
        ((height as f64 * scale).round() as u32).max(1),
    )
}

fn composite_onto_white(image: DynamicImage) -> RgbImage {
    let rgba = image.into_rgba8();
    RgbImage::from_fn(rgba.width(), rgba.height(), |x, y| {
        let pixel = rgba.get_pixel(x, y).0;
        let alpha = pixel[3] as u16;
        let blend = |channel: u8| ((channel as u16 * alpha + 255 * (255 - alpha)) / 255) as u8;
        Rgb([blend(pixel[0]), blend(pixel[1]), blend(pixel[2])])
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageFormat, Rgba, RgbaImage};

    fn png_bytes(width: u32, height: u32) -> Vec<u8> {
        let image = RgbaImage::from_pixel(width, height, Rgba([20, 80, 160, 128]));
        let mut bytes = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image)
            .write_to(&mut bytes, ImageFormat::Png)
            .unwrap();
        bytes.into_inner()
    }

    #[test]
    fn compresses_and_limits_image_payload() {
        let compressed = compress_image_bytes(&png_bytes(900, 600)).unwrap();

        assert_eq!(compressed.media_type, "image/webp");
        assert!(!compressed.data.is_empty());
        assert!(compressed.data.len() <= MAX_IMAGE_B64_SIZE);
        assert!(STANDARD.decode(compressed.data).is_ok());
    }

    #[test]
    fn rejects_empty_and_invalid_input() {
        assert_eq!(
            compress_image_bytes(&[]).unwrap_err(),
            "Choose an image file"
        );
        assert_eq!(
            compress_image_bytes(b"not an image").unwrap_err(),
            "The selected image could not be decoded"
        );
    }

    #[test]
    fn preserves_aspect_ratio_within_dimension_limit() {
        assert_eq!(scaled_dimensions(960, 480), (480, 240));
        assert_eq!(scaled_dimensions(120, 80), (120, 80));
    }
}
