use std::collections::HashSet;
use std::fmt::Write as FmtWrite;
use std::fs::{self, File, OpenOptions};
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use image::codecs::gif::GifDecoder;
use image::{AnimationDecoder, DynamicImage, GenericImageView, ImageFormat};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use sha2::{Digest, Sha256};

use crate::error::{AppError, AppResult};
use crate::ids::create_id;
use crate::imaging::gif_pipeline::inspect_gif_bytes;
use crate::imaging::import_limits::decode_import_image;
use crate::models::ImportImageFilePayload;
use crate::paths::AppPaths;

const SOURCE_FILE_ORPHAN_GRACE_PERIOD: Duration = Duration::from_secs(24 * 60 * 60);

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
    pub byte_size: i64,
    pub sha256: String,
    pub mime_type: String,
    pub is_animated: bool,
    pub frame_count: Option<i64>,
    pub original_loop_mode: String,
    pub original_loop_count: Option<i64>,
    pub has_alpha: Option<bool>,
    pub thumbnail_path: String,
}

#[derive(Debug)]
pub(crate) struct PreparedSourceFile {
    planned_source_file_id: String,
    file: ImportImageFilePayload,
    metadata: ImageMetadata,
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedSourceArtifactSnapshot {
    sha256: String,
    source_file_id: String,
    original_root: PathBuf,
    original_path: PathBuf,
    thumbnail_root: PathBuf,
    thumbnail_path: PathBuf,
    original_existed: bool,
    thumbnail_existed: bool,
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
    has_alpha: i64,
    image: DynamicImage,
}

impl PreparedSourceFile {
    pub(crate) fn artifact_snapshot(
        &self,
        connection: &Connection,
        paths: &AppPaths,
    ) -> AppResult<PreparedSourceArtifactSnapshot> {
        let existing = connection
            .query_row(
                "SELECT id, original_path_in_library FROM source_files WHERE sha256 = ?1",
                [self.metadata.sha256.as_str()],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        let (source_file_id, original_path) = existing
            .map(|(id, path)| (id, PathBuf::from(path)))
            .unwrap_or_else(|| {
                (
                    self.planned_source_file_id.clone(),
                    self.planned_original_path(paths),
                )
            });
        let thumbnail_path = source_thumbnail_path(paths, &source_file_id);
        validate_managed_artifact_path(&paths.originals_dir, &original_path, "원본")?;
        validate_managed_artifact_path(
            &paths.source_file_thumbnails_dir,
            &thumbnail_path,
            "썸네일",
        )?;
        Ok(PreparedSourceArtifactSnapshot {
            sha256: self.metadata.sha256.clone(),
            source_file_id,
            original_root: paths.originals_dir.clone(),
            original_existed: entry_exists(&original_path)?,
            original_path,
            thumbnail_root: paths.source_file_thumbnails_dir.clone(),
            thumbnail_existed: entry_exists(&thumbnail_path)?,
            thumbnail_path,
        })
    }
    pub(crate) fn planned_source_file_id(&self) -> &str {
        &self.planned_source_file_id
    }

    pub(crate) fn original_filename(&self) -> &str {
        &self.file.original_filename
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.file.bytes
    }

    pub(crate) fn decoded_image(&self) -> &DynamicImage {
        &self.metadata.image
    }

    pub(crate) fn planned_original_path(&self, paths: &AppPaths) -> PathBuf {
        original_library_path(paths, &self.metadata.sha256, &self.metadata.extension)
    }

    pub(crate) fn planned_source_file(&self, paths: &AppPaths) -> StoredSourceFile {
        let original_path = self.planned_original_path(paths);
        let thumbnail_path = source_thumbnail_path(paths, &self.planned_source_file_id);

        StoredSourceFile {
            id: self.planned_source_file_id.clone(),
            original_path_in_library: path_string(&original_path),
            original_extension: self.metadata.extension.clone(),
            width: self.metadata.width,
            height: self.metadata.height,
            byte_size: self.metadata.byte_size,
            sha256: self.metadata.sha256.clone(),
            mime_type: self.metadata.mime_type.clone(),
            is_animated: self.metadata.is_animated != 0,
            frame_count: self.metadata.frame_count,
            original_loop_mode: self.metadata.original_loop_mode.clone(),
            original_loop_count: self.metadata.original_loop_count,
            has_alpha: Some(self.metadata.has_alpha != 0),
            thumbnail_path: path_string(&thumbnail_path),
        }
    }
}

impl PreparedSourceArtifactSnapshot {
    pub(crate) fn cleanup_if_unreferenced(&self, connection: &Connection) -> AppResult<()> {
        if !self.thumbnail_existed {
            let referenced = connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM source_files WHERE id = ?1)",
                [self.source_file_id.as_str()],
                |row| row.get::<_, i64>(0),
            )? != 0;
            if !referenced {
                remove_managed_artifact_file(&self.thumbnail_root, &self.thumbnail_path)?;
            }
        }
        if !self.original_existed {
            let referenced = connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM source_files WHERE sha256 = ?1)",
                [self.sha256.as_str()],
                |row| row.get::<_, i64>(0),
            )? != 0;
            if !referenced {
                remove_managed_artifact_file(&self.original_root, &self.original_path)?;
            }
        }
        Ok(())
    }
}

#[derive(Default)]
struct SourceFileCleanupReferences {
    original_paths: HashSet<PathBuf>,
    sha256_values: HashSet<String>,
    source_file_ids: HashSet<String>,
}

struct SourceFileCleanupCandidate {
    path: PathBuf,
    canonical_parent: PathBuf,
}

struct OwnedOriginalArtifact {
    sha256: String,
    target_file_name: String,
}

struct OwnedThumbnailArtifact {
    source_file_id: String,
}

pub(crate) fn cleanup_source_file_crash_orphans(
    connection: &Connection,
    paths: &AppPaths,
) -> AppResult<()> {
    cleanup_source_file_crash_orphans_at(connection, paths, SystemTime::now())
}

fn cleanup_source_file_crash_orphans_at(
    connection: &Connection,
    paths: &AppPaths,
    now: SystemTime,
) -> AppResult<()> {
    let references = load_source_file_cleanup_references(connection, paths)?;
    let mut candidates = collect_original_cleanup_candidates(paths, &references, now)?;
    candidates.extend(collect_thumbnail_cleanup_candidates(
        paths,
        &references,
        now,
    )?);

    for candidate in candidates {
        remove_expired_regular_file(&candidate.path, &candidate.canonical_parent, now)?;
    }
    Ok(())
}

fn load_source_file_cleanup_references(
    connection: &Connection,
    paths: &AppPaths,
) -> AppResult<SourceFileCleanupReferences> {
    let mut statement = connection.prepare(
        "SELECT id, original_path_in_library, sha256, original_extension FROM source_files",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
        ))
    })?;
    let mut references = SourceFileCleanupReferences::default();

    for row in rows {
        let (source_file_id, stored_path, sha256, extension) = row?;
        if source_file_id.is_empty()
            || source_file_id.chars().any(char::is_control)
            || !is_lower_hex(&sha256, 64)
            || !is_owned_original_extension(&extension)
        {
            return Err(AppError::new(
                "source_cleanup",
                "source_files 메타데이터가 예상 형식과 달라 고아 파일 정리를 건너뜁니다.",
            ));
        }

        let expected_path = original_library_path(paths, &sha256, &extension);
        if Path::new(&stored_path) != expected_path {
            return Err(AppError::new(
                "source_cleanup",
                "DB의 원본 경로가 관리 원본 폴더와 일치하지 않아 고아 파일 정리를 건너뜁니다.",
            ));
        }

        references.original_paths.insert(expected_path);
        references.sha256_values.insert(sha256);
        references.source_file_ids.insert(source_file_id);
    }

    Ok(references)
}

fn collect_original_cleanup_candidates(
    paths: &AppPaths,
    references: &SourceFileCleanupReferences,
    now: SystemTime,
) -> AppResult<Vec<SourceFileCleanupCandidate>> {
    let canonical_root = canonical_regular_directory(&paths.originals_dir)?;
    let mut candidates = Vec::new();

    for entry in fs::read_dir(&paths.originals_dir)? {
        let entry = entry?;
        let bucket_name = entry.file_name();
        let Some(bucket_name) = bucket_name.to_str() else {
            continue;
        };
        if !is_lower_hex(bucket_name, 2) {
            continue;
        }

        let bucket_path = entry.path();
        let Ok(metadata) = fs::symlink_metadata(&bucket_path) else {
            continue;
        };
        if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
            continue;
        }
        let Ok(canonical_bucket) = bucket_path.canonicalize() else {
            continue;
        };
        if canonical_bucket != canonical_root.join(bucket_name) {
            continue;
        }

        for artifact_entry in fs::read_dir(&bucket_path)? {
            let artifact_entry = artifact_entry?;
            let artifact_name = artifact_entry.file_name();
            let Some(artifact_name) = artifact_name.to_str() else {
                continue;
            };
            let Some(owned) = parse_owned_original_artifact(artifact_name, bucket_name) else {
                continue;
            };
            let target_path = bucket_path.join(&owned.target_file_name);
            if references.sha256_values.contains(&owned.sha256)
                || references.original_paths.contains(&target_path)
            {
                continue;
            }

            let artifact_path = artifact_entry.path();
            if is_expired_regular_file(&artifact_path, now) {
                candidates.push(SourceFileCleanupCandidate {
                    path: artifact_path,
                    canonical_parent: canonical_bucket.clone(),
                });
            }
        }
    }

    Ok(candidates)
}

fn collect_thumbnail_cleanup_candidates(
    paths: &AppPaths,
    references: &SourceFileCleanupReferences,
    now: SystemTime,
) -> AppResult<Vec<SourceFileCleanupCandidate>> {
    let canonical_root = canonical_regular_directory(&paths.source_file_thumbnails_dir)?;
    let mut candidates = Vec::new();

    for entry in fs::read_dir(&paths.source_file_thumbnails_dir)? {
        let entry = entry?;
        let artifact_name = entry.file_name();
        let Some(artifact_name) = artifact_name.to_str() else {
            continue;
        };
        let Some(owned) = parse_owned_thumbnail_artifact(artifact_name) else {
            continue;
        };
        if references.source_file_ids.contains(&owned.source_file_id) {
            continue;
        }

        let artifact_path = entry.path();
        if is_expired_regular_file(&artifact_path, now) {
            candidates.push(SourceFileCleanupCandidate {
                path: artifact_path,
                canonical_parent: canonical_root.clone(),
            });
        }
    }

    Ok(candidates)
}

fn canonical_regular_directory(path: &Path) -> AppResult<PathBuf> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
        return Err(AppError::new(
            "source_cleanup",
            "고아 파일 정리 루트가 실제 관리 디렉터리가 아니어서 정리를 건너뜁니다.",
        ));
    }
    Ok(path.canonicalize()?)
}

fn parse_owned_original_artifact(
    file_name: &str,
    bucket_name: &str,
) -> Option<OwnedOriginalArtifact> {
    let target_file_name = strip_owned_transient_suffix(file_name)?;
    let (sha256, extension) = target_file_name.rsplit_once('.')?;
    if !is_lower_hex(sha256, 64)
        || sha256.get(..2)? != bucket_name
        || !is_owned_original_extension(extension)
    {
        return None;
    }

    Some(OwnedOriginalArtifact {
        sha256: sha256.to_string(),
        target_file_name: target_file_name.to_string(),
    })
}

fn parse_owned_thumbnail_artifact(file_name: &str) -> Option<OwnedThumbnailArtifact> {
    let target_file_name = strip_owned_transient_suffix(file_name)?;
    let source_file_id = target_file_name.strip_suffix(".png")?;
    if !is_generated_source_file_id(source_file_id) {
        return None;
    }
    Some(OwnedThumbnailArtifact {
        source_file_id: source_file_id.to_string(),
    })
}

fn strip_owned_transient_suffix(file_name: &str) -> Option<&str> {
    if let Some(transient_name) = file_name.strip_prefix('.') {
        return transient_name
            .strip_suffix(".incoming")
            .or_else(|| transient_name.strip_suffix(".repair-backup"));
    }
    Some(file_name)
}

fn is_owned_original_extension(extension: &str) -> bool {
    matches!(extension, "jpg" | "jpeg" | "png" | "gif")
}

fn is_generated_source_file_id(value: &str) -> bool {
    let Some(suffix) = value.strip_prefix("source_") else {
        return false;
    };
    let Some((timestamp, counter)) = suffix.split_once('_') else {
        return false;
    };
    is_lower_hex(timestamp, 32) && is_lower_hex(counter, 8)
}

fn is_lower_hex(value: &str, expected_len: usize) -> bool {
    value.len() == expected_len
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn is_expired_regular_file(path: &Path, now: SystemTime) -> bool {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return false;
    };
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return false;
    }
    let Ok(modified) = metadata.modified() else {
        return false;
    };
    now.duration_since(modified)
        .map(|age| age >= SOURCE_FILE_ORPHAN_GRACE_PERIOD)
        .unwrap_or(false)
}

fn remove_expired_regular_file(
    path: &Path,
    expected_canonical_parent: &Path,
    now: SystemTime,
) -> AppResult<()> {
    if !is_expired_regular_file(path, now) {
        return Ok(());
    }
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    let Ok(canonical_parent) = parent.canonicalize() else {
        return Ok(());
    };
    if canonical_parent != expected_canonical_parent {
        return Ok(());
    }

    fs::remove_file(path)?;
    Ok(())
}
pub fn import_source_file_from_bytes(
    transaction: &Transaction<'_>,
    paths: &AppPaths,
    file: &ImportImageFilePayload,
    options: SourceFileImportOptions,
) -> AppResult<StoredSourceFile> {
    let prepared = prepare_source_file_from_bytes(file, options)?;
    commit_prepared_source_file(transaction, paths, &prepared)
}

pub(crate) fn prepare_source_file_from_bytes(
    file: &ImportImageFilePayload,
    options: SourceFileImportOptions,
) -> AppResult<PreparedSourceFile> {
    let metadata = inspect_file(file, options)?;

    Ok(PreparedSourceFile {
        planned_source_file_id: create_id("source"),
        file: file.clone(),
        metadata,
    })
}

pub(crate) fn commit_prepared_source_file(
    transaction: &Transaction<'_>,
    paths: &AppPaths,
    prepared: &PreparedSourceFile,
) -> AppResult<StoredSourceFile> {
    let source_file = ensure_source_file(transaction, paths, prepared)?;
    let thumbnail_path = source_thumbnail_path(paths, &source_file.id);
    ensure_thumbnail(
        &prepared.metadata.image,
        &paths.source_file_thumbnails_dir,
        &thumbnail_path,
    )?;

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
    let image = decode_import_image(&file.bytes, image_format)?;
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
    let has_alpha = i64::from(decoded_has_alpha(&file.bytes, &extension, &image)?);

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
        has_alpha,
        image,
    })
}

fn ensure_source_file(
    transaction: &Transaction<'_>,
    paths: &AppPaths,
    prepared: &PreparedSourceFile,
) -> AppResult<StoredSourceFile> {
    let file = &prepared.file;
    let metadata = &prepared.metadata;
    if let Some(mut source_file) = find_source_file(transaction, &metadata.sha256)? {
        ensure_original_bytes(
            &paths.originals_dir,
            Path::new(&source_file.original_path_in_library),
            &file.bytes,
        )?;
        if source_file.has_alpha.is_none() {
            transaction.execute(
                "UPDATE source_files SET has_alpha = ?1 WHERE id = ?2 AND has_alpha IS NULL",
                params![metadata.has_alpha, source_file.id],
            )?;
            source_file.has_alpha = Some(metadata.has_alpha != 0);
        }
        return Ok(source_file);
    }

    let source_file_id = prepared.planned_source_file_id.clone();
    let original_path = original_library_path(paths, &metadata.sha256, &metadata.extension);
    if let Some(parent) = original_path.parent() {
        fs::create_dir_all(parent)?;
    }
    ensure_original_bytes(&paths.originals_dir, &original_path, &file.bytes)?;

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
           has_alpha,
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
           ?14,
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
            metadata.has_alpha,
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
        byte_size: metadata.byte_size,
        sha256: metadata.sha256.clone(),
        mime_type: metadata.mime_type.clone(),
        is_animated: metadata.is_animated != 0,
        frame_count: metadata.frame_count,
        original_loop_mode: metadata.original_loop_mode.clone(),
        original_loop_count: metadata.original_loop_count,
        has_alpha: Some(metadata.has_alpha != 0),
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
               height,
               byte_size,
               sha256,
               mime_type,
               is_animated,
               frame_count,
               original_loop_mode,
               original_loop_count,
               has_alpha
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
                    byte_size: row.get("byte_size")?,
                    sha256: row.get("sha256")?,
                    mime_type: row.get("mime_type")?,
                    is_animated: row.get::<_, i64>("is_animated")? != 0,
                    frame_count: row.get("frame_count")?,
                    original_loop_mode: row
                        .get::<_, Option<String>>("original_loop_mode")?
                        .unwrap_or_else(|| "preserve".to_string()),
                    original_loop_count: row.get("original_loop_count")?,
                    has_alpha: row
                        .get::<_, Option<i64>>("has_alpha")?
                        .map(|value| value != 0),
                    thumbnail_path: String::new(),
                })
            },
        )
        .optional()
        .map_err(AppError::from)
}

fn ensure_original_bytes(root: &Path, path: &Path, bytes: &[u8]) -> AppResult<()> {
    fs::create_dir_all(root)?;
    let root = root.canonicalize()?;
    let parent = path.parent().ok_or_else(|| {
        AppError::new(
            "source_path",
            "원본 라이브러리 파일의 상위 경로가 없습니다.",
        )
    })?;
    fs::create_dir_all(parent)?;
    let parent = parent.canonicalize()?;
    if !parent.starts_with(&root) {
        return Err(AppError::new(
            "source_path",
            "관리되는 원본 라이브러리 밖의 파일은 복구할 수 없습니다.",
        ));
    }

    let expected_sha256 = sha256_hex(bytes);
    if original_file_matches(path, bytes.len(), &expected_sha256)? {
        return Ok(());
    }

    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| AppError::new("source_path", "원본 파일 이름이 올바르지 않습니다."))?;
    let incoming = parent.join(format!(".{file_name}.incoming"));
    let backup = parent.join(format!(".{file_name}.repair-backup"));
    remove_managed_file_if_present(&incoming)?;
    if path.exists() {
        remove_managed_file_if_present(&backup)?;
    }

    let write_result = (|| -> AppResult<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&incoming)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        Ok(())
    })();
    if let Err(error) = write_result {
        let _ = remove_managed_file_if_present(&incoming);
        return Err(error);
    }

    let had_existing = path.exists();
    if had_existing {
        if let Err(error) = fs::rename(path, &backup) {
            let _ = remove_managed_file_if_present(&incoming);
            return Err(error.into());
        }
    }
    if let Err(error) = fs::rename(&incoming, path) {
        if had_existing {
            let _ = fs::rename(&backup, path);
        }
        let _ = remove_managed_file_if_present(&incoming);
        return Err(error.into());
    }
    if !original_file_matches(path, bytes.len(), &expected_sha256)? {
        let _ = remove_managed_file_if_present(path);
        if had_existing {
            let _ = fs::rename(&backup, path);
        }
        return Err(AppError::new(
            "source_integrity",
            "원본 라이브러리 파일을 검증된 바이트로 복구하지 못했습니다.",
        ));
    }
    if had_existing {
        remove_managed_file_if_present(&backup)?;
    }
    Ok(())
}

fn original_file_matches(
    path: &Path,
    expected_len: usize,
    expected_sha256: &str,
) -> AppResult<bool> {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return Ok(false);
    };
    if !metadata.file_type().is_file()
        || usize::try_from(metadata.len()).unwrap_or(usize::MAX) != expected_len
    {
        return Ok(false);
    }
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let actual = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Ok(actual == expected_sha256)
}

fn remove_managed_file_if_present(path: &Path) -> AppResult<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_dir() => Err(AppError::new(
            "source_path",
            "원본 파일 복구 경로에 디렉터리가 있어 작업을 중단했습니다.",
        )),
        Ok(_) => {
            fs::remove_file(path)?;
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn ensure_thumbnail(
    source_image: &DynamicImage,
    root: &Path,
    thumbnail_path: &Path,
) -> AppResult<()> {
    fs::create_dir_all(root)?;
    validate_managed_artifact_path(root, thumbnail_path, "썸네일")?;
    let parent = thumbnail_path
        .parent()
        .ok_or_else(|| AppError::new("source_path", "썸네일 파일의 상위 경로가 없습니다."))?;
    fs::create_dir_all(parent)?;
    let canonical_root = root.canonicalize()?;
    let canonical_parent = parent.canonicalize()?;
    if !canonical_parent.starts_with(&canonical_root) {
        return Err(AppError::new(
            "source_path",
            "관리되는 썸네일 폴더 밖에는 파일을 만들 수 없습니다.",
        ));
    }
    match fs::symlink_metadata(thumbnail_path) {
        Ok(metadata) if metadata.file_type().is_file() && !metadata.file_type().is_symlink() => {
            return Ok(())
        }
        Ok(_) => {
            return Err(AppError::new(
                "source_path",
                "썸네일 경로에 안전하지 않은 파일 항목이 있습니다.",
            ))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }

    let file_name = thumbnail_path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| AppError::new("source_path", "썸네일 파일 이름이 올바르지 않습니다."))?;
    let incoming = canonical_parent.join(format!(".{file_name}.incoming"));
    remove_managed_file_if_present(&incoming)?;
    let write_result = (|| -> AppResult<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&incoming)?;
        source_image
            .thumbnail(256, 256)
            .write_to(&mut file, ImageFormat::Png)?;
        file.sync_all()?;
        Ok(())
    })();
    if let Err(error) = write_result {
        let _ = remove_managed_file_if_present(&incoming);
        return Err(error);
    }
    if let Err(error) = fs::rename(&incoming, thumbnail_path) {
        let _ = remove_managed_file_if_present(&incoming);
        return Err(error.into());
    }
    let metadata = fs::symlink_metadata(thumbnail_path)?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        let _ = remove_managed_file_if_present(thumbnail_path);
        return Err(AppError::new(
            "source_path",
            "생성된 썸네일 파일을 안전하게 확인할 수 없습니다.",
        ));
    }
    Ok(())
}

fn validate_managed_artifact_path(root: &Path, path: &Path, label: &str) -> AppResult<()> {
    if path == root || !path.starts_with(root) {
        return Err(AppError::new(
            "source_path",
            format!("{label} 파일 경로가 관리 폴더 밖에 있습니다."),
        ));
    }
    Ok(())
}

fn entry_exists(path: &Path) -> AppResult<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

fn remove_managed_artifact_file(root: &Path, path: &Path) -> AppResult<()> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    if metadata.file_type().is_dir() {
        return Err(AppError::new(
            "source_path",
            "롤백할 파일 경로에 디렉터리가 있어 삭제하지 않았습니다.",
        ));
    }
    let canonical_root = root.canonicalize()?;
    let parent = path
        .parent()
        .ok_or_else(|| AppError::new("source_path", "롤백할 파일의 상위 경로가 없습니다."))?;
    let canonical_parent = parent.canonicalize()?;
    if !canonical_parent.starts_with(&canonical_root) {
        return Err(AppError::new(
            "source_path",
            "관리 폴더 밖의 파일은 롤백 정리하지 않습니다.",
        ));
    }
    fs::remove_file(path)?;
    Ok(())
}
fn decoded_has_alpha(bytes: &[u8], extension: &str, image: &DynamicImage) -> AppResult<bool> {
    if matches!(extension, "jpg" | "jpeg") {
        return Ok(false);
    }

    if extension != "gif" {
        return Ok(image.to_rgba8().pixels().any(|pixel| pixel[3] < 255));
    }

    let decoder = GifDecoder::new(Cursor::new(bytes))?;
    let frames = decoder.into_frames().collect_frames()?;
    Ok(frames
        .iter()
        .any(|frame| frame.buffer().pixels().any(|pixel| pixel[3] < 255)))
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

pub(crate) fn ensure_source_file_has_alpha(
    connection: &Connection,
    source_file_id: &str,
    path: &Path,
    extension: &str,
) -> AppResult<bool> {
    let current = connection.query_row(
        "SELECT has_alpha FROM source_files WHERE id = ?1",
        [source_file_id],
        |row| row.get::<_, Option<i64>>(0),
    )?;
    if let Some(current) = current {
        return Ok(current != 0);
    }

    let bytes = fs::read(path)?;
    let format = image_format_for_extension(extension)
        .ok_or_else(|| AppError::new("validation", "지원하지 않는 이미지 형식입니다."))?;
    let image = decode_import_image(&bytes, format)?;
    let has_alpha = decoded_has_alpha(&bytes, extension, &image)?;
    connection.execute(
        "UPDATE source_files SET has_alpha = ?1 WHERE id = ?2 AND has_alpha IS NULL",
        params![i64::from(has_alpha), source_file_id],
    )?;
    Ok(has_alpha)
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

pub(crate) fn source_thumbnail_path(paths: &AppPaths, source_file_id: &str) -> PathBuf {
    paths
        .source_file_thumbnails_dir
        .join(format!("{source_file_id}.png"))
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Cursor;
    use std::path::Path;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use image::codecs::gif::GifEncoder;
    use image::{Delay, DynamicImage, Frame, ImageBuffer, ImageFormat, Rgba};
    use rusqlite::{params, Connection};

    use super::{
        cleanup_source_file_crash_orphans_at, commit_prepared_source_file, decoded_has_alpha,
        ensure_original_bytes, prepare_source_file_from_bytes, sha256_hex, SourceFileImportOptions,
    };
    use crate::models::ImportImageFilePayload;
    use crate::paths::AppPaths;

    #[test]
    fn prepared_source_owns_validated_bytes_and_decoded_metadata() {
        let image =
            DynamicImage::ImageRgba8(ImageBuffer::from_pixel(3, 2, Rgba([10, 20, 30, 255])));
        let mut bytes = Vec::new();
        image
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();
        let expected_sha256 = sha256_hex(&bytes);
        let expected_len = bytes.len();
        let file = ImportImageFilePayload {
            original_filename: "candidate.png".to_string(),
            bytes,
        };

        let prepared = prepare_source_file_from_bytes(
            &file,
            SourceFileImportOptions {
                allow_gif: false,
                exact_dimensions: Some((3, 2)),
            },
        )
        .unwrap();
        drop(file);

        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let paths = AppPaths::prepare(std::env::temp_dir().join(format!(
            "pmtconcon-source-prepare-{}-{suffix}",
            std::process::id()
        )))
        .unwrap();
        let planned = prepared.planned_source_file(&paths);

        assert!(prepared.planned_source_file_id().starts_with("source_"));
        assert_eq!(prepared.original_filename(), "candidate.png");
        assert_eq!(prepared.bytes().len(), expected_len);
        assert_eq!(prepared.decoded_image().width(), 3);
        assert_eq!(prepared.decoded_image().height(), 2);
        assert_eq!(planned.id, prepared.planned_source_file_id());
        assert_eq!(planned.original_extension, "png");
        assert_eq!(planned.mime_type, "image/png");
        assert_eq!(planned.width, 3);
        assert_eq!(planned.height, 2);
        assert_eq!(planned.byte_size, expected_len as i64);
        assert_eq!(planned.sha256, expected_sha256);
        assert_eq!(planned.has_alpha, Some(false));
        assert_eq!(
            Path::new(&planned.original_path_in_library),
            prepared.planned_original_path(&paths)
        );
        assert_eq!(prepared.metadata.extension, "png");
        fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn commit_uses_planned_id_for_new_sha_and_existing_id_for_deduped_sha() {
        let image =
            DynamicImage::ImageRgba8(ImageBuffer::from_pixel(3, 2, Rgba([50, 60, 70, 255])));
        let mut bytes = Vec::new();
        image
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();
        let file = ImportImageFilePayload {
            original_filename: "candidate.png".to_string(),
            bytes,
        };
        let options = SourceFileImportOptions {
            allow_gif: false,
            exact_dimensions: None,
        };
        let first_prepared = prepare_source_file_from_bytes(&file, options).unwrap();
        let first_planned_id = first_prepared.planned_source_file_id().to_string();
        let second_prepared = prepare_source_file_from_bytes(&file, options).unwrap();
        assert_ne!(first_planned_id, second_prepared.planned_source_file_id());

        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let paths = AppPaths::prepare(std::env::temp_dir().join(format!(
            "pmtconcon-source-commit-{}-{suffix}",
            std::process::id()
        )))
        .unwrap();
        let mut connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE TABLE source_files (
                   id TEXT PRIMARY KEY,
                   original_filename TEXT NOT NULL,
                   original_path_in_library TEXT NOT NULL,
                   original_extension TEXT NOT NULL,
                   mime_type TEXT NOT NULL,
                   width INTEGER NOT NULL,
                   height INTEGER NOT NULL,
                   byte_size INTEGER NOT NULL,
                   sha256 TEXT NOT NULL UNIQUE,
                   has_alpha INTEGER,
                   is_animated INTEGER NOT NULL,
                   frame_count INTEGER,
                   original_loop_mode TEXT,
                   original_loop_count INTEGER,
                   imported_from_path TEXT,
                   created_at TEXT NOT NULL
                 );",
            )
            .unwrap();

        let transaction = connection.transaction().unwrap();
        let first = commit_prepared_source_file(&transaction, &paths, &first_prepared).unwrap();
        transaction.commit().unwrap();
        assert_eq!(first.id, first_planned_id);

        let transaction = connection.transaction().unwrap();
        let deduped = commit_prepared_source_file(&transaction, &paths, &second_prepared).unwrap();
        transaction.commit().unwrap();
        assert_eq!(deduped.id, first.id);
        assert_ne!(deduped.id, second_prepared.planned_source_file_id());
        assert_eq!(
            Path::new(&deduped.original_path_in_library),
            first_prepared.planned_original_path(&paths)
        );
        assert!(Path::new(&deduped.thumbnail_path).exists());

        fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn alpha_v1_distinguishes_opaque_and_transparent_static_pixels() {
        let opaque =
            DynamicImage::ImageRgba8(ImageBuffer::from_pixel(2, 2, Rgba([10, 20, 30, 255])));
        let transparent =
            DynamicImage::ImageRgba8(ImageBuffer::from_pixel(2, 2, Rgba([10, 20, 30, 200])));

        assert!(!decoded_has_alpha(&[], "png", &opaque).unwrap());
        assert!(decoded_has_alpha(&[], "png", &transparent).unwrap());
    }

    #[test]
    fn alpha_v1_scans_later_gif_frames() {
        let opaque = ImageBuffer::from_pixel(2, 2, Rgba([20, 40, 80, 255]));
        let transparent = ImageBuffer::from_pixel(2, 2, Rgba([20, 40, 80, 0]));
        let mut bytes = Vec::new();
        {
            let mut encoder = GifEncoder::new(Cursor::new(&mut bytes));
            encoder
                .encode_frames([
                    Frame::from_parts(opaque.clone(), 0, 0, Delay::from_numer_denom_ms(100, 1)),
                    Frame::from_parts(transparent, 0, 0, Delay::from_numer_denom_ms(100, 1)),
                ])
                .unwrap();
        }
        let first = DynamicImage::ImageRgba8(opaque);

        assert!(decoded_has_alpha(&bytes, "gif", &first).unwrap());
    }
    #[test]
    fn content_addressed_original_is_atomically_repaired_when_bytes_are_tampered() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let paths = AppPaths::prepare(std::env::temp_dir().join(format!(
            "pmtconcon-source-repair-{}-{suffix}",
            std::process::id()
        )))
        .unwrap();
        let path = paths.originals_dir.join("aa").join("original.bin");
        let expected = b"verified-original-bytes";
        ensure_original_bytes(&paths.originals_dir, &path, expected).unwrap();
        fs::write(&path, vec![b'x'; expected.len()]).unwrap();

        ensure_original_bytes(&paths.originals_dir, &path, expected).unwrap();

        assert_eq!(fs::read(&path).unwrap(), expected);
        let parent = path.parent().unwrap();
        assert!(!parent.join(".original.bin.incoming").exists());
        assert!(!parent.join(".original.bin.repair-backup").exists());
        fs::remove_dir_all(paths.root).unwrap();
    }
    fn cleanup_test_paths(label: &str) -> AppPaths {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        AppPaths::prepare(std::env::temp_dir().join(format!(
            "pmtconcon-source-cleanup-{label}-{}-{suffix}",
            std::process::id()
        )))
        .unwrap()
    }

    fn create_cleanup_source_files_table(connection: &Connection) {
        connection
            .execute_batch(
                "CREATE TABLE source_files (
                   id TEXT PRIMARY KEY,
                   original_path_in_library TEXT NOT NULL,
                   sha256 TEXT NOT NULL,
                   original_extension TEXT NOT NULL
                 );",
            )
            .unwrap();
    }

    #[test]
    fn cleanup_removes_only_old_unreferenced_owned_artifacts_and_is_idempotent() {
        let paths = cleanup_test_paths("old-unreferenced");
        let connection = Connection::open_in_memory().unwrap();
        create_cleanup_source_files_table(&connection);

        let hash = "a".repeat(64);
        let bucket = paths.originals_dir.join("aa");
        fs::create_dir_all(&bucket).unwrap();
        let original_name = format!("{hash}.png");
        let original_paths = [
            bucket.join(&original_name),
            bucket.join(format!(".{original_name}.incoming")),
            bucket.join(format!(".{original_name}.repair-backup")),
        ];
        for path in &original_paths {
            fs::write(path, b"orphan-original").unwrap();
        }

        let source_id = format!("source_{}_{}", "1".repeat(32), "2".repeat(8));
        let thumbnail_name = format!("{source_id}.png");
        let thumbnail_paths = [
            paths.source_file_thumbnails_dir.join(&thumbnail_name),
            paths
                .source_file_thumbnails_dir
                .join(format!(".{thumbnail_name}.incoming")),
            paths
                .source_file_thumbnails_dir
                .join(format!(".{thumbnail_name}.repair-backup")),
        ];
        for path in &thumbnail_paths {
            fs::write(path, b"orphan-thumbnail").unwrap();
        }

        let unknown_original = bucket.join("keep-me.txt");
        let unknown_thumbnail = paths.source_file_thumbnails_dir.join("keep-me.png");
        fs::write(&unknown_original, b"unknown").unwrap();
        fs::write(&unknown_thumbnail, b"unknown").unwrap();
        let outside_dir = paths.root.join("outside-managed-source-roots");
        fs::create_dir_all(&outside_dir).unwrap();
        let outside_file = outside_dir.join(&original_name);
        fs::write(&outside_file, b"outside").unwrap();

        let future = SystemTime::now()
            .checked_add(Duration::from_secs(26 * 60 * 60))
            .unwrap();
        cleanup_source_file_crash_orphans_at(&connection, &paths, future).unwrap();

        for path in original_paths.iter().chain(thumbnail_paths.iter()) {
            assert!(
                !path.exists(),
                "old unreferenced artifact survived: {path:?}"
            );
        }
        assert!(unknown_original.exists());
        assert!(unknown_thumbnail.exists());
        assert!(outside_file.exists());

        cleanup_source_file_crash_orphans_at(&connection, &paths, future).unwrap();
        assert!(unknown_original.exists());
        assert!(unknown_thumbnail.exists());
        assert!(outside_file.exists());
        fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn cleanup_preserves_old_referenced_and_new_unreferenced_artifacts() {
        let paths = cleanup_test_paths("preserve");
        let connection = Connection::open_in_memory().unwrap();
        create_cleanup_source_files_table(&connection);

        let referenced_hash = "b".repeat(64);
        let referenced_bucket = paths.originals_dir.join("bb");
        fs::create_dir_all(&referenced_bucket).unwrap();
        let referenced_original_name = format!("{referenced_hash}.png");
        let referenced_original_paths = [
            referenced_bucket.join(&referenced_original_name),
            referenced_bucket.join(format!(".{referenced_original_name}.incoming")),
            referenced_bucket.join(format!(".{referenced_original_name}.repair-backup")),
        ];
        for path in &referenced_original_paths {
            fs::write(path, b"referenced-original").unwrap();
        }

        let referenced_source_id = format!("source_{}_{}", "3".repeat(32), "4".repeat(8));
        let referenced_thumbnail_name = format!("{referenced_source_id}.png");
        let referenced_thumbnail_paths = [
            paths
                .source_file_thumbnails_dir
                .join(&referenced_thumbnail_name),
            paths
                .source_file_thumbnails_dir
                .join(format!(".{referenced_thumbnail_name}.incoming")),
            paths
                .source_file_thumbnails_dir
                .join(format!(".{referenced_thumbnail_name}.repair-backup")),
        ];
        for path in &referenced_thumbnail_paths {
            fs::write(path, b"referenced-thumbnail").unwrap();
        }
        connection
            .execute(
                "INSERT INTO source_files (
                   id, original_path_in_library, sha256, original_extension
                 ) VALUES (?1, ?2, ?3, ?4)",
                params![
                    referenced_source_id,
                    referenced_original_paths[0].to_string_lossy().to_string(),
                    referenced_hash,
                    "png"
                ],
            )
            .unwrap();

        let old_future = SystemTime::now()
            .checked_add(Duration::from_secs(26 * 60 * 60))
            .unwrap();
        cleanup_source_file_crash_orphans_at(&connection, &paths, old_future).unwrap();
        for path in referenced_original_paths
            .iter()
            .chain(referenced_thumbnail_paths.iter())
        {
            assert!(path.exists(), "referenced artifact was removed: {path:?}");
        }

        let new_hash = "c".repeat(64);
        let new_bucket = paths.originals_dir.join("cc");
        fs::create_dir_all(&new_bucket).unwrap();
        let new_original = new_bucket.join(format!("{new_hash}.png"));
        fs::write(&new_original, b"new-original").unwrap();
        let new_source_id = format!("source_{}_{}", "5".repeat(32), "6".repeat(8));
        let new_thumbnail = paths
            .source_file_thumbnails_dir
            .join(format!("{new_source_id}.png"));
        fs::write(&new_thumbnail, b"new-thumbnail").unwrap();

        let new_future = SystemTime::now()
            .checked_add(Duration::from_secs(23 * 60 * 60))
            .unwrap();
        cleanup_source_file_crash_orphans_at(&connection, &paths, new_future).unwrap();
        assert!(new_original.exists());
        assert!(new_thumbnail.exists());
        fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn cleanup_fails_closed_when_database_reference_is_uncertain() {
        let paths = cleanup_test_paths("uncertain-reference");
        let connection = Connection::open_in_memory().unwrap();
        create_cleanup_source_files_table(&connection);

        let orphan_hash = "d".repeat(64);
        let bucket = paths.originals_dir.join("dd");
        fs::create_dir_all(&bucket).unwrap();
        let orphan = bucket.join(format!("{orphan_hash}.png"));
        fs::write(&orphan, b"must-survive-failed-cleanup").unwrap();

        let uncertain_hash = "e".repeat(64);
        let uncertain_source_id = format!("source_{}_{}", "7".repeat(32), "8".repeat(8));
        let outside_path = paths
            .root
            .join("not-originals")
            .join(format!("{uncertain_hash}.png"));
        connection
            .execute(
                "INSERT INTO source_files (
                   id, original_path_in_library, sha256, original_extension
                 ) VALUES (?1, ?2, ?3, ?4)",
                params![
                    uncertain_source_id,
                    outside_path.to_string_lossy().to_string(),
                    uncertain_hash,
                    "png"
                ],
            )
            .unwrap();

        let future = SystemTime::now()
            .checked_add(Duration::from_secs(26 * 60 * 60))
            .unwrap();
        assert!(cleanup_source_file_crash_orphans_at(&connection, &paths, future).is_err());
        assert!(orphan.exists());
        fs::remove_dir_all(paths.root).unwrap();
    }
}
