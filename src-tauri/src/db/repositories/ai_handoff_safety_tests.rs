use std::fs;
use std::io::Cursor;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};
use rusqlite::{params, Connection};

use crate::db::connection::open_database;
use crate::db::repositories::ai_handoff::{
    cleanup_ai_web_handoffs, commit_ai_web_handoff_result, prepare_ai_web_handoff,
    validate_ai_web_handoff_result, PrepareAiWebHandoffPayload,
};
use crate::db::repositories::collections::create_collection;
use crate::db::repositories::imports::import_image_files;
use crate::models::ImportImageFilePayload;
use crate::paths::AppPaths;

fn png_bytes(color: [u8; 4]) -> Vec<u8> {
    let image = ImageBuffer::from_pixel(8, 8, Rgba(color));
    let mut cursor = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image)
        .write_to(&mut cursor, ImageFormat::Png)
        .unwrap();
    cursor.into_inner()
}

fn temp_paths(label: &str) -> AppPaths {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    AppPaths::prepare(std::env::temp_dir().join(format!(
        "pmtconcon-ai-handoff-safety-{label}-{}-{suffix}",
        std::process::id()
    )))
    .unwrap()
}

fn fixture(label: &str) -> (AppPaths, Connection, String, String, String) {
    let paths = temp_paths(label);
    let mut connection = open_database(&paths.database_path).unwrap();
    let collection =
        create_collection(&mut connection, Some(format!("웹 전달 안전성 {label}"))).unwrap();
    let icon = import_image_files(
        &mut connection,
        &paths,
        &collection.id,
        vec![ImportImageFilePayload {
            original_filename: "original.png".to_string(),
            bytes: png_bytes([20, 40, 220, 0]),
        }],
    )
    .unwrap()
    .imported_icons
    .into_iter()
    .next()
    .unwrap();
    (
        paths,
        connection,
        collection.id,
        icon.id,
        icon.source_file_id,
    )
}

fn import_additional_icon(
    connection: &mut Connection,
    paths: &AppPaths,
    collection_id: &str,
    label: &str,
) -> String {
    import_image_files(
        connection,
        paths,
        collection_id,
        vec![ImportImageFilePayload {
            original_filename: format!("{label}.png"),
            bytes: png_bytes([20, 40, 220, 0]),
        }],
    )
    .unwrap()
    .imported_icons
    .into_iter()
    .next()
    .unwrap()
    .id
}

fn payload(icon_id: &str) -> PrepareAiWebHandoffPayload {
    PrepareAiWebHandoffPayload {
        icon_id: Some(icon_id.to_string()),
        icon_ids: Vec::new(),
        layout_mode: Some("single".to_string()),
        operation: Some("edit".to_string()),
        service_surface: "gemini_web".to_string(),
        user_prompt: "색을 조금 더 선명하게".to_string(),
    }
}

fn insert_candidate_for_request(
    connection: &Connection,
    candidate_id: &str,
    request_id: &str,
    source_file_id: &str,
) {
    connection
        .execute(
            "INSERT INTO ai_candidates (
               id, request_id, candidate_index, raw_source_file_id,
               raw_source_sha256, output_format, width, height, is_animated,
               has_alpha, provider_capabilities_snapshot_json, created_at
             )
             SELECT
               ?1, ?2, 0, source.id, source.sha256, source.original_extension,
               source.width, source.height, source.is_animated, source.has_alpha,
               request.capability_snapshot_json,
               strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             FROM source_files source
             JOIN ai_requests request ON request.id = ?2
             WHERE source.id = ?3",
            params![candidate_id, request_id, source_file_id],
        )
        .unwrap();
}

fn count_regular_files(root: &Path) -> usize {
    let Ok(entries) = fs::read_dir(root) else {
        return 0;
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| {
            let Ok(metadata) = fs::symlink_metadata(entry.path()) else {
                return 0;
            };
            if metadata.file_type().is_file() {
                1
            } else if metadata.file_type().is_dir() && !metadata.file_type().is_symlink() {
                count_regular_files(&entry.path())
            } else {
                0
            }
        })
        .sum()
}

#[test]
fn migration_triggers_reject_bad_dates_cross_request_candidates_and_rewrites() {
    let (paths, mut connection, collection_id, icon_id, source_file_id) =
        fixture("migration-guards");
    let first =
        prepare_ai_web_handoff(&mut connection, &paths, &collection_id, payload(&icon_id)).unwrap();
    let second_icon_id =
        import_additional_icon(&mut connection, &paths, &collection_id, "second-guard");
    let second = prepare_ai_web_handoff(
        &mut connection,
        &paths,
        &collection_id,
        payload(&second_icon_id),
    )
    .unwrap();

    let malformed_date = connection.execute(
        "UPDATE ai_web_handoff_packages
         SET expires_at = 'not-a-date',
             extended_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE request_id = ?1",
        [&first.request_id],
    );
    assert!(malformed_date.is_err());
    let wrong_extension = connection.execute(
        "UPDATE ai_web_handoff_packages
         SET expires_at = strftime(
               '%Y-%m-%dT%H:%M:%fZ',
               expires_at,
               '+29 days'
             ),
             extended_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE request_id = ?1",
        [&first.request_id],
    );
    assert!(wrong_extension.is_err());

    insert_candidate_for_request(
        &connection,
        "ai_candidate_cross_request",
        &second.request_id,
        &source_file_id,
    );
    let source_sha256: String = connection
        .query_row(
            "SELECT sha256 FROM source_files WHERE id = ?1",
            [&source_file_id],
            |row| row.get(0),
        )
        .unwrap();
    let cross_request = connection.execute(
        "UPDATE ai_web_handoff_packages
         SET candidate_id = 'ai_candidate_cross_request',
             result_sha256 = ?1,
             result_received_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE request_id = ?2",
        params![source_sha256, first.request_id],
    );
    assert!(cross_request.is_err());

    insert_candidate_for_request(
        &connection,
        "ai_candidate_same_request",
        &first.request_id,
        &source_file_id,
    );
    let wrong_hash = connection.execute(
        "UPDATE ai_web_handoff_packages
         SET candidate_id = 'ai_candidate_same_request',
             result_sha256 = ?1,
             result_received_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE request_id = ?2",
        params!["f".repeat(64), first.request_id],
    );
    assert!(wrong_hash.is_err());
    let direct_result_write = connection.execute(
        "UPDATE ai_web_handoff_packages
         SET candidate_id = 'ai_candidate_same_request',
             result_sha256 = ?1,
             result_received_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE request_id = ?2",
        params![source_sha256, first.request_id],
    );
    assert!(direct_result_write.is_err());

    let transaction = connection.transaction().unwrap();
    transaction
        .execute(
            "UPDATE ai_requests
             SET status = 'completed',
                 completed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?1",
            [&first.request_id],
        )
        .unwrap();
    transaction
        .execute(
            "UPDATE ai_web_handoff_packages
             SET candidate_id = 'ai_candidate_same_request',
                 result_sha256 = ?1,
                 result_received_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                 cleanup_requested_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE request_id = ?2",
            params![source_sha256, first.request_id],
        )
        .unwrap();
    transaction.commit().unwrap();

    let second_result_write = connection.execute(
        "UPDATE ai_web_handoff_packages
         SET candidate_id = 'ai_candidate_same_request',
             result_sha256 = ?1,
             result_received_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE request_id = ?2",
        params![source_sha256, first.request_id],
    );
    assert!(second_result_write.is_err());

    let active_cleanup_intent = connection.execute(
        "UPDATE ai_web_handoff_packages
         SET cleanup_requested_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE request_id = ?1",
        [&second.request_id],
    );
    assert!(active_cleanup_intent.is_err());
    connection
        .execute(
            "UPDATE ai_requests
             SET status = 'cancelled',
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?1",
            [&second.request_id],
        )
        .unwrap();
    connection
        .execute(
            "UPDATE ai_web_handoff_packages
             SET cleanup_requested_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE request_id = ?1",
            [&second.request_id],
        )
        .unwrap();
    assert!(connection
        .execute(
            "UPDATE ai_web_handoff_packages
             SET cleanup_requested_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE request_id = ?1",
            [&second.request_id],
        )
        .is_err());
    connection
        .execute(
            "UPDATE ai_web_handoff_packages
             SET payload_deleted_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE request_id = ?1",
            [&second.request_id],
        )
        .unwrap();
    assert!(connection
        .execute(
            "UPDATE ai_web_handoff_packages
             SET payload_deleted_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE request_id = ?1",
            [&second.request_id],
        )
        .is_err());

    drop(connection);
    fs::remove_dir_all(&paths.root).unwrap();
}

#[test]
fn commit_database_failure_rolls_back_candidate_and_new_source_artifacts() {
    let (paths, mut connection, collection_id, icon_id, _) = fixture("commit-rollback");
    let session =
        prepare_ai_web_handoff(&mut connection, &paths, &collection_id, payload(&icon_id)).unwrap();
    let result = ImportImageFilePayload {
        original_filename: "result.png".to_string(),
        bytes: png_bytes([230, 80, 30, 128]),
    };
    let inspection =
        validate_ai_web_handoff_result(&mut connection, &paths, &session.request_id, &result)
            .unwrap();
    let signature = inspection.validation_signature.unwrap();
    let source_rows_before: i64 = connection
        .query_row("SELECT COUNT(*) FROM source_files", [], |row| row.get(0))
        .unwrap();
    let original_files_before = count_regular_files(&paths.originals_dir);
    let thumbnail_files_before = count_regular_files(&paths.source_file_thumbnails_dir);
    connection
        .execute_batch(
            "CREATE TRIGGER test_fail_ai_handoff_commit
             BEFORE UPDATE OF candidate_id ON ai_web_handoff_packages
             WHEN NEW.candidate_id IS NOT NULL
             BEGIN
               SELECT RAISE(ABORT, 'test handoff commit failure');
             END;",
        )
        .unwrap();

    assert!(commit_ai_web_handoff_result(
        &mut connection,
        &paths,
        &session.request_id,
        result,
        &signature,
    )
    .is_err());
    let persisted = connection
        .query_row(
            "SELECT
               (SELECT COUNT(*) FROM source_files),
               (SELECT COUNT(*) FROM ai_candidates WHERE request_id = ?1),
               request.status,
               package.candidate_id,
               package.cleanup_requested_at
             FROM ai_requests request
             JOIN ai_web_handoff_packages package
               ON package.request_id = request.id
             WHERE request.id = ?1",
            [&session.request_id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(
        persisted,
        (
            source_rows_before,
            0,
            "awaiting_result".to_string(),
            None,
            None,
        )
    );
    assert_eq!(
        count_regular_files(&paths.originals_dir),
        original_files_before
    );
    assert_eq!(
        count_regular_files(&paths.source_file_thumbnails_dir),
        thumbnail_files_before
    );

    drop(connection);
    fs::remove_dir_all(&paths.root).unwrap();
}

#[test]
fn startup_cleanup_recovers_both_intent_before_delete_and_delete_before_marker() {
    let (paths, mut connection, collection_id, icon_id, original_source_id) =
        fixture("crash-recovery");
    let first =
        prepare_ai_web_handoff(&mut connection, &paths, &collection_id, payload(&icon_id)).unwrap();
    let second_icon_id =
        import_additional_icon(&mut connection, &paths, &collection_id, "second-crash");
    let second = prepare_ai_web_handoff(
        &mut connection,
        &paths,
        &collection_id,
        payload(&second_icon_id),
    )
    .unwrap();

    let first_dir = std::path::PathBuf::from(&first.upload_preview_path)
        .parent()
        .unwrap()
        .to_path_buf();
    let second_dir = std::path::PathBuf::from(&second.upload_preview_path)
        .parent()
        .unwrap()
        .to_path_buf();

    for request_id in [&first.request_id, &second.request_id] {
        let transaction = connection.transaction().unwrap();
        transaction
            .execute(
                "UPDATE ai_requests
                 SET status = 'cancelled',
                     updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE id = ?1",
                [request_id],
            )
            .unwrap();
        transaction
            .execute(
                "UPDATE ai_web_handoff_packages
                 SET cleanup_requested_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                     updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE request_id = ?1",
                [request_id],
            )
            .unwrap();
        transaction.commit().unwrap();
    }
    fs::remove_dir_all(&second_dir).unwrap();
    assert!(first_dir.is_dir());
    assert!(!second_dir.exists());

    let cleanup_report = cleanup_ai_web_handoffs(&connection, &paths).unwrap();
    assert_eq!(cleanup_report.removed, 2);
    assert_eq!(cleanup_report.deferred, 0);
    assert!(!first_dir.exists());
    assert!(!second_dir.exists());
    let marked: i64 = connection
        .query_row(
            "SELECT COUNT(*)
             FROM ai_web_handoff_packages
             WHERE request_id IN (?1, ?2)
               AND cleanup_requested_at IS NOT NULL
               AND payload_deleted_at IS NOT NULL",
            params![first.request_id, second.request_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(marked, 2);
    let original_path: String = connection
        .query_row(
            "SELECT original_path_in_library
             FROM source_files
             WHERE id = ?1",
            [&original_source_id],
            |row| row.get(0),
        )
        .unwrap();
    assert!(Path::new(&original_path).is_file());

    drop(connection);
    fs::remove_dir_all(&paths.root).unwrap();
}
