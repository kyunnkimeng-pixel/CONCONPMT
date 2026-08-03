use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior};

use crate::db::repositories::ai_managed_artifacts;
use crate::error::{AppError, AppResult};
use crate::paths::AppPaths;

const GRID_CLEANUP_DIRECTORY: &str = ".grid-cleanup";
const MAX_MANAGED_TREE_DEPTH: u8 = 6;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct AiGridPayloadCleanupReport {
    pub removed: usize,
    pub deferred: usize,
}

#[derive(Debug)]
struct GridArtifactSource {
    source_file_id: String,
    original_path: PathBuf,
    original_extension: String,
    sha256: String,
    thumbnail_path: PathBuf,
}

#[derive(Debug)]
struct MovedFile {
    source: PathBuf,
    quarantine: PathBuf,
}

pub(crate) fn managed_ai_grid_payload_bytes(
    connection: &Connection,
    paths: &AppPaths,
) -> AppResult<u64> {
    let mut statement = connection.prepare(
        "SELECT DISTINCT
           source.id,
           source.original_path_in_library,
           source.original_extension,
           source.sha256
         FROM ai_request_artifacts artifact
         JOIN ai_grid_payload_retention retention
           ON retention.request_id = artifact.request_id
         JOIN source_files source ON source.id = artifact.source_file_id
         WHERE retention.payload_deleted_at IS NULL",
    )?;
    let sources = statement
        .query_map([], |row| {
            Ok(GridArtifactSource {
                source_file_id: row.get(0)?,
                original_path: PathBuf::from(row.get::<_, String>(1)?),
                original_extension: row.get(2)?,
                sha256: row.get(3)?,
                thumbnail_path: PathBuf::new(),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut seen = HashSet::new();
    let mut total = 0_u64;
    for source in sources {
        validate_source_identity(paths, &source)?;
        total = add_existing_regular_file_bytes(
            &paths.originals_dir,
            &source.original_path,
            &mut seen,
            total,
        )?;
        let thumbnail = source_thumbnail_path(paths, &source.source_file_id);
        total = add_existing_regular_file_bytes(
            &paths.source_file_thumbnails_dir,
            &thumbnail,
            &mut seen,
            total,
        )?;
    }
    Ok(total)
}

pub(crate) fn cleanup_ai_grid_payloads_at(
    connection: &Connection,
    paths: &AppPaths,
    cutoff_modifier: &str,
) -> AppResult<AiGridPayloadCleanupReport> {
    let mut statement = connection.prepare(
        "SELECT retention.request_id,
                julianday(retention.expires_at)
                  <= julianday('now', ?1) AS is_expired
         FROM ai_grid_payload_retention retention
         JOIN ai_requests request ON request.id = retention.request_id
         WHERE retention.payload_deleted_at IS NULL
           AND (
             retention.cleanup_requested_at IS NOT NULL
             OR julianday(retention.expires_at) <= julianday('now', ?1)
             OR request.status IN ('completed', 'failed', 'cancelled', 'expired')
           )
         ORDER BY
           CASE
             WHEN request.status IN ('completed', 'failed', 'cancelled', 'expired')
               OR retention.cleanup_requested_at IS NOT NULL
             THEN 0 ELSE 1
           END,
           julianday(retention.created_at),
           retention.request_id",
    )?;
    let candidates = statement
        .query_map([cutoff_modifier], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? != 0))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    drop(statement);

    let mut report = AiGridPayloadCleanupReport::default();
    for (request_id, is_expired) in candidates {
        if validate_managed_id(&request_id, "ai_request_").is_err() {
            report.deferred += 1;
            continue;
        }
        if request_grid_payload_cleanup(connection, &request_id, is_expired).is_err() {
            report.deferred += 1;
            continue;
        }
        match finish_grid_payload_cleanup(connection, paths, &request_id) {
            Ok(true) => report.removed += 1,
            Ok(false) => {}
            Err(_) => report.deferred += 1,
        }
    }
    Ok(report)
}

fn request_grid_payload_cleanup(
    connection: &Connection,
    request_id: &str,
    is_expired: bool,
) -> AppResult<()> {
    let transaction = Transaction::new_unchecked(connection, TransactionBehavior::Immediate)?;
    if is_expired {
        transaction.execute(
            "UPDATE ai_requests
             SET status = 'expired',
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?1
               AND request_scope IN ('grid_edit', 'single_generate', 'grid_generate')
               AND status IN (
                 'draft', 'prepared', 'awaiting_result', 'layout_review_pending'
               )",
            [request_id],
        )?;
    }
    transaction.execute(
        "UPDATE ai_grid_payload_retention
         SET cleanup_requested_at = COALESCE(
               cleanup_requested_at,
               strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             ),
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE request_id = ?1
           AND payload_deleted_at IS NULL",
        [request_id],
    )?;
    transaction.commit()?;
    Ok(())
}

fn finish_grid_payload_cleanup(
    connection: &Connection,
    paths: &AppPaths,
    request_id: &str,
) -> AppResult<bool> {
    let already_deleted = connection
        .query_row(
            "SELECT payload_deleted_at IS NOT NULL
             FROM ai_grid_payload_retention
             WHERE request_id = ?1",
            [request_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("AI 그리드 전달 기록을 찾을 수 없습니다."))?
        != 0;
    if already_deleted {
        return Ok(false);
    }

    let quarantine_root = grid_quarantine_root(paths, request_id);
    if quarantine_root.exists() {
        remove_safe_managed_tree(paths, &quarantine_root)?;
    }

    let transaction = Transaction::new_unchecked(connection, TransactionBehavior::Immediate)?;
    let sources = load_grid_artifact_sources(&transaction, paths, request_id)?;
    transaction.execute(
        "DELETE FROM ai_request_artifacts WHERE request_id = ?1",
        [request_id],
    )?;

    let mut moved = Vec::new();
    let move_result = (|| -> AppResult<()> {
        for source in &sources {
            if !source_is_unreferenced(&transaction, &source.source_file_id)? {
                continue;
            }
            let source_quarantine =
                quarantine_source_directory(paths, request_id, &source.source_file_id)?;
            move_regular_file_if_present(
                &paths.originals_dir,
                &source.original_path,
                &source_quarantine.join(format!("original.{}", source.original_extension)),
                &mut moved,
            )?;
            move_regular_file_if_present(
                &paths.source_file_thumbnails_dir,
                &source.thumbnail_path,
                &source_quarantine.join("thumbnail.png"),
                &mut moved,
            )?;
        }
        Ok(())
    })();
    if let Err(error) = move_result {
        drop(transaction);
        restore_moved_files(&moved);
        return Err(error);
    }
    if let Err(error) = transaction.commit() {
        restore_moved_files(&moved);
        return Err(error.into());
    }

    if quarantine_root.exists() {
        remove_safe_managed_tree(paths, &quarantine_root)?;
    }
    let updated = connection.execute(
        "UPDATE ai_grid_payload_retention
         SET payload_deleted_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE request_id = ?1
           AND cleanup_requested_at IS NOT NULL
           AND payload_deleted_at IS NULL",
        [request_id],
    )?;
    if updated != 1 {
        return Err(AppError::new(
            "ai_grid_cleanup_state",
            "AI 그리드 임시 파일 정리 상태를 저장하지 못했습니다.",
        ));
    }
    Ok(true)
}

fn load_grid_artifact_sources(
    connection: &Connection,
    paths: &AppPaths,
    request_id: &str,
) -> AppResult<Vec<GridArtifactSource>> {
    let mut statement = connection.prepare(
        "SELECT DISTINCT
           source.id,
           source.original_path_in_library,
           source.original_extension,
           source.sha256
         FROM ai_request_artifacts artifact
         JOIN source_files source ON source.id = artifact.source_file_id
         WHERE artifact.request_id = ?1",
    )?;
    let mut sources = statement
        .query_map([request_id], |row| {
            Ok(GridArtifactSource {
                source_file_id: row.get(0)?,
                original_path: PathBuf::from(row.get::<_, String>(1)?),
                original_extension: row.get(2)?,
                sha256: row.get(3)?,
                thumbnail_path: PathBuf::new(),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    for source in &mut sources {
        source.thumbnail_path = source_thumbnail_path(paths, &source.source_file_id);
        validate_source_identity(paths, source)?;
    }
    Ok(sources)
}

fn source_is_unreferenced(connection: &Connection, source_file_id: &str) -> AppResult<bool> {
    let referenced = connection.query_row(
        "SELECT
           EXISTS(SELECT 1 FROM icons WHERE source_file_id = ?1)
           OR EXISTS(SELECT 1 FROM icons WHERE thumbnail_override_source_file_id = ?1)
           OR EXISTS(SELECT 1 FROM collections WHERE cover_source_file_id = ?1)
           OR EXISTS(SELECT 1 FROM ai_candidates WHERE raw_source_file_id = ?1)
           OR EXISTS(
             SELECT 1 FROM ai_request_artifacts WHERE source_file_id = ?1
           )
           OR EXISTS(
             SELECT 1 FROM icon_ai_lineages WHERE original_source_file_id = ?1
           )
           OR EXISTS(
             SELECT 1 FROM icon_ai_versions
             WHERE base_original_source_file_id = ?1
                OR effective_source_file_id = ?1
           )
           OR EXISTS(
             SELECT 1 FROM processed_asset_variants WHERE source_file_id = ?1
           )",
        [source_file_id],
        |row| row.get::<_, i64>(0),
    )?;
    Ok(referenced == 0)
}

fn validate_source_identity(paths: &AppPaths, source: &GridArtifactSource) -> AppResult<()> {
    if source.sha256.len() != 64
        || !source.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
        || source.sha256.bytes().any(|byte| byte.is_ascii_uppercase())
        || !matches!(
            source.original_extension.as_str(),
            "png" | "jpg" | "jpeg" | "gif"
        )
    {
        return Err(managed_path_error());
    }
    validate_managed_id(&source.source_file_id, "")?;
    let prefix = source.sha256.get(..2).ok_or_else(managed_path_error)?;
    let expected = paths
        .originals_dir
        .join(prefix)
        .join(format!("{}.{}", source.sha256, source.original_extension));
    if source.original_path != expected {
        return Err(managed_path_error());
    }
    Ok(())
}

fn add_existing_regular_file_bytes(
    root: &Path,
    path: &Path,
    seen: &mut HashSet<PathBuf>,
    total: u64,
) -> AppResult<u64> {
    let Some((canonical_path, byte_size)) = validate_existing_regular_file(root, path)? else {
        return Ok(total);
    };
    if !seen.insert(canonical_path) {
        return Ok(total);
    }
    total.checked_add(byte_size).ok_or_else(storage_size_error)
}

fn move_regular_file_if_present(
    root: &Path,
    source: &Path,
    quarantine: &Path,
    moved: &mut Vec<MovedFile>,
) -> AppResult<()> {
    if validate_existing_regular_file(root, source)?.is_none() {
        return Ok(());
    }
    if fs::symlink_metadata(quarantine).is_ok() {
        return Err(managed_path_error());
    }
    fs::rename(source, quarantine)?;
    moved.push(MovedFile {
        source: source.to_path_buf(),
        quarantine: quarantine.to_path_buf(),
    });
    Ok(())
}

fn restore_moved_files(moved: &[MovedFile]) {
    for item in moved.iter().rev() {
        if fs::symlink_metadata(&item.quarantine).is_ok()
            && fs::symlink_metadata(&item.source).is_err()
        {
            let _ = fs::rename(&item.quarantine, &item.source);
        }
    }
}

fn validate_existing_regular_file(root: &Path, path: &Path) -> AppResult<Option<(PathBuf, u64)>> {
    let root_metadata = fs::symlink_metadata(root)?;
    if !root_metadata.file_type().is_dir() || is_link_or_reparse_point(&root_metadata) {
        return Err(managed_path_error());
    }
    let canonical_root = root.canonicalize()?;
    let relative = path.strip_prefix(root).map_err(|_| managed_path_error())?;
    if relative
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(managed_path_error());
    }
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return Err(managed_path_error());
        };
        current.push(component);
        let metadata = match fs::symlink_metadata(&current) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        if is_link_or_reparse_point(&metadata) {
            return Err(managed_path_error());
        }
        if current == path {
            if !metadata.file_type().is_file() {
                return Err(managed_path_error());
            }
            let canonical = path.canonicalize()?;
            if !canonical.starts_with(&canonical_root) {
                return Err(managed_path_error());
            }
            return Ok(Some((canonical, metadata.len())));
        }
        if !metadata.file_type().is_dir() {
            return Err(managed_path_error());
        }
    }
    Ok(None)
}

fn quarantine_source_directory(
    paths: &AppPaths,
    request_id: &str,
    source_file_id: &str,
) -> AppResult<PathBuf> {
    validate_managed_id(request_id, "ai_request_")?;
    validate_managed_id(source_file_id, "")?;
    let target = grid_quarantine_root(paths, request_id).join(source_file_id);
    ai_managed_artifacts::prepare_owned_directory(&paths.root, &paths.ai_handoffs_dir, &target)
}

fn grid_quarantine_root(paths: &AppPaths, request_id: &str) -> PathBuf {
    paths
        .ai_handoffs_dir
        .join(GRID_CLEANUP_DIRECTORY)
        .join(request_id)
}

fn remove_safe_managed_tree(paths: &AppPaths, target: &Path) -> AppResult<()> {
    validate_managed_tree(target, 0)?;
    ai_managed_artifacts::remove_owned_directory_if_present(
        &paths.root,
        &paths.ai_handoffs_dir,
        target,
    )
}

fn validate_managed_tree(path: &Path, depth: u8) -> AppResult<()> {
    if depth > MAX_MANAGED_TREE_DEPTH {
        return Err(managed_path_error());
    }
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    if !metadata.file_type().is_dir() || is_link_or_reparse_point(&metadata) {
        return Err(managed_path_error());
    }
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let metadata = fs::symlink_metadata(entry.path())?;
        if is_link_or_reparse_point(&metadata) {
            return Err(managed_path_error());
        }
        if metadata.file_type().is_dir() {
            validate_managed_tree(&entry.path(), depth.saturating_add(1))?;
        } else if !metadata.file_type().is_file() {
            return Err(managed_path_error());
        }
    }
    Ok(())
}

fn validate_managed_id(value: &str, prefix: &str) -> AppResult<()> {
    if !value.is_empty()
        && value.starts_with(prefix)
        && value.len() <= 96
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        Ok(())
    } else {
        Err(managed_path_error())
    }
}

fn source_thumbnail_path(paths: &AppPaths, source_file_id: &str) -> PathBuf {
    paths
        .source_file_thumbnails_dir
        .join(format!("{source_file_id}.png"))
}

fn is_link_or_reparse_point(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return true;
        }
    }
    false
}

fn managed_path_error() -> AppError {
    AppError::new(
        "ai_grid_cleanup_path",
        "AI 그리드 임시 파일 경로가 관리 영역을 벗어나거나 링크를 포함합니다.",
    )
}

fn storage_size_error() -> AppError {
    AppError::new(
        "ai_handoff_storage_size",
        "AI 전달 임시 저장소 크기를 계산할 수 없습니다.",
    )
}
