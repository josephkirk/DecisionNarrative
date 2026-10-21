//! Project-local file storage for uploaded thumbnails.

use std::path::{Path, PathBuf};

/// Largest accepted thumbnail upload (10 MiB).
pub const MAX_UPLOAD_BYTES: usize = 10 * 1024 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub enum FileError {
    Invalid(String),
    NotFound,
    Io(String),
}

pub fn image_extension(name: &str) -> Option<&'static str> {
    let lower = name.trim().to_ascii_lowercase();
    if lower.ends_with(".png") {
        Some("png")
    } else if lower.ends_with(".jpg") {
        Some("jpg")
    } else if lower.ends_with(".jpeg") {
        Some("jpeg")
    } else {
        None
    }
}

fn is_web_url(reference: &str) -> bool {
    reference.starts_with("http://") || reference.starts_with("https://")
}

/// Accepts an `http(s)` URL or a relative image path inside the project.
pub fn validate_image_ref(reference: &str) -> Result<(), String> {
    let trimmed = reference.trim();
    if trimmed.is_empty() {
        return Err("image reference cannot be empty".into());
    }
    if is_web_url(trimmed) {
        return Ok(());
    }
    if trimmed.starts_with('/') {
        return Err("image path must be relative to the project".into());
    }
    if trimmed.contains('\\') {
        return Err("image path must use forward slashes".into());
    }
    for segment in trimmed.split('/') {
        if segment.is_empty() || segment == "." || segment == ".." {
            return Err("image path cannot contain empty, '.' or '..' segments".into());
        }
    }
    if image_extension(trimmed).is_none() {
        return Err("image path must end with .png, .jpg, or .jpeg".into());
    }
    Ok(())
}

fn magic_matches(extension: &str, bytes: &[u8]) -> bool {
    match extension {
        "png" => bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]),
        _ => bytes.starts_with(&[0xFF, 0xD8, 0xFF]),
    }
}

/// Store an uploaded thumbnail under `project-files/thumbnails/` and return the
/// project-relative path that is persisted on records.
pub fn store_thumbnail(root: &Path, original_name: &str, bytes: &[u8]) -> Result<String, FileError> {
    let extension = image_extension(original_name).ok_or_else(|| {
        FileError::Invalid("thumbnail must be a .png, .jpg, or .jpeg file".into())
    })?;
    if bytes.is_empty() {
        return Err(FileError::Invalid("thumbnail is empty".into()));
    }
    if bytes.len() > MAX_UPLOAD_BYTES {
        return Err(FileError::Invalid(format!(
            "thumbnail exceeds the {MAX_UPLOAD_BYTES} byte limit"
        )));
    }
    if !magic_matches(extension, bytes) {
        return Err(FileError::Invalid(
            "thumbnail bytes do not match the file extension".into(),
        ));
    }
    let directory = root.join("project-files").join("thumbnails");
    std::fs::create_dir_all(&directory).map_err(|error| FileError::Io(error.to_string()))?;
    let file_name = format!("{}.{extension}", uuid::Uuid::new_v4().simple());
    std::fs::write(directory.join(&file_name), bytes)
        .map_err(|error| FileError::Io(error.to_string()))?;
    Ok(format!("project-files/thumbnails/{file_name}"))
}

/// Resolve a stored project-relative image path, refusing anything that escapes
/// the project directory.
pub fn resolve_project_file(root: &Path, relative: &str) -> Result<PathBuf, FileError> {
    let trimmed = relative.trim();
    if trimmed.is_empty() {
        return Err(FileError::Invalid("file path cannot be empty".into()));
    }
    for segment in trimmed.split('/') {
        if segment.is_empty() || segment == "." || segment == ".." {
            return Err(FileError::Invalid(
                "file path cannot contain empty, '.' or '..' segments".into(),
            ));
        }
        if !segment
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | ' '))
        {
            return Err(FileError::Invalid(
                "file path contains an unsupported character".into(),
            ));
        }
    }
    if image_extension(trimmed).is_none() {
        return Err(FileError::Invalid(
            "file path must end with .png, .jpg, or .jpeg".into(),
        ));
    }
    let canonical_root = root
        .canonicalize()
        .map_err(|error| FileError::Io(error.to_string()))?;
    let canonical = root
        .join(trimmed)
        .canonicalize()
        .map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound => FileError::NotFound,
            _ => FileError::Io(error.to_string()),
        })?;
    if !canonical.starts_with(&canonical_root) {
        return Err(FileError::Invalid(
            "file path escapes the project directory".into(),
        ));
    }
    Ok(canonical)
}
