use std::fmt::Write as FmtWrite;
use std::fs;
use std::path::{Path, PathBuf};

use image::{DynamicImage, GenericImageView, ImageFormat};
use rusqlite::{params, OptionalExtension, Transaction};
use sha2::{Digest, Sha256};

use crate::error::{AppError, AppResult};
use crate::ids::create_id;
use crate::imaging::gif_pipeline::inspect_gif_bytes;
use crate::models::ImportImageFilePayload;
use crate::paths::AppPaths;

#[derive(Debug, Clone, Copy)]
pub struct SourceFileImportOptions {
    pub allow_gif: bool,
    pub exact_dimensions: Option<(i64, i64)>,
}

#[derive(Debug, Clone)]
pub struct StoredSourceFile {
    pub id: String,
    pub original_path_in_library: String,
    pub original_extension: String,
    pub width: i64,
    pub height: i64,
    pub thumbnail_path: String,
}

#[derive(Debug)]
struct ImageMetadata {
    extension: String,
    mime_type: String,
    width: i64,
    height: i64,
    byte_size: i64,
    sha256: String,
    is_animated: i64,
    frame_count: Option<i64>,
    original_loop_mode: String,
    original_loop_count: Option<i64>,
    image: DynamicImage,
}

pub fn import_source_file_from_bytes(
    transaction: &Transaction<'_>,
    paths: &AppPaths,
    file: &ImportImageFilePayload,
    options: SourceFileImportOptions,
) -> AppResult<StoredSourceFile> {
    let metadata = inspect_file(file, options)?;
    let source_file = ensure_source_file(transaction, paths, file, &metadata)?;
    let thumbnail_path = source_thumbnail_path(paths, &source_file.id);
    ensure_thumbnail(&metadata.image, &thumbnail_path)?;

    Ok(StoredSourceFile {
        thumbnail_path: path_string(&thumbnail_path),
        ..source_file
    })
}

fn inspect_file(
    file: &ImportImageFilePayload,
    options: SourceFileImportOptions,
) -> AppResult<ImageMetadata> {
    if file.bytes.is_empty() {
        return Err(AppError::new("validation", "빈 파일은 가져올 수 없습니다."));
    }

    let extension = normalized_extension(&file.original_filename).ok_or_else(|| {
        AppError::new(
            "validation",
            "jpg, jpeg, png, gif 파일만 가져올 수 있습니다.",
        )
    })?;
    if extension == "gif" && !options.allow_gif {
        return Err(AppError::new(
            "validation",
            "이 작업은 JPG 또는 PNG 파일만 지원합니다.",
        ));
    }

    let image_format = image_format_for_extension(&extension)
        .ok_or_else(|| AppError::new("validation", "지원하지 않는 이미지 형식입니다."))?;
    let image = image::load_from_memory_with_format(&file.bytes, image_format)
        .map_err(|_| AppError::new("validation", "이미지 파일을 해석할 수 없습니다."))?;
    let (width, height) = image.dimensions();

    if width == 0 || height == 0 {
        return Err(AppError::new(
            "validation",
            "가로세로 크기가 없는 이미지는 가져올 수 없습니다.",
        ));
    }

    if let Some((expected_width, expected_height)) = options.exact_dimensions {
        if i64::from(width) != expected_width || i64::from(height) != expected_height {
            return Err(AppError::new(
                "validation",
                format!(
                    "대표 이미지는 {expected_width}×{expected_height}px JPG/PNG만 사용할 수 있습니다."
                ),
            ));
        }
    }

    let gif_metadata = if extension == "gif" {
        Some(
            inspect_gif_bytes(&file.bytes)
                .map_err(|message| AppError::new("validation", message))?,
        )
    } else {
        None
    };
    let frame_count = gif_metadata.as_ref().map(|metadata| metadata.frame_count);
    let original_loop_mode = gif_metadata
        .as_ref()
        .map(|metadata| metadata.loop_mode.clone())
        .unwrap_or_else(|| "preserve".to_string());
    let original_loop_count = gif_metadata
        .as_ref()
        .and_then(|metadata| metadata.loop_count);

    Ok(ImageMetadata {
        extension: extension.clone(),
        mime_type: mime_type_for_extension(&extension).to_string(),
        width: i64::from(width),
        height: i64::from(height),
        byte_size: file.bytes.len() as i64,
        sha256: sha256_hex(&file.bytes),
        is_animated: i64::from(frame_count.unwrap_or(1) > 1),
        frame_count,
        original_loop_mode,
        original_loop_count,
        image,
    })
}

fn ensure_source_file(
    transaction: &Transaction<'_>,
    paths: &AppPaths,
    file: &ImportImageFilePayload,
    metadata: &ImageMetadata,
) -> AppResult<StoredSourceFile> {
    if let Some(source_file) = find_source_file(transaction, &metadata.sha256)? {
        ensure_original_bytes(&source_file.original_path_in_library, &file.bytes)?;
        return Ok(source_file);
    }

    let source_file_id = create_id("source");
    let original_path = original_library_path(paths, &metadata.sha256, &metadata.extension);
    if let Some(parent) = original_path.parent() {
        fs::create_dir_all(parent)?;
    }
    if !original_path.exists() {
        fs::write(&original_path, &file.bytes)?;
    }

    let original_path_in_library = path_string(&original_path);
    transaction.execute(
        "INSERT INTO source_files (
           id,
           original_filename,
           original_path_in_library,
           original_extension,
           mime_type,
           width,
           height,
           byte_size,
           sha256,
           is_animated,
           frame_count,
           original_loop_mode,
           original_loop_count,
           imported_from_path,
           created_at
         )
         VALUES (
           ?1,
           ?2,
           ?3,
           ?4,
           ?5,
           ?6,
           ?7,
           ?8,
           ?9,
           ?10,
           ?11,
           ?12,
           ?13,
           NULL,
           strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         )",
        params![
            source_file_id,
            file.original_filename,
            original_path_in_library,
            metadata.extension,
            metadata.mime_type,
            metadata.width,
            metadata.height,
            metadata.byte_size,
            metadata.sha256,
            metadata.is_animated,
            metadata.frame_count,
            metadata.original_loop_mode,
            metadata.original_loop_count,
        ],
    )?;

    Ok(StoredSourceFile {
        id: source_file_id,
        original_path_in_library,
        original_extension: metadata.extension.clone(),
        width: metadata.width,
        height: metadata.height,
        thumbnail_path: String::new(),
    })
}

fn find_source_file(
    transaction: &Transaction<'_>,
    sha256: &str,
) -> AppResult<Option<StoredSourceFile>> {
    transaction
        .query_row(
            "SELECT
               id,
               original_path_in_library,
               original_extension,
               width,
               height
             FROM source_files
             WHERE sha256 = ?1",
            params![sha256],
            |row| {
                Ok(StoredSourceFile {
                    id: row.get("id")?,
                    original_path_in_library: row.get("original_path_in_library")?,
                    original_extension: row.get("original_extension")?,
                    width: row.get("width")?,
                    height: row.get("height")?,
                    thumbnail_path: String::new(),
                })
            },
        )
        .optional()
        .map_err(AppError::from)
}

fn ensure_original_bytes(path: &str, bytes: &[u8]) -> AppResult<()> {
    let path = PathBuf::from(path);
    if path.exists() {
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, bytes)?;

    Ok(())
}

fn ensure_thumbnail(image: &DynamicImage, thumbnail_path: &Path) -> AppResult<()> {
    if thumbnail_path.exists() {
        return Ok(());
    }

    if let Some(parent) = thumbnail_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let thumbnail = image.thumbnail(256, 256);
    thumbnail.save_with_format(thumbnail_path, ImageFormat::Png)?;

    Ok(())
}

fn normalized_extension(filename: &str) -> Option<String> {
    let extension = Path::new(filename)
        .extension()
        .and_then(|extension| extension.to_str())?
        .trim()
        .to_ascii_lowercase();

    match extension.as_str() {
        "jpg" | "jpeg" | "png" | "gif" => Some(extension),
        _ => None,
    }
}

fn image_format_for_extension(extension: &str) -> Option<ImageFormat> {
    match extension {
        "jpg" | "jpeg" => Some(ImageFormat::Jpeg),
        "png" => Some(ImageFormat::Png),
        "gif" => Some(ImageFormat::Gif),
        _ => None,
    }
}

fn mime_type_for_extension(extension: &str) -> &'static str {
    match extension {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        _ => "application/octet-stream",
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(64);

    for byte in digest {
        let _ = write!(&mut output, "{byte:02x}");
    }

    output
}

fn original_library_path(paths: &AppPaths, sha256: &str, extension: &str) -> PathBuf {
    let prefix = sha256.get(0..2).unwrap_or("00");
    paths
        .originals_dir
        .join(prefix)
        .join(format!("{sha256}.{extension}"))
}

fn source_thumbnail_path(paths: &AppPaths, source_file_id: &str) -> PathBuf {
    paths
        .source_file_thumbnails_dir
        .join(format!("{source_file_id}.png"))
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}
