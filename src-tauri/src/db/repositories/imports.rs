use std::fmt::Write as FmtWrite;
use std::fs;
use std::path::{Path, PathBuf};

use image::{DynamicImage, GenericImageView, ImageFormat};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use sha2::{Digest, Sha256};

use crate::db::repositories::{collections as collection_repository, icons as icon_repository};
use crate::error::{AppError, AppResult};
use crate::ids::create_id;
use crate::imaging::gif_pipeline::inspect_gif_bytes;
use crate::models::{ImportImageFilePayload, ImportImagesResultDto, RejectedImportFileDto};
use crate::paths::AppPaths;

pub fn import_image_files(
    connection: &mut Connection,
    paths: &AppPaths,
    collection_id: &str,
    files: Vec<ImportImageFilePayload>,
) -> AppResult<ImportImagesResultDto> {
    let transaction = connection.transaction()?;
    let collection = load_collection_for_import(&transaction, collection_id)?;
    let mut next_order_index = next_icon_order_index(&transaction, collection_id)?;
    let mut has_cover =
        collection.cover_icon_id.is_some() || collection.cover_source_file_id.is_some();
    let mut imported_icon_ids = Vec::new();
    let mut rejected_files = Vec::new();

    for file in files {
        let original_filename = file.original_filename.clone();

        match inspect_import_file(&file) {
            Ok(metadata) => {
                let source_file = ensure_source_file(&transaction, paths, &file, &metadata)?;
                let display_name = display_name_from_filename(&file.original_filename);
                let thumbnail_path = source_thumbnail_path(paths, &source_file.id);
                ensure_thumbnail(&metadata.image, &thumbnail_path)?;

                let current_preview_path = if source_file.original_extension == "gif" {
                    PathBuf::from(&source_file.original_path_in_library)
                } else {
                    thumbnail_path.clone()
                };

                let icon_id = insert_icon(
                    &transaction,
                    &collection,
                    &source_file,
                    &display_name,
                    next_order_index,
                    &thumbnail_path,
                    &current_preview_path,
                )?;
                next_order_index += 1;

                if !has_cover {
                    set_cover(&transaction, collection_id, &icon_id, &source_file.id)?;
                    has_cover = true;
                }

                imported_icon_ids.push(icon_id);
            }
            Err(reason) => rejected_files.push(RejectedImportFileDto {
                original_filename,
                reason,
            }),
        }
    }

    transaction.commit()?;

    let collection = collection_repository::get_collection(connection, collection_id)?;
    let imported_icons = icon_repository::list_icons(connection, collection_id)?
        .into_iter()
        .filter(|icon| imported_icon_ids.contains(&icon.id))
        .collect();

    Ok(ImportImagesResultDto {
        collection,
        imported_icons,
        rejected_files,
    })
}

#[derive(Debug)]
struct CollectionImportRecord {
    id: String,
    cover_source_file_id: Option<String>,
    cover_icon_id: Option<String>,
    default_cell_width: i64,
    default_cell_height: i64,
}

#[derive(Debug)]
struct SourceFileRecord {
    id: String,
    original_path_in_library: String,
    original_extension: String,
    width: i64,
    height: i64,
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

#[derive(Debug)]
struct CropRect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

fn load_collection_for_import(
    transaction: &Transaction<'_>,
    collection_id: &str,
) -> AppResult<CollectionImportRecord> {
    transaction
        .query_row(
            "SELECT
               id,
               cover_source_file_id,
               cover_icon_id,
               default_cell_width,
               default_cell_height
             FROM collections
             WHERE id = ?1
               AND deleted_at IS NULL",
            params![collection_id],
            |row| {
                Ok(CollectionImportRecord {
                    id: row.get("id")?,
                    cover_source_file_id: row.get("cover_source_file_id")?,
                    cover_icon_id: row.get("cover_icon_id")?,
                    default_cell_width: row.get("default_cell_width")?,
                    default_cell_height: row.get("default_cell_height")?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("이미지를 가져올 모음을 찾을 수 없습니다."))
}

fn inspect_import_file(file: &ImportImageFilePayload) -> Result<ImageMetadata, String> {
    if file.bytes.is_empty() {
        return Err("빈 파일은 가져올 수 없습니다.".to_string());
    }

    let extension = normalized_extension(&file.original_filename)
        .ok_or_else(|| "jpg, jpeg, png, gif 파일만 가져올 수 있습니다.".to_string())?;
    let image_format = image_format_for_extension(&extension)
        .ok_or_else(|| "jpg, jpeg, png, gif 파일만 가져올 수 있습니다.".to_string())?;
    let image = image::load_from_memory_with_format(&file.bytes, image_format)
        .map_err(|_| "이미지 파일을 해석할 수 없습니다.".to_string())?;
    let (width, height) = image.dimensions();

    if width == 0 || height == 0 {
        return Err("가로세로 크기가 없는 이미지는 가져올 수 없습니다.".to_string());
    }

    let gif_metadata = if extension == "gif" {
        Some(inspect_gif_bytes(&file.bytes)?)
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
) -> AppResult<SourceFileRecord> {
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

    Ok(SourceFileRecord {
        id: source_file_id,
        original_path_in_library,
        original_extension: metadata.extension.clone(),
        width: metadata.width,
        height: metadata.height,
    })
}

fn find_source_file(
    transaction: &Transaction<'_>,
    sha256: &str,
) -> AppResult<Option<SourceFileRecord>> {
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
                Ok(SourceFileRecord {
                    id: row.get("id")?,
                    original_path_in_library: row.get("original_path_in_library")?,
                    original_extension: row.get("original_extension")?,
                    width: row.get("width")?,
                    height: row.get("height")?,
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

fn insert_icon(
    transaction: &Transaction<'_>,
    collection: &CollectionImportRecord,
    source_file: &SourceFileRecord,
    display_name: &str,
    order_index: i64,
    thumbnail_path: &Path,
    current_preview_path: &Path,
) -> AppResult<String> {
    let icon_id = create_id("icon");
    transaction.execute(
        "INSERT INTO icons (
           id,
           collection_id,
           source_file_id,
           display_name,
           shape,
           order_index,
           thumbnail_path,
           current_preview_path,
           created_at,
           updated_at
         )
         VALUES (
           ?1,
           ?2,
           ?3,
           ?4,
           'single',
           ?5,
           ?6,
           ?7,
           strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
           strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         )",
        params![
            icon_id,
            collection.id,
            source_file.id,
            display_name,
            order_index,
            path_string(thumbnail_path),
            path_string(current_preview_path),
        ],
    )?;

    insert_default_crop_settings(transaction, &icon_id, collection, source_file)?;
    insert_single_icon_piece(transaction, &icon_id)?;

    Ok(icon_id)
}

fn insert_default_crop_settings(
    transaction: &Transaction<'_>,
    icon_id: &str,
    collection: &CollectionImportRecord,
    source_file: &SourceFileRecord,
) -> AppResult<()> {
    let crop = centered_crop_rect(
        source_file.width,
        source_file.height,
        collection.default_cell_width,
        collection.default_cell_height,
    );

    transaction.execute(
        "INSERT INTO crop_settings (
           id,
           icon_id,
           crop_mode,
           crop_x,
           crop_y,
           crop_w,
           crop_h,
           preset_position,
           source_width_at_apply,
           source_height_at_apply,
           viewport_width_at_apply,
           viewport_height_at_apply,
           updated_at
         )
         VALUES (
           ?1,
           ?2,
           'free',
           ?3,
           ?4,
           ?5,
           ?6,
           'center',
           ?7,
           ?8,
           ?9,
           ?10,
           strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         )",
        params![
            create_id("crop"),
            icon_id,
            crop.x,
            crop.y,
            crop.width,
            crop.height,
            source_file.width,
            source_file.height,
            collection.default_cell_width,
            collection.default_cell_height,
        ],
    )?;

    Ok(())
}

fn insert_single_icon_piece(transaction: &Transaction<'_>, icon_id: &str) -> AppResult<()> {
    transaction.execute(
        "INSERT INTO icon_pieces (
           id,
           icon_id,
           piece_index,
           piece_role,
           alt_text,
           created_at,
           updated_at
         )
         VALUES (
           ?1,
           ?2,
           0,
           'single',
           ?3,
           strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
           strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         )",
        params![create_id("piece"), icon_id, ""],
    )?;

    Ok(())
}

fn set_cover(
    transaction: &Transaction<'_>,
    collection_id: &str,
    icon_id: &str,
    source_file_id: &str,
) -> AppResult<()> {
    transaction.execute(
        "UPDATE collections
         SET cover_icon_id = ?1,
             cover_source_file_id = ?2,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?3
           AND deleted_at IS NULL",
        params![icon_id, source_file_id, collection_id],
    )?;

    Ok(())
}

fn next_icon_order_index(transaction: &Transaction<'_>, collection_id: &str) -> AppResult<i64> {
    Ok(transaction.query_row(
        "SELECT COALESCE(MAX(order_index) + 1, 0)
         FROM icons
         WHERE collection_id = ?1
           AND deleted_at IS NULL",
        params![collection_id],
        |row| row.get(0),
    )?)
}

fn centered_crop_rect(
    source_width: i64,
    source_height: i64,
    cell_width: i64,
    cell_height: i64,
) -> CropRect {
    let source_width = source_width.max(1) as f64;
    let source_height = source_height.max(1) as f64;
    let target_aspect = cell_width.max(1) as f64 / cell_height.max(1) as f64;
    let source_aspect = source_width / source_height;

    let (crop_width, crop_height) = if source_aspect > target_aspect {
        (source_height * target_aspect, source_height)
    } else {
        (source_width, source_width / target_aspect)
    };

    CropRect {
        x: ((source_width - crop_width) / 2.0).max(0.0),
        y: ((source_height - crop_height) / 2.0).max(0.0),
        width: crop_width.min(source_width),
        height: crop_height.min(source_height),
    }
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

fn display_name_from_filename(filename: &str) -> String {
    Path::new(filename)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(str::trim)
        .filter(|stem| !stem.is_empty())
        .unwrap_or("새 아이콘")
        .to_string()
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

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::time::{SystemTime, UNIX_EPOCH};

    use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};
    use rusqlite::Connection;

    use crate::db::migrations;
    use crate::db::repositories::collections::create_collection;
    use crate::models::ImportImageFilePayload;
    use crate::paths::AppPaths;

    use super::import_image_files;

    fn connection() -> Connection {
        let mut connection = Connection::open_in_memory().unwrap();
        migrations::run(&mut connection).unwrap();
        connection
    }

    fn temp_paths() -> AppPaths {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        AppPaths::prepare(std::env::temp_dir().join(format!("pmtconcon-import-{suffix}"))).unwrap()
    }

    fn png_bytes() -> Vec<u8> {
        let image = ImageBuffer::from_pixel(20, 16, Rgba([255, 0, 0, 255]));
        let mut cursor = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image)
            .write_to(&mut cursor, ImageFormat::Png)
            .unwrap();
        cursor.into_inner()
    }

    fn gif_bytes() -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut encoder = gif::Encoder::new(&mut bytes, 8, 8, &[]).unwrap();
            encoder.set_repeat(gif::Repeat::Finite(3)).unwrap();

            for color in [[255, 0, 0, 255], [0, 255, 0, 255]] {
                let mut pixels = Vec::with_capacity(8 * 8 * 4);
                for _ in 0..(8 * 8) {
                    pixels.extend_from_slice(&color);
                }
                let frame = gif::Frame::from_rgba_speed(8, 8, &mut pixels, 10);
                encoder.write_frame(&frame).unwrap();
            }
        }

        bytes
    }

    #[test]
    fn import_copies_original_and_creates_icon_rows() {
        let mut connection = connection();
        let paths = temp_paths();
        let collection =
            create_collection(&mut connection, Some("가져오기 테스트".to_string())).unwrap();
        let bytes = png_bytes();

        let result = import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![ImportImageFilePayload {
                original_filename: "first.png".to_string(),
                bytes,
            }],
        )
        .unwrap();

        assert_eq!(result.imported_icons.len(), 1);
        assert!(result.rejected_files.is_empty());
        assert_eq!(
            result.collection.cover_icon_id,
            Some(result.imported_icons[0].id.clone())
        );
        assert_eq!(result.imported_icons[0].order_index, 0);
        assert!(result.imported_icons[0].thumbnail_url.is_some());
        assert!(result.imported_icons[0].current_preview_url.is_some());
        assert_eq!(result.imported_icons[0].pieces[0].alt_text, "");

        let source_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM source_files", [], |row| row.get(0))
            .unwrap();
        let icon_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM icons", [], |row| row.get(0))
            .unwrap();
        assert_eq!(source_count, 1);
        assert_eq!(icon_count, 1);

        let original_path: String = connection
            .query_row(
                "SELECT original_path_in_library FROM source_files",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(std::path::Path::new(&original_path).exists());

        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn import_rejects_unsupported_files_and_reuses_duplicate_sources() {
        let mut connection = connection();
        let paths = temp_paths();
        let collection =
            create_collection(&mut connection, Some("중복 테스트".to_string())).unwrap();
        let bytes = png_bytes();

        let result = import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![
                ImportImageFilePayload {
                    original_filename: "one.png".to_string(),
                    bytes: bytes.clone(),
                },
                ImportImageFilePayload {
                    original_filename: "two.png".to_string(),
                    bytes,
                },
                ImportImageFilePayload {
                    original_filename: "notes.txt".to_string(),
                    bytes: b"not an image".to_vec(),
                },
            ],
        )
        .unwrap();

        assert_eq!(result.imported_icons.len(), 2);
        assert_eq!(result.rejected_files.len(), 1);
        assert_eq!(result.imported_icons[0].order_index, 0);
        assert_eq!(result.imported_icons[1].order_index, 1);

        let source_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM source_files", [], |row| row.get(0))
            .unwrap();
        let icon_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM icons", [], |row| row.get(0))
            .unwrap();
        assert_eq!(source_count, 1);
        assert_eq!(icon_count, 2);

        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn import_gif_records_animation_metadata_and_keeps_original_gif() {
        let mut connection = connection();
        let paths = temp_paths();
        let collection =
            create_collection(&mut connection, Some("GIF 가져오기 테스트".to_string())).unwrap();
        let bytes = gif_bytes();

        let result = import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![ImportImageFilePayload {
                original_filename: "loop.gif".to_string(),
                bytes: bytes.clone(),
            }],
        )
        .unwrap();

        assert_eq!(result.imported_icons.len(), 1);
        assert!(result.imported_icons[0]
            .current_preview_url
            .as_deref()
            .is_some_and(|path| path.ends_with(".gif")));

        let source: (i64, Option<i64>, String, Option<i64>, String) = connection
            .query_row(
                "SELECT
                   is_animated,
                   frame_count,
                   original_loop_mode,
                   original_loop_count,
                   original_path_in_library
                 FROM source_files",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .unwrap();

        assert_eq!(source.0, 1);
        assert_eq!(source.1, Some(2));
        assert_eq!(source.2, "count");
        assert_eq!(source.3, Some(3));
        assert_eq!(std::fs::read(source.4).unwrap(), bytes);

        std::fs::remove_dir_all(paths.root).unwrap();
    }
}
