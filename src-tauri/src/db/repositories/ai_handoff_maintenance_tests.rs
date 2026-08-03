use std::fs;
use std::io::Cursor;
use std::time::{SystemTime, UNIX_EPOCH};

use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};
use rusqlite::{params, Connection, TransactionBehavior};

use crate::db::connection::open_database;
use crate::db::repositories::ai_handoff::{
    delete_ai_web_handoff_payload, get_ai_web_handoff, get_ai_web_handoff_storage_status,
    list_recent_ai_web_handoffs, prepare_ai_web_handoff, prepare_ai_web_handoff_with_test_quota,
    run_ai_web_handoff_maintenance, PrepareAiWebHandoffPayload,
};
use crate::db::repositories::collections::create_collection;
use crate::db::repositories::imports::import_image_files;
use crate::models::ImportImageFilePayload;
use crate::paths::AppPaths;

fn png_bytes(color: [u8; 4]) -> Vec<u8> {
    let image = ImageBuffer::from_pixel(16, 16, Rgba(color));
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
        "pmtconcon-ai-handoff-maintenance-{label}-{}-{suffix}",
        std::process::id()
    )))
    .unwrap()
}

fn fixture(label: &str) -> (AppPaths, Connection, String, Vec<String>) {
    let paths = temp_paths(label);
    let mut connection = open_database(&paths.database_path).unwrap();
    let collection = create_collection(
        &mut connection,
        Some("AI web handoff maintenance".to_string()),
    )
    .unwrap();
    let icons = import_image_files(
        &mut connection,
        &paths,
        &collection.id,
        vec![
            ImportImageFilePayload {
                original_filename: "first.png".to_string(),
                bytes: png_bytes([20, 40, 220, 255]),
            },
            ImportImageFilePayload {
                original_filename: "second.png".to_string(),
                bytes: png_bytes([230, 80, 30, 255]),
            },
        ],
    )
    .unwrap()
    .imported_icons
    .into_iter()
    .map(|icon| icon.id)
    .collect();
    (paths, connection, collection.id, icons)
}

fn payload(icon_id: &str) -> PrepareAiWebHandoffPayload {
    PrepareAiWebHandoffPayload {
        icon_id: Some(icon_id.to_string()),
        icon_ids: Vec::new(),
        layout_mode: Some("single".to_string()),
        operation: Some("edit".to_string()),
        service_surface: "gemini_web".to_string(),
        user_prompt: "make it brighter".to_string(),
    }
}

#[test]
fn quota_rejects_new_package_without_deleting_an_active_payload() {
    let (paths, mut connection, collection_id, icon_ids) = fixture("quota");
    let first = prepare_ai_web_handoff(
        &mut connection,
        &paths,
        &collection_id,
        payload(&icon_ids[0]),
    )
    .unwrap();
    let before = get_ai_web_handoff_storage_status(&connection, &paths).unwrap();
    assert!(before.used_bytes > 0);
    assert_eq!(before.live_payload_count, 1);

    let error = prepare_ai_web_handoff_with_test_quota(
        &mut connection,
        &paths,
        &collection_id,
        payload(&icon_ids[1]),
        before.used_bytes,
    )
    .unwrap_err();
    assert_eq!(error.code, "ai_handoff_payload_quota_exceeded");

    let restored = get_ai_web_handoff(&mut connection, &paths, &first.request_id).unwrap();
    assert_eq!(restored.request_id, first.request_id);
    let counts = connection
        .query_row(
            "SELECT
               COUNT(*),
               SUM(CASE WHEN request.status = 'awaiting_result' THEN 1 ELSE 0 END)
             FROM ai_web_handoff_packages package
             JOIN ai_requests request ON request.id = package.request_id",
            [],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )
        .unwrap();
    assert_eq!(counts, (1, 1));

    drop(connection);
    fs::remove_dir_all(paths.root).unwrap();
}

#[test]
fn recent_history_keeps_deleted_rows_and_applies_a_bounded_limit() {
    let (paths, mut connection, collection_id, icon_ids) = fixture("history");
    let first = prepare_ai_web_handoff(
        &mut connection,
        &paths,
        &collection_id,
        payload(&icon_ids[0]),
    )
    .unwrap();
    let deleted =
        delete_ai_web_handoff_payload(&mut connection, &paths, &first.request_id).unwrap();
    assert!(deleted.payload_deleted);

    let second = prepare_ai_web_handoff(
        &mut connection,
        &paths,
        &collection_id,
        payload(&icon_ids[1]),
    )
    .unwrap();
    let history = list_recent_ai_web_handoffs(&connection, None).unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].request_id, second.request_id);
    assert_eq!(history[0].payload_state, "available");
    let first_history = history
        .iter()
        .find(|item| item.request_id == first.request_id)
        .unwrap();
    assert_eq!(first_history.request_status, "cancelled");
    assert_eq!(first_history.payload_state, "deleted");

    let limited = list_recent_ai_web_handoffs(&connection, Some(1)).unwrap();
    assert_eq!(limited.len(), 1);
    assert_eq!(limited[0].request_id, second.request_id);
    let invalid_limit = list_recent_ai_web_handoffs(&connection, Some(0)).unwrap_err();
    assert_eq!(invalid_limit.code, "ai_handoff_history_limit");

    let storage = get_ai_web_handoff_storage_status(&connection, &paths).unwrap();
    assert_eq!(storage.retained_history_count, 2);
    assert_eq!(storage.live_payload_count, 1);

    drop(connection);
    fs::remove_dir_all(paths.root).unwrap();
}

#[test]
fn one_shot_maintenance_finishes_crash_cleanup_and_preserves_history() {
    let (paths, mut connection, collection_id, icon_ids) = fixture("cleanup");
    let session = prepare_ai_web_handoff(
        &mut connection,
        &paths,
        &collection_id,
        payload(&icon_ids[0]),
    )
    .unwrap();
    let package_dir = paths.ai_handoffs_dir.join(&session.request_id);
    assert!(package_dir.is_dir());

    {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .unwrap();
        transaction
            .execute(
                "UPDATE ai_requests
                 SET status = 'cancelled',
                     updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE id = ?1
                   AND status = 'awaiting_result'",
                [&session.request_id],
            )
            .unwrap();
        transaction
            .execute(
                "UPDATE ai_web_handoff_packages
                 SET cleanup_requested_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                     updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE request_id = ?1
                   AND cleanup_requested_at IS NULL",
                [&session.request_id],
            )
            .unwrap();
        transaction.commit().unwrap();
    }

    let report = run_ai_web_handoff_maintenance(&connection, &paths).unwrap();
    assert_eq!(report.removed_count, 1);
    assert_eq!(report.deferred_count, 0);
    assert_eq!(report.storage.used_bytes, 0);
    assert_eq!(report.storage.retained_history_count, 1);
    assert_eq!(report.storage.live_payload_count, 0);
    assert_eq!(report.storage.cleanup_pending_count, 0);
    assert!(!package_dir.exists());

    let state = connection
        .query_row(
            "SELECT request.status, package.payload_deleted_at IS NOT NULL
             FROM ai_requests request
             JOIN ai_web_handoff_packages package ON package.request_id = request.id
             WHERE request.id = ?1",
            params![session.request_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .unwrap();
    assert_eq!(state, ("cancelled".to_string(), 1));
    let history = list_recent_ai_web_handoffs(&connection, None).unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].payload_state, "deleted");

    let second_report = run_ai_web_handoff_maintenance(&connection, &paths).unwrap();
    assert_eq!(second_report.removed_count, 0);
    assert_eq!(second_report.storage.retained_history_count, 1);

    drop(connection);
    fs::remove_dir_all(paths.root).unwrap();
}
