use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::error::{AppError, AppResult};
use crate::models::LibraryCleanupResultDto;
use crate::paths::AppPaths;

#[derive(Debug)]
struct OrphanSourceFile {
    id: String,
    original_path_in_library: String,
}

pub fn cleanup_library(
    connection: &Connection,
    paths: &AppPaths,
    apply: bool,
) -> AppResult<LibraryCleanupResultDto> {
    let orphan_sources = orphan_source_files(connection)?;
    let mut result = LibraryCleanupResultDto {
        orphaned_source_files: 0,
        removed_original_files: 0,
        removed_thumbnail_files: 0,
        removed_temp_files: 0,
    };

    for source in orphan_sources {
        let original_path = PathBuf::from(&source.original_path_in_library);
        let thumbnail_path = paths
            .source_file_thumbnails_dir
            .join(format!("{}.png", source.id));

        let original_exists = is_managed_existing_file(paths, &original_path);
        let thumbnail_exists = is_managed_existing_file(paths, &thumbnail_path);
        if original_exists || thumbnail_exists {
            result.orphaned_source_files += 1;
        }

        if original_exists {
            result.removed_original_files += 1;
            if apply {
                fs::remove_file(&original_path)?;
            }
        }

        if thumbnail_exists {
            result.removed_thumbnail_files += 1;
            if apply {
                fs::remove_file(&thumbnail_path)?;
            }
        }
    }

    let temp_files = temp_files(paths)?;
    result.removed_temp_files = temp_files.len() as i64;
    if apply {
        for path in temp_files {
            fs::remove_file(path)?;
        }
    }

    Ok(result)
}

fn orphan_source_files(connection: &Connection) -> AppResult<Vec<OrphanSourceFile>> {
    let mut statement = connection.prepare(
        "SELECT s.id, s.original_path_in_library
         FROM source_files s
         WHERE NOT EXISTS (
             SELECT 1 FROM icons i
             WHERE i.source_file_id = s.id
               AND i.deleted_at IS NULL
           )
           AND NOT EXISTS (
             SELECT 1 FROM icons i
             WHERE i.thumbnail_override_source_file_id = s.id
               AND i.deleted_at IS NULL
           )
           AND NOT EXISTS (
             SELECT 1 FROM collections c
             WHERE c.cover_source_file_id = s.id
               AND c.deleted_at IS NULL
           )",
    )?;

    let sources = statement
        .query_map([], |row| {
            Ok(OrphanSourceFile {
                id: row.get("id")?,
                original_path_in_library: row.get("original_path_in_library")?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(sources)
}

fn temp_files(paths: &AppPaths) -> AppResult<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_files(paths, &paths.temp_import_dir, &mut files)?;
    collect_files(paths, &paths.temp_export_dir, &mut files)?;
    Ok(files)
}

fn collect_files(paths: &AppPaths, directory: &Path, files: &mut Vec<PathBuf>) -> AppResult<()> {
    if !directory.exists() {
        return Ok(());
    }
    ensure_within_root(paths, directory)?;

    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files(paths, &path, files)?;
        } else if is_managed_existing_file(paths, &path) {
            files.push(path);
        }
    }

    Ok(())
}

fn is_managed_existing_file(paths: &AppPaths, path: &Path) -> bool {
    path.is_file() && ensure_within_root(paths, path).is_ok()
}

fn ensure_within_root(paths: &AppPaths, path: &Path) -> AppResult<()> {
    let root = paths.root.canonicalize().unwrap_or_else(|_| paths.root.clone());
    let target = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

    if target.starts_with(&root) {
        Ok(())
    } else {
        Err(AppError::new(
            "validation",
            "앱 라이브러리 밖의 파일은 정리하지 않습니다.",
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use rusqlite::{params, Connection};

    use crate::db::migrations;
    use crate::paths::AppPaths;

    use super::cleanup_library;

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
        AppPaths::prepare(std::env::temp_dir().join(format!("pmtconcon-cleanup-{suffix}")))
            .unwrap()
    }

    #[test]
    fn cleanup_library_only_deletes_unreferenced_managed_files_when_applied() {
        let connection = connection();
        let paths = temp_paths();
        let orphan_path = paths.originals_dir.join("orphan.png");
        std::fs::write(&orphan_path, b"orphan").unwrap();

        connection
            .execute(
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
                   created_at
                 )
                 VALUES (
                   'source_orphan',
                   'orphan.png',
                   ?1,
                   'png',
                   'image/png',
                   1,
                   1,
                   6,
                   'orphanhash',
                   strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 )",
                params![orphan_path.to_string_lossy().to_string()],
            )
            .unwrap();

        let preview = cleanup_library(&connection, &paths, false).unwrap();
        assert_eq!(preview.orphaned_source_files, 1);
        assert!(orphan_path.exists());

        let applied = cleanup_library(&connection, &paths, true).unwrap();
        assert_eq!(applied.removed_original_files, 1);
        assert!(!orphan_path.exists());

        std::fs::remove_dir_all(paths.root).unwrap();
    }
}
