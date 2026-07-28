use std::fs;
use std::io::Cursor;
use std::time::{SystemTime, UNIX_EPOCH};

use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};
use rusqlite::Connection;

use crate::db::connection::open_database;
use crate::db::repositories::ai_grid::{
    cancel_ai_grid_request, get_ai_grid_workspace, mark_ai_grid_awaiting_result,
    prepare_ai_generation, prepare_ai_generation_with_references, prepare_ai_grid_edit,
    record_ai_grid_output_artifact, PrepareAiGenerationReferences, PrepareAiGenerationRequest,
};
use crate::db::repositories::ai_handoff::{
    cleanup_ai_web_handoffs, get_ai_web_handoff_storage_status, list_recent_ai_web_handoffs,
    reserve_ai_transfer_storage_with_test_quota, run_ai_web_handoff_maintenance,
};
use crate::db::repositories::collections::create_collection;
use crate::db::repositories::imports::import_image_files;
use crate::models::ImportImageFilePayload;
use crate::paths::AppPaths;
use crate::sheet::composer::{default_ai_generation_layout, default_ai_grid_layout};

fn png_bytes(size: u32, color: [u8; 4]) -> Vec<u8> {
    let image = ImageBuffer::from_pixel(size, size, Rgba(color));
    let mut cursor = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image)
        .write_to(&mut cursor, ImageFormat::Png)
        .unwrap();
    cursor.into_inner()
}

fn fixture(label: &str) -> (AppPaths, Connection, String, Vec<String>) {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let paths = AppPaths::prepare(std::env::temp_dir().join(format!(
        "pmtconcon-ai-grid-retention-{label}-{}-{suffix}",
        std::process::id()
    )))
    .unwrap();
    let mut connection = open_database(&paths.database_path).unwrap();
    let collection =
        create_collection(&mut connection, Some("AI grid retention".to_string())).unwrap();
    let icon_ids = import_image_files(
        &mut connection,
        &paths,
        &collection.id,
        vec![
            ImportImageFilePayload {
                original_filename: "first.png".to_string(),
                bytes: png_bytes(32, [30, 80, 210, 255]),
            },
            ImportImageFilePayload {
                original_filename: "second.png".to_string(),
                bytes: png_bytes(32, [230, 70, 20, 255]),
            },
        ],
    )
    .unwrap()
    .imported_icons
    .into_iter()
    .map(|icon| icon.id)
    .collect();
    (paths, connection, collection.id, icon_ids)
}

fn prepare_grid(
    connection: &mut Connection,
    paths: &AppPaths,
    collection_id: &str,
    icon_ids: &[String],
) -> String {
    prepare_ai_grid_edit(
        connection,
        paths,
        collection_id,
        icon_ids.to_vec(),
        default_ai_grid_layout(icon_ids.len(), 1024).unwrap(),
        None,
    )
    .unwrap()
    .request_id
}

#[test]
fn grid_input_and_result_are_in_total_storage_and_recent_history() {
    let (paths, mut connection, collection_id, icon_ids) = fixture("total-history");
    let request_id = prepare_grid(&mut connection, &paths, &collection_id, &icon_ids);
    let input_status = get_ai_web_handoff_storage_status(&connection, &paths).unwrap();
    assert!(input_status.used_bytes > 0);
    assert_eq!(input_status.retained_history_count, 1);
    assert_eq!(input_status.live_payload_count, 1);

    let workspace = get_ai_grid_workspace(&connection, &request_id).unwrap();
    let manifest = workspace.input_artifact.unwrap().manifest_json;
    mark_ai_grid_awaiting_result(&connection, &request_id).unwrap();
    record_ai_grid_output_artifact(
        &mut connection,
        &paths,
        &request_id,
        ImportImageFilePayload {
            original_filename: "grid-result.png".to_string(),
            bytes: png_bytes(1024, [120, 180, 40, 255]),
        },
        &manifest,
    )
    .unwrap();

    let output_status = get_ai_web_handoff_storage_status(&connection, &paths).unwrap();
    assert!(output_status.used_bytes > input_status.used_bytes);
    let history = list_recent_ai_web_handoffs(&connection, None).unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].request_id, request_id);
    assert_eq!(history[0].request_scope, "grid_edit");
    assert_eq!(history[0].handoff_kind, "ai_grid_sheet");
    assert!(history[0].has_result);

    drop(connection);
    fs::remove_dir_all(paths.root).unwrap();
}

#[test]
fn recent_history_exposes_generation_reference_sheets_but_keeps_source_free_generation_closed() {
    let (paths, mut connection, collection_id, icon_ids) = fixture("generation-history");
    let referenced_single = prepare_ai_generation_with_references(
        &mut connection,
        &paths,
        &collection_id,
        PrepareAiGenerationRequest {
            target_names: vec!["기쁨".to_string()],
            layout: default_ai_generation_layout(1, 1_024).unwrap(),
            payload_input_signature: "single-reference-history".to_string(),
            retry_of_request_id: None,
        },
        PrepareAiGenerationReferences {
            selected_icon_ids: vec![icon_ids[0].clone()],
            external_files: Vec::new(),
        },
    )
    .unwrap();
    let referenced_grid = prepare_ai_generation_with_references(
        &mut connection,
        &paths,
        &collection_id,
        PrepareAiGenerationRequest {
            target_names: vec!["기쁨".to_string(), "놀람".to_string()],
            layout: default_ai_grid_layout(2, 1_024).unwrap(),
            payload_input_signature: "grid-reference-history".to_string(),
            retry_of_request_id: None,
        },
        PrepareAiGenerationReferences {
            selected_icon_ids: vec![icon_ids[1].clone()],
            external_files: Vec::new(),
        },
    )
    .unwrap();
    let source_free = prepare_ai_generation(
        &mut connection,
        &collection_id,
        PrepareAiGenerationRequest {
            target_names: vec!["웃음".to_string(), "울음".to_string()],
            layout: default_ai_grid_layout(2, 1_024).unwrap(),
            payload_input_signature: "source-free-history".to_string(),
            retry_of_request_id: None,
        },
    )
    .unwrap();

    let history = list_recent_ai_web_handoffs(&connection, None).unwrap();
    for request_id in [&referenced_single.request_id, &referenced_grid.request_id] {
        let item = history
            .iter()
            .find(|item| &item.request_id == request_id)
            .unwrap();
        assert_eq!(item.payload_state, "available");
    }
    let source_free_item = history
        .iter()
        .find(|item| item.request_id == source_free.request_id)
        .unwrap();
    assert_eq!(source_free_item.payload_state, "closed");

    drop(connection);
    fs::remove_dir_all(paths.root).unwrap();
}
#[test]
fn quota_cleanup_evicts_terminal_grid_before_rejecting_and_preserves_active_grid() {
    let (paths, mut connection, collection_id, icon_ids) = fixture("quota-priority");
    let terminal_id = prepare_grid(&mut connection, &paths, &collection_id, &icon_ids);
    cancel_ai_grid_request(&connection, &terminal_id).unwrap();

    {
        let _reservation =
            reserve_ai_transfer_storage_with_test_quota(&connection, &paths, 1, 1).unwrap();
    }
    let terminal_status = get_ai_web_handoff_storage_status(&connection, &paths).unwrap();
    assert_eq!(terminal_status.used_bytes, 0);
    let terminal_history = list_recent_ai_web_handoffs(&connection, None).unwrap();
    assert_eq!(terminal_history[0].payload_state, "deleted");

    let active_id = prepare_grid(&mut connection, &paths, &collection_id, &icon_ids);
    let active_status = get_ai_web_handoff_storage_status(&connection, &paths).unwrap();
    assert!(active_status.used_bytes > 0);
    assert_eq!(active_status.live_payload_count, 1);
    let error = match reserve_ai_transfer_storage_with_test_quota(
        &connection,
        &paths,
        1,
        active_status.used_bytes,
    ) {
        Ok(_) => panic!("active grid payload must not be evicted to satisfy quota"),
        Err(error) => error,
    };
    assert_eq!(error.code, "ai_handoff_payload_quota_exceeded");
    assert!(get_ai_grid_workspace(&connection, &active_id)
        .unwrap()
        .input_artifact
        .is_some());

    drop(connection);
    fs::remove_dir_all(paths.root).unwrap();
}

#[test]
fn out_of_root_grid_artifact_is_never_deleted_and_stays_cleanup_pending() {
    let (paths, mut connection, collection_id, icon_ids) = fixture("containment");
    let request_id = prepare_grid(&mut connection, &paths, &collection_id, &icon_ids);
    let (source_file_id, original_path): (String, String) = connection
        .query_row(
            "SELECT source.id, source.original_path_in_library
             FROM ai_request_artifacts artifact
             JOIN source_files source ON source.id = artifact.source_file_id
             WHERE artifact.request_id = ?1 AND artifact.role = 'input_sheet'",
            [&request_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    let outside = paths.root.join("must-not-delete.png");
    fs::write(&outside, b"outside").unwrap();
    connection
        .execute(
            "UPDATE source_files SET original_path_in_library = ?1 WHERE id = ?2",
            [outside.to_string_lossy().as_ref(), source_file_id.as_str()],
        )
        .unwrap();
    cancel_ai_grid_request(&connection, &request_id).unwrap();

    let report = cleanup_ai_web_handoffs(&connection, &paths).unwrap();
    assert_eq!(report.removed, 0);
    assert_eq!(report.deferred, 1);
    assert!(outside.is_file());
    let state: (i64, i64, i64) = connection
        .query_row(
            "SELECT
               retention.cleanup_requested_at IS NOT NULL,
               retention.payload_deleted_at IS NOT NULL,
               EXISTS(
                 SELECT 1 FROM ai_request_artifacts artifact
                 WHERE artifact.request_id = retention.request_id
               )
             FROM ai_grid_payload_retention retention
             WHERE retention.request_id = ?1",
            [&request_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(state, (1, 0, 1));

    connection
        .execute(
            "UPDATE source_files SET original_path_in_library = ?1 WHERE id = ?2",
            [original_path.as_str(), source_file_id.as_str()],
        )
        .unwrap();
    fs::remove_file(outside).unwrap();
    let retry = run_ai_web_handoff_maintenance(&connection, &paths).unwrap();
    assert_eq!(retry.removed_count, 1);
    assert_eq!(retry.deferred_count, 0);

    drop(connection);
    fs::remove_dir_all(paths.root).unwrap();
}
#[cfg(windows)]
#[test]
fn locked_grid_payload_stays_cleanup_pending_and_retries_after_unlock() {
    use std::fs::OpenOptions;
    use std::os::windows::fs::OpenOptionsExt;

    const FILE_SHARE_READ: u32 = 0x0000_0001;

    let (paths, mut connection, collection_id, icon_ids) = fixture("locked");
    let request_id = prepare_grid(&mut connection, &paths, &collection_id, &icon_ids);
    let source_path: String = connection
        .query_row(
            "SELECT source.original_path_in_library
             FROM ai_request_artifacts artifact
             JOIN source_files source ON source.id = artifact.source_file_id
             WHERE artifact.request_id = ?1 AND artifact.role = 'input_sheet'",
            [&request_id],
            |row| row.get(0),
        )
        .unwrap();
    cancel_ai_grid_request(&connection, &request_id).unwrap();
    let locked_file = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .open(&source_path)
        .unwrap();

    let first = run_ai_web_handoff_maintenance(&connection, &paths).unwrap();
    assert_eq!(first.removed_count, 0);
    assert_eq!(first.deferred_count, 1);
    assert_eq!(first.storage.cleanup_pending_count, 1);
    let pending: (i64, i64, i64) = connection
        .query_row(
            "SELECT
               retention.cleanup_requested_at IS NOT NULL,
               retention.payload_deleted_at IS NOT NULL,
               EXISTS(
                 SELECT 1 FROM ai_request_artifacts artifact
                 WHERE artifact.request_id = retention.request_id
               )
             FROM ai_grid_payload_retention retention
             WHERE retention.request_id = ?1",
            [&request_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(pending, (1, 0, 1));

    drop(locked_file);
    let second = run_ai_web_handoff_maintenance(&connection, &paths).unwrap();
    assert_eq!(second.removed_count, 1);
    assert_eq!(second.deferred_count, 0);
    assert_eq!(second.storage.cleanup_pending_count, 0);
    assert_eq!(second.storage.used_bytes, 0);

    drop(connection);
    fs::remove_dir_all(paths.root).unwrap();
}
