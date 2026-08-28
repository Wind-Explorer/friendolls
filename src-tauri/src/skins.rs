use std::io::Cursor;
use std::path::PathBuf;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager, State};

use crate::network::Network;

const MAX_SKIN_BYTES: usize = 64 * 1024;
const SKIN_WIDTH: u32 = 64;
const SKIN_HEIGHT: u32 = 64;

fn validate_hash(hash: &str) -> Result<(), String> {
    if wyd_common::is_skin_hash(hash) {
        Ok(())
    } else {
        Err("Skin hash must be a 64-character SHA-256 value".to_owned())
    }
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn validate(bytes: &[u8]) -> Result<String, String> {
    if bytes.is_empty() || bytes.len() > MAX_SKIN_BYTES {
        return Err(format!(
            "Skin PNG must be at most {} KiB",
            MAX_SKIN_BYTES / 1024
        ));
    }
    let dimensions = image::ImageReader::with_format(Cursor::new(bytes), image::ImageFormat::Png)
        .into_dimensions()
        .map_err(|_| "Skin must be a valid PNG image".to_owned())?;
    if dimensions != (SKIN_WIDTH, SKIN_HEIGHT) {
        return Err(format!("Skin must be {SKIN_WIDTH}×{SKIN_HEIGHT} pixels"));
    }
    image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
        .map_err(|_| "Skin must be a valid PNG image".to_owned())?;
    Ok(hash(bytes))
}

fn local_path(handle: &AppHandle, hash: &str) -> Result<PathBuf, String> {
    Ok(handle
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?
        .join("skins")
        .join(format!("{hash}.png")))
}

fn cache_path(handle: &AppHandle, hash: &str) -> Result<PathBuf, String> {
    Ok(handle
        .path()
        .app_cache_dir()
        .map_err(|error| error.to_string())?
        .join("skins")
        .join(format!("{hash}.png")))
}

fn read_verified(path: PathBuf, expected_hash: &str) -> Result<Option<Vec<u8>>, String> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.to_string()),
    };
    Ok(validate(&bytes)
        .is_ok_and(|actual_hash| actual_hash == expected_hash)
        .then_some(bytes))
}

pub(crate) fn decode_response(data: &str, expected_hash: &str) -> Option<Vec<u8>> {
    if data.len() > wyd_common::MAX_SKIN_B64_SIZE {
        return None;
    }
    let bytes = STANDARD.decode(data).ok()?;
    (validate(&bytes).ok()?.as_str() == expected_hash).then_some(bytes)
}

fn write(path: PathBuf, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "Skin path has no parent directory".to_owned())?;
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    std::fs::write(path, bytes).map_err(|error| error.to_string())
}

pub(crate) fn read_local_base64(handle: &AppHandle, skin_hash: &str) -> Option<String> {
    validate_hash(skin_hash).ok()?;
    read_verified(local_path(handle, skin_hash).ok()?, skin_hash)
        .ok()
        .flatten()
        .map(|bytes| STANDARD.encode(bytes))
}

pub(crate) fn store_local(handle: &AppHandle, data: &[u8]) -> Result<String, String> {
    let skin_hash = validate(data)?;
    write(local_path(handle, &skin_hash)?, data)?;
    Ok(skin_hash)
}

#[tauri::command]
#[specta::specta]
pub async fn resolve_skin(
    handle: AppHandle,
    network: State<'_, Network>,
    user_id: String,
    skin_hash: String,
) -> Result<Option<String>, String> {
    validate_hash(&skin_hash)?;
    crate::user::validate_id(&user_id)?;

    let bytes = if user_id == network.public_key() {
        read_verified(local_path(&handle, &skin_hash)?, &skin_hash)?
    } else if let Some(bytes) = read_verified(cache_path(&handle, &skin_hash)?, &skin_hash)? {
        Some(bytes)
    } else {
        let Some(bytes) = network.request_skin(user_id, skin_hash.clone()).await? else {
            return Ok(None);
        };
        write(cache_path(&handle, &skin_hash)?, &bytes)?;
        Some(bytes)
    };

    Ok(bytes.map(|bytes| STANDARD.encode(bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_png_and_invalid_hashes() {
        assert!(validate(b"not a png").is_err());
        assert!(validate_hash("../skin").is_err());
        assert!(validate_hash(&"a".repeat(64)).is_ok());
    }

    #[test]
    fn rejects_png_dimensions_before_full_validation() {
        let mut bytes = Cursor::new(Vec::new());
        image::DynamicImage::new_rgba8(SKIN_WIDTH + 1, SKIN_HEIGHT)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();

        assert_eq!(
            validate(&bytes.into_inner()).unwrap_err(),
            "Skin must be 64×64 pixels"
        );
    }

    #[test]
    fn accepts_a_64_by_64_png_and_returns_its_sha256_hash() {
        let mut bytes = Cursor::new(Vec::new());
        image::DynamicImage::new_rgba8(SKIN_WIDTH, SKIN_HEIGHT)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        let bytes = bytes.into_inner();

        assert_eq!(validate(&bytes).unwrap(), hash(&bytes));
    }
}
