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
           )
           AND NOT EXISTS (
             SELECT 1 FROM icons i
             WHERE i.thumbnail_override_source_file_id = s.id
           )
           AND NOT EXISTS (
             SELECT 1 FROM collections c
             WHERE c.cover_source_file_id = s.id
           )
           AND NOT EXISTS (
             SELECT 1 FROM ai_candidates candidate
             WHERE candidate.raw_source_file_id = s.id
           )
           AND NOT EXISTS (
             SELECT 1 FROM icon_ai_lineages lineage
             WHERE lineage.original_source_file_id = s.id
           )
           AND NOT EXISTS (
             SELECT 1 FROM icon_ai_versions version
             WHERE version.base_original_source_file_id = s.id
                OR version.effective_source_file_id = s.id
           )
           AND NOT EXISTS (
             SELECT 1 FROM processed_asset_variants variant
             WHERE variant.source_file_id = s.id
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
    let root = paths
        .root
        .canonicalize()
        .unwrap_or_else(|_| paths.root.clone());
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
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use rusqlite::{params, Connection};

    use crate::db::migrations;
    use crate::db::repositories::ai::import_local_ai_candidate;
    use crate::db::repositories::collections::create_collection;
    use crate::db::repositories::icons::delete_icons;
    use crate::db::repositories::imports::import_image_files;
    use crate::ids::create_id;
    use crate::models::{ImportAiCandidatePayload, ImportImageFilePayload};
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
        AppPaths::prepare(std::env::temp_dir().join(format!("pmtconcon-cleanup-{suffix}"))).unwrap()
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

    #[test]
    fn cleanup_preserves_ai_variant_and_soft_deleted_history_sources() {
        let mut connection = connection();
        let paths = temp_paths();
        let collection =
            create_collection(&mut connection, Some("정리 보존 테스트".to_string())).unwrap();
        let original_bytes = {
            let image = image::ImageBuffer::from_pixel(2, 2, image::Rgba([10, 20, 30, 255]));
            let mut cursor = std::io::Cursor::new(Vec::new());
            image::DynamicImage::ImageRgba8(image)
                .write_to(&mut cursor, image::ImageFormat::Png)
                .unwrap();
            cursor.into_inner()
        };
        let icon = import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![ImportImageFilePayload {
                original_filename: "original.png".to_string(),
                bytes: original_bytes,
            }],
        )
        .unwrap()
        .imported_icons
        .into_iter()
        .next()
        .unwrap();
        let candidate_bytes = {
            let image = image::ImageBuffer::from_pixel(2, 2, image::Rgba([30, 220, 80, 255]));
            let mut cursor = std::io::Cursor::new(Vec::new());
            image::DynamicImage::ImageRgba8(image)
                .write_to(&mut cursor, image::ImageFormat::Png)
                .unwrap();
            cursor.into_inner()
        };
        let review = import_local_ai_candidate(
            &mut connection,
            &paths,
            &collection.id,
            ImportAiCandidatePayload {
                icon_id: icon.id.clone(),
                service_surface: "gemini_web".to_string(),
                file: ImportImageFilePayload {
                    original_filename: "candidate.png".to_string(),
                    bytes: candidate_bytes,
                },
            },
        )
        .unwrap();
        let candidate = review.candidates.first().unwrap();
        let candidate_path = PathBuf::from(&candidate.source.original_image_url);

        let insert_source = |connection: &Connection,
                             paths: &AppPaths,
                             id: &str,
                             filename: &str,
                             sha256: &str,
                             bytes: &[u8]| {
            let path = paths.originals_dir.join(filename);
            std::fs::write(&path, bytes).unwrap();
            connection
                .execute(
                    "INSERT INTO source_files (
                       id, original_filename, original_path_in_library,
                       original_extension, mime_type, width, height, byte_size,
                       sha256, is_animated, has_alpha, created_at
                     ) VALUES (
                       ?1, ?2, ?3, 'png', 'image/png', 1, 1, ?4,
                       ?5, 0, 0, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                     )",
                    params![
                        id,
                        filename,
                        path.to_string_lossy().to_string(),
                        i64::try_from(bytes.len()).unwrap(),
                        sha256,
                    ],
                )
                .unwrap();
            path
        };
        let version_source_id = create_id("source");
        let version_path = insert_source(
            &connection,
            &paths,
            &version_source_id,
            "version-only.png",
            &"ab".repeat(32),
            b"version-only",
        );
        let (original_source_id, lineage_id, lineage_generation): (String, String, i64) =
            connection
                .query_row(
                    "SELECT source_file_id, original_lineage_id, original_lineage_generation
                     FROM icons WHERE id = ?1",
                    [&icon.id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .unwrap();
        let original_path: String = connection
            .query_row(
                "SELECT original_path_in_library FROM source_files WHERE id = ?1",
                [&original_source_id],
                |row| row.get(0),
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO icon_ai_versions (
                   id, icon_id, candidate_id, base_original_source_file_id,
                   base_original_lineage_id, base_original_lineage_generation,
                   parent_version_id, effective_source_file_id, input_stage, apply_kind,
                   provider_native_width, provider_native_height,
                   target_canvas_width, target_canvas_height,
                   normalization_recipe_json, normalization_recipe_hash,
                   canvas_kind, animation_kind, payload_input_signature, created_at
                 ) VALUES (
                   ?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7, 'base_source', 'active_source',
                   1, 1, 1, 1, '{}', 'normalization-hash',
                   'source', 'static', 'payload-signature',
                   strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 )",
                params![
                    create_id("ai_version"),
                    icon.id,
                    candidate.id,
                    original_source_id,
                    lineage_id,
                    lineage_generation,
                    version_source_id,
                ],
            )
            .unwrap();

        let variant_source_id = create_id("source");
        let variant_sha = "cd".repeat(32);
        let variant_path = insert_source(
            &connection,
            &paths,
            &variant_source_id,
            "variant-only.png",
            &variant_sha,
            b"variant-only",
        );
        let piece_id = icon.pieces[0].id.clone();
        let profile_id: String = connection
            .query_row(
                "SELECT id FROM export_profiles WHERE collection_id = ?1 LIMIT 1",
                [&collection.id],
                |row| row.get(0),
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO processed_asset_variants (
                   id, icon_id, piece_id, profile_id, source_file_id,
                   kind, preset, path, format, width, height, byte_size,
                   settings_json, source_hash, crop_hash, profile_hash,
                   settings_hash, output_sha256, created_at, is_active_for_export
                 ) VALUES (
                   ?1, ?2, ?3, ?4, ?5,
                   'baseline_export', 'baseline', ?6, 'png', 1, 1, ?7,
                   '{}', ?8, 'crop', 'profile', 'settings', NULL,
                   strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), 0
                 )",
                params![
                    create_id("variant"),
                    icon.id,
                    piece_id,
                    profile_id,
                    variant_source_id,
                    variant_path.to_string_lossy().to_string(),
                    std::fs::metadata(&variant_path).unwrap().len() as i64,
                    variant_sha,
                ],
            )
            .unwrap();
        let lineage_source_id = create_id("source");
        let lineage_path = insert_source(
            &connection,
            &paths,
            &lineage_source_id,
            "versionless-lineage.png",
            &"ef".repeat(32),
            b"versionless-lineage",
        );
        connection
            .execute(
                "INSERT INTO icon_ai_lineages (
                   icon_id, lineage_id, lineage_generation,
                   original_source_file_id, created_at
                 ) VALUES (
                   ?1, ?2, ?3, ?4,
                   strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 )",
                params![
                    icon.id,
                    create_id("lineage"),
                    lineage_generation + 10,
                    lineage_source_id,
                ],
            )
            .unwrap();
        delete_icons(&mut connection, &collection.id, vec![icon.id.clone()]).unwrap();

        let cleanup = cleanup_library(&connection, &paths, true).unwrap();
        assert_eq!(cleanup.orphaned_source_files, 0);
        assert!(Path::new(&original_path).is_file());
        assert!(candidate_path.is_file());
        assert!(version_path.is_file());
        assert!(lineage_path.is_file());
        assert!(variant_path.is_file());
        std::fs::remove_dir_all(paths.root).unwrap();
    }
}
