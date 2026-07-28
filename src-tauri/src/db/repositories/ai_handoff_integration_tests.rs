use std::fs;
use std::io::Cursor;
use std::time::{SystemTime, UNIX_EPOCH};

use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};
use rusqlite::params;
use sha2::{Digest, Sha256};

use crate::db::connection::open_database;
use crate::db::repositories::ai::get_ai_review_state;
use crate::db::repositories::ai_handoff::{
    cleanup_ai_web_handoffs_after_days, commit_ai_web_handoff_result,
    delete_ai_web_handoff_payload, extend_ai_web_handoff_retention, get_ai_web_handoff_after_days,
    get_latest_ai_web_handoff_for_icon, prepare_ai_web_handoff, validate_ai_web_handoff_result,
    verified_ai_web_handoff_upload_path, PrepareAiWebHandoffPayload,
};
use crate::db::repositories::collections::create_collection;
use crate::db::repositories::imports::import_image_files;
use crate::models::ImportImageFilePayload;
use crate::paths::AppPaths;

fn png_bytes(width: u32, height: u32, color: [u8; 4]) -> Vec<u8> {
    let image = ImageBuffer::from_pixel(width, height, Rgba(color));
    let mut cursor = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image)
        .write_to(&mut cursor, ImageFormat::Png)
        .unwrap();
    cursor.into_inner()
}

fn animated_gif_bytes() -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut encoder = gif::Encoder::new(&mut bytes, 8, 8, &[]).unwrap();
        encoder.set_repeat(gif::Repeat::Infinite).unwrap();
        for color in [[20_u8, 40, 220, 255], [230_u8, 80, 30, 255]] {
            let mut pixels = Vec::with_capacity(8 * 8 * 4);
            for _ in 0..(8 * 8) {
                pixels.extend_from_slice(&color);
            }
            let mut frame = gif::Frame::from_rgba_speed(8, 8, &mut pixels, 10);
            frame.delay = 5;
            encoder.write_frame(&frame).unwrap();
        }
    }
    bytes
}

fn temp_paths(label: &str) -> AppPaths {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    AppPaths::prepare(std::env::temp_dir().join(format!(
        "pmtconcon-ai-handoff-{label}-{}-{suffix}",
        std::process::id()
    )))
    .unwrap()
}

fn single_payload(icon_id: &str) -> PrepareAiWebHandoffPayload {
    PrepareAiWebHandoffPayload {
        icon_id: Some(icon_id.to_string()),
        icon_ids: Vec::new(),
        layout_mode: Some("single".to_string()),
        operation: Some("edit".to_string()),
        service_surface: "gemini_web".to_string(),
        user_prompt: "표정을 더 밝게".to_string(),
    }
}

#[test]
fn handoff_commit_attaches_one_inactive_candidate_without_changing_current_source() {
    let paths = temp_paths("commit");
    let (collection_id, icon_id, original_source_id, original_source_path, original_sha256) = {
        let mut connection = open_database(&paths.database_path).unwrap();
        let collection =
            create_collection(&mut connection, Some("웹 전달 통합".to_string())).unwrap();
        let icon = import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![ImportImageFilePayload {
                original_filename: "original.png".to_string(),
                bytes: png_bytes(8, 8, [20, 40, 220, 0]),
            }],
        )
        .unwrap()
        .imported_icons
        .into_iter()
        .next()
        .unwrap();
        let (path, sha256) = connection
            .query_row(
                "SELECT original_path_in_library, sha256
                 FROM source_files
                 WHERE id = ?1",
                [&icon.source_file_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .unwrap();
        (collection.id, icon.id, icon.source_file_id, path, sha256)
    };

    let request_id = {
        let mut connection = open_database(&paths.database_path).unwrap();
        let before = get_ai_review_state(&connection, &collection_id, &icon_id).unwrap();
        let before_revision = before.visual_source.activation_revision;
        let session = prepare_ai_web_handoff(
            &mut connection,
            &paths,
            &collection_id,
            single_payload(&icon_id),
        )
        .unwrap();
        assert_eq!(session.kind, "static_icon_sheet");
        assert_eq!(session.layout_mode, "single");
        assert_eq!(session.expected_width, 8);
        assert_eq!(session.expected_height, 8);
        assert!(session.expected_has_alpha);
        assert_eq!(session.native_drag_supported, cfg!(windows));
        assert!(session.final_prompt.contains("exactly 8×8px"));
        let upload_path = std::path::PathBuf::from(&session.upload_preview_path);
        assert!(upload_path.is_file());
        let package_dir = upload_path.parent().unwrap().to_path_buf();
        assert!(package_dir.join("manifest.json").is_file());
        assert!(package_dir.join("prompt.txt").is_file());

        let result = ImportImageFilePayload {
            original_filename: "download.bin".to_string(),
            bytes: png_bytes(8, 8, [230, 80, 30, 128]),
        };
        let inspection =
            validate_ai_web_handoff_result(&mut connection, &paths, &session.request_id, &result)
                .unwrap();
        assert!(inspection.accepted);
        let signature = inspection.validation_signature.unwrap();
        let committed = commit_ai_web_handoff_result(
            &mut connection,
            &paths,
            &session.request_id,
            result,
            &signature,
        )
        .unwrap();
        assert!(committed.accepted);
        let review = committed.review_state.unwrap();
        assert_eq!(review.visual_source.original_source.id, original_source_id);
        assert_eq!(
            review.visual_source.effective_render_source.id,
            original_source_id
        );
        assert_eq!(review.visual_source.active_candidate_id, None);
        assert_eq!(review.visual_source.active_version_id, None);
        assert_eq!(review.visual_source.activation_revision, before_revision);
        assert_eq!(
            review
                .candidates
                .iter()
                .filter(|candidate| candidate.request_id == session.request_id)
                .count(),
            1
        );
        assert!(!package_dir.exists());

        let persisted = connection
            .query_row(
                "SELECT
                   request.status,
                   package.candidate_id IS NOT NULL,
                   package.payload_deleted_at IS NOT NULL,
                   (SELECT COUNT(*) FROM ai_candidates WHERE request_id = request.id)
                 FROM ai_requests request
                 JOIN ai_web_handoff_packages package
                   ON package.request_id = request.id
                 WHERE request.id = ?1",
                [&session.request_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(persisted, ("completed".to_string(), 1, 1, 1));
        session.request_id
    };

    {
        let connection = open_database(&paths.database_path).unwrap();
        let review = get_ai_review_state(&connection, &collection_id, &icon_id).unwrap();
        assert_eq!(
            review.visual_source.effective_render_source.id,
            original_source_id
        );
        assert_eq!(
            review
                .candidates
                .iter()
                .filter(|candidate| candidate.request_id == request_id)
                .count(),
            1
        );
        assert!(std::path::Path::new(&original_source_path).is_file());
        let persisted_original = fs::read(&original_source_path).unwrap();
        assert_eq!(
            format!("{:x}", Sha256::digest(&persisted_original)),
            original_sha256
        );
    }

    fs::remove_dir_all(&paths.root).unwrap();
}

#[test]
fn proportional_web_result_is_preserved_raw_and_registered_for_local_normalization() {
    let paths = temp_paths("proportional-result");
    let mut connection = open_database(&paths.database_path).unwrap();
    let collection = create_collection(&mut connection, Some("비율 정규화".to_string())).unwrap();
    let icon = import_image_files(
        &mut connection,
        &paths,
        &collection.id,
        vec![ImportImageFilePayload {
            original_filename: "original.png".to_string(),
            bytes: png_bytes(8, 8, [20, 40, 220, 0]),
        }],
    )
    .unwrap()
    .imported_icons
    .into_iter()
    .next()
    .unwrap();
    let session = prepare_ai_web_handoff(
        &mut connection,
        &paths,
        &collection.id,
        single_payload(&icon.id),
    )
    .unwrap();
    let result = ImportImageFilePayload {
        original_filename: "gemini-1024.png".to_string(),
        bytes: png_bytes(16, 16, [230, 80, 30, 128]),
    };

    let inspection =
        validate_ai_web_handoff_result(&mut connection, &paths, &session.request_id, &result)
            .unwrap();
    assert!(inspection.accepted);
    assert_eq!(
        (inspection.actual_width, inspection.actual_height),
        (Some(16), Some(16))
    );
    assert!(inspection.issues.iter().any(|issue| {
        issue.code == "ai_handoff_result_size_normalization" && issue.severity == "warning"
    }));
    let committed = commit_ai_web_handoff_result(
        &mut connection,
        &paths,
        &session.request_id,
        result,
        inspection.validation_signature.as_deref().unwrap(),
    )
    .unwrap();
    assert!(committed.accepted);
    let candidate = connection
        .query_row(
            "SELECT candidate.width, candidate.height, source.width, source.height
             FROM ai_candidates candidate
             JOIN source_files source ON source.id = candidate.raw_source_file_id
             WHERE candidate.request_id = ?1",
            [&session.request_id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(candidate, (16, 16, 16, 16));

    drop(connection);
    fs::remove_dir_all(&paths.root).unwrap();
}

#[test]
fn drag_upload_path_is_fixed_to_verified_current_package_file() {
    let paths = temp_paths("drag-path");
    let mut connection = open_database(&paths.database_path).unwrap();
    let collection = create_collection(&mut connection, Some("드래그 경로".to_string())).unwrap();
    let icon = import_image_files(
        &mut connection,
        &paths,
        &collection.id,
        vec![ImportImageFilePayload {
            original_filename: "original.png".to_string(),
            bytes: png_bytes(8, 8, [20, 40, 220, 0]),
        }],
    )
    .unwrap()
    .imported_icons
    .into_iter()
    .next()
    .unwrap();
    let session = prepare_ai_web_handoff(
        &mut connection,
        &paths,
        &collection.id,
        single_payload(&icon.id),
    )
    .unwrap();

    let upload_path =
        verified_ai_web_handoff_upload_path(&mut connection, &paths, &session.request_id).unwrap();
    assert_eq!(
        upload_path.file_name().and_then(|name| name.to_str()),
        Some("upload.png")
    );
    assert_eq!(
        upload_path,
        std::path::PathBuf::from(&session.upload_preview_path)
    );
    assert!(upload_path.starts_with(paths.ai_handoffs_dir.canonicalize().unwrap()));

    let traversal =
        verified_ai_web_handoff_upload_path(&mut connection, &paths, "../upload.png").unwrap_err();
    assert_eq!(traversal.code, "ai_handoff_request_id");

    fs::write(&upload_path, b"tampered").unwrap();
    let tampered =
        verified_ai_web_handoff_upload_path(&mut connection, &paths, &session.request_id)
            .unwrap_err();
    assert_eq!(tampered.code, "ai_handoff_payload_corrupt");

    drop(connection);
    fs::remove_dir_all(&paths.root).unwrap();
}
#[test]
fn handoff_retention_is_seven_days_and_can_only_be_extended_once() {
    let paths = temp_paths("retention");
    let mut connection = open_database(&paths.database_path).unwrap();
    let collection = create_collection(&mut connection, Some("웹 전달 보존".to_string())).unwrap();
    let icon = import_image_files(
        &mut connection,
        &paths,
        &collection.id,
        vec![ImportImageFilePayload {
            original_filename: "original.png".to_string(),
            bytes: png_bytes(8, 8, [20, 40, 220, 255]),
        }],
    )
    .unwrap()
    .imported_icons
    .into_iter()
    .next()
    .unwrap();
    let session = prepare_ai_web_handoff(
        &mut connection,
        &paths,
        &collection.id,
        single_payload(&icon.id),
    )
    .unwrap();
    let initial_days: f64 = connection
        .query_row(
            "SELECT julianday(expires_at) - julianday(created_at)
             FROM ai_web_handoff_packages
             WHERE request_id = ?1",
            [&session.request_id],
            |row| row.get(0),
        )
        .unwrap();
    assert!((initial_days - 7.0).abs() < 0.000_001);

    let extended =
        extend_ai_web_handoff_retention(&mut connection, &paths, &session.request_id).unwrap();
    assert!(!extended.can_extend);
    let extended_days: f64 = connection
        .query_row(
            "SELECT julianday(expires_at) - julianday(created_at)
             FROM ai_web_handoff_packages
             WHERE request_id = ?1",
            [&session.request_id],
            |row| row.get(0),
        )
        .unwrap();
    assert!((extended_days - 37.0).abs() < 0.000_001);
    let second =
        extend_ai_web_handoff_retention(&mut connection, &paths, &session.request_id).unwrap_err();
    assert_eq!(second.code, "ai_handoff_retention_unavailable");

    drop(connection);
    fs::remove_dir_all(&paths.root).unwrap();
}

#[test]
fn startup_cleanup_expires_due_package_and_removes_only_its_transient_files() {
    let paths = temp_paths("cleanup");
    let mut connection = open_database(&paths.database_path).unwrap();
    let collection = create_collection(&mut connection, Some("웹 전달 만료".to_string())).unwrap();
    let icon = import_image_files(
        &mut connection,
        &paths,
        &collection.id,
        vec![ImportImageFilePayload {
            original_filename: "original.png".to_string(),
            bytes: png_bytes(8, 8, [20, 40, 220, 255]),
        }],
    )
    .unwrap()
    .imported_icons
    .into_iter()
    .next()
    .unwrap();
    let original_path: String = connection
        .query_row(
            "SELECT original_path_in_library
             FROM source_files
             WHERE id = ?1",
            [&icon.source_file_id],
            |row| row.get(0),
        )
        .unwrap();
    let session = prepare_ai_web_handoff(
        &mut connection,
        &paths,
        &collection.id,
        single_payload(&icon.id),
    )
    .unwrap();
    let package_dir = std::path::PathBuf::from(&session.upload_preview_path)
        .parent()
        .unwrap()
        .to_path_buf();
    let cleanup_report = cleanup_ai_web_handoffs_after_days(&connection, &paths, 8).unwrap();
    assert_eq!(cleanup_report.removed, 1);
    assert_eq!(cleanup_report.deferred, 0);
    assert!(!package_dir.exists());
    assert!(std::path::Path::new(&original_path).is_file());
    let status = connection
        .query_row(
            "SELECT
               request.status,
               package.payload_deleted_at IS NOT NULL
             FROM ai_requests request
             JOIN ai_web_handoff_packages package
               ON package.request_id = request.id
             WHERE request.id = ?1",
            [&session.request_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .unwrap();
    assert_eq!(status, ("expired".to_string(), 1));

    drop(connection);
    fs::remove_dir_all(&paths.root).unwrap();
}

#[test]
fn latest_restore_discards_stale_handoff_before_web_work_and_preserves_original() {
    let paths = temp_paths("stale-restore");
    let mut connection = open_database(&paths.database_path).unwrap();
    let collection =
        create_collection(&mut connection, Some("오래된 전달 복원".to_string())).unwrap();
    let icon = import_image_files(
        &mut connection,
        &paths,
        &collection.id,
        vec![ImportImageFilePayload {
            original_filename: "original.png".to_string(),
            bytes: png_bytes(8, 8, [20, 40, 220, 0]),
        }],
    )
    .unwrap()
    .imported_icons
    .into_iter()
    .next()
    .unwrap();
    let original_path: String = connection
        .query_row(
            "SELECT original_path_in_library FROM source_files WHERE id = ?1",
            [&icon.source_file_id],
            |row| row.get(0),
        )
        .unwrap();
    let session = prepare_ai_web_handoff(
        &mut connection,
        &paths,
        &collection.id,
        single_payload(&icon.id),
    )
    .unwrap();
    let package_dir = std::path::PathBuf::from(&session.upload_preview_path)
        .parent()
        .unwrap()
        .to_path_buf();
    connection
        .execute(
            "UPDATE icon_ai_state
             SET revision = revision + 1,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE icon_id = ?1",
            [&icon.id],
        )
        .unwrap();

    let restored =
        get_latest_ai_web_handoff_for_icon(&mut connection, &paths, &collection.id, &icon.id)
            .unwrap();
    assert!(restored.is_none());
    assert!(!package_dir.exists());
    assert!(std::path::Path::new(&original_path).is_file());
    let state = connection
        .query_row(
            "SELECT request.status,
                    package.cleanup_requested_at IS NOT NULL,
                    package.payload_deleted_at IS NOT NULL
             FROM ai_requests request
             JOIN ai_web_handoff_packages package ON package.request_id = request.id
             WHERE request.id = ?1",
            [&session.request_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(state, ("cancelled".to_string(), 1, 1));

    drop(connection);
    fs::remove_dir_all(&paths.root).unwrap();
}

#[test]
fn direct_access_to_expired_handoff_marks_and_removes_only_transient_payload() {
    let paths = temp_paths("expired-access");
    let mut connection = open_database(&paths.database_path).unwrap();
    let collection =
        create_collection(&mut connection, Some("만료 직접 접근".to_string())).unwrap();
    let icon = import_image_files(
        &mut connection,
        &paths,
        &collection.id,
        vec![ImportImageFilePayload {
            original_filename: "original.png".to_string(),
            bytes: png_bytes(8, 8, [20, 40, 220, 0]),
        }],
    )
    .unwrap()
    .imported_icons
    .into_iter()
    .next()
    .unwrap();
    let original_path: String = connection
        .query_row(
            "SELECT original_path_in_library FROM source_files WHERE id = ?1",
            [&icon.source_file_id],
            |row| row.get(0),
        )
        .unwrap();
    let session = prepare_ai_web_handoff(
        &mut connection,
        &paths,
        &collection.id,
        single_payload(&icon.id),
    )
    .unwrap();
    let package_dir = std::path::PathBuf::from(&session.upload_preview_path)
        .parent()
        .unwrap()
        .to_path_buf();

    let error =
        get_ai_web_handoff_after_days(&mut connection, &paths, &session.request_id, 8).unwrap_err();
    assert_eq!(error.code, "ai_handoff_expired");
    assert!(!package_dir.exists());
    assert!(std::path::Path::new(&original_path).is_file());
    let state = connection
        .query_row(
            "SELECT request.status,
                    package.cleanup_requested_at IS NOT NULL,
                    package.payload_deleted_at IS NOT NULL
             FROM ai_requests request
             JOIN ai_web_handoff_packages package ON package.request_id = request.id
             WHERE request.id = ?1",
            [&session.request_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(state, ("expired".to_string(), 1, 1));

    drop(connection);
    fs::remove_dir_all(&paths.root).unwrap();
}

#[test]
fn deleting_handoff_payload_twice_is_idempotent_and_preserves_original() {
    let paths = temp_paths("delete-idempotent");
    let mut connection = open_database(&paths.database_path).unwrap();
    let collection =
        create_collection(&mut connection, Some("전달 닫기 재호출".to_string())).unwrap();
    let icon = import_image_files(
        &mut connection,
        &paths,
        &collection.id,
        vec![ImportImageFilePayload {
            original_filename: "original.png".to_string(),
            bytes: png_bytes(8, 8, [20, 40, 220, 0]),
        }],
    )
    .unwrap()
    .imported_icons
    .into_iter()
    .next()
    .unwrap();
    let original_path: String = connection
        .query_row(
            "SELECT original_path_in_library FROM source_files WHERE id = ?1",
            [&icon.source_file_id],
            |row| row.get(0),
        )
        .unwrap();
    let first_session = prepare_ai_web_handoff(
        &mut connection,
        &paths,
        &collection.id,
        single_payload(&icon.id),
    )
    .unwrap();
    let first_package_dir = std::path::PathBuf::from(&first_session.upload_preview_path)
        .parent()
        .unwrap()
        .to_path_buf();
    let session = prepare_ai_web_handoff(
        &mut connection,
        &paths,
        &collection.id,
        single_payload(&icon.id),
    )
    .unwrap();
    let package_dir = std::path::PathBuf::from(&session.upload_preview_path)
        .parent()
        .unwrap()
        .to_path_buf();
    assert!(!first_package_dir.exists());
    let first_state = connection
        .query_row(
            "SELECT request.status,
                    package.cleanup_requested_at IS NOT NULL,
                    package.payload_deleted_at IS NOT NULL
             FROM ai_requests request
             JOIN ai_web_handoff_packages package ON package.request_id = request.id
             WHERE request.id = ?1",
            [&first_session.request_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(first_state, ("cancelled".to_string(), 1, 1));

    let restored =
        get_latest_ai_web_handoff_for_icon(&mut connection, &paths, &collection.id, &icon.id)
            .unwrap()
            .unwrap();
    assert_eq!(restored.request_id, session.request_id);

    let first_delete =
        delete_ai_web_handoff_payload(&mut connection, &paths, &session.request_id).unwrap();
    assert!(first_delete.session_closed);
    assert!(first_delete.payload_deleted);
    assert!(!first_delete.cleanup_deferred);
    let second_delete =
        delete_ai_web_handoff_payload(&mut connection, &paths, &session.request_id).unwrap();
    assert_eq!(second_delete, first_delete);
    assert!(
        get_latest_ai_web_handoff_for_icon(&mut connection, &paths, &collection.id, &icon.id)
            .unwrap()
            .is_none()
    );
    assert!(!package_dir.exists());
    assert!(std::path::Path::new(&original_path).is_file());
    let state = connection
        .query_row(
            "SELECT request.status,
                    package.cleanup_requested_at IS NOT NULL,
                    package.payload_deleted_at IS NOT NULL
             FROM ai_requests request
             JOIN ai_web_handoff_packages package ON package.request_id = request.id
             WHERE request.id = ?1",
            [&session.request_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(state, ("cancelled".to_string(), 1, 1));

    drop(connection);
    fs::remove_dir_all(&paths.root).unwrap();
}

#[test]
fn gif_and_grid_prepare_fail_with_typed_errors_before_request_creation() {
    let paths = temp_paths("unsupported");
    let mut connection = open_database(&paths.database_path).unwrap();
    let collection = create_collection(&mut connection, Some("웹 전달 범위".to_string())).unwrap();
    let icons = import_image_files(
        &mut connection,
        &paths,
        &collection.id,
        vec![
            ImportImageFilePayload {
                original_filename: "static.png".to_string(),
                bytes: png_bytes(8, 8, [20, 40, 220, 255]),
            },
            ImportImageFilePayload {
                original_filename: "animated.gif".to_string(),
                bytes: animated_gif_bytes(),
            },
        ],
    )
    .unwrap()
    .imported_icons;
    let static_icon = icons
        .iter()
        .find(|icon| icon.display_name.contains("static"))
        .unwrap();
    let gif_icon = icons
        .iter()
        .find(|icon| icon.display_name.contains("animated"))
        .unwrap();

    let gif_error = prepare_ai_web_handoff(
        &mut connection,
        &paths,
        &collection.id,
        single_payload(&gif_icon.id),
    )
    .unwrap_err();
    assert_eq!(gif_error.code, "ai_handoff_gif_unsupported");

    let mut grid_payload = single_payload(&static_icon.id);
    grid_payload.icon_ids = vec![static_icon.id.clone(), gif_icon.id.clone()];
    grid_payload.layout_mode = Some("grid".to_string());
    let grid_error =
        prepare_ai_web_handoff(&mut connection, &paths, &collection.id, grid_payload).unwrap_err();
    assert_eq!(grid_error.code, "ai_handoff_grid_unsupported");

    let request_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM ai_web_handoff_packages",
            params![],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(request_count, 0);

    drop(connection);
    fs::remove_dir_all(&paths.root).unwrap();
}
