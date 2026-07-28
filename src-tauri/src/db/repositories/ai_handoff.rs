use std::fs::{self, File, OpenOptions};
use std::io::{Cursor, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{Duration, SystemTime};

use image::{DynamicImage, ImageFormat};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::db::repositories::ai as ai_repository;
use crate::db::repositories::ai_grid_retention;
use crate::db::repositories::ai_managed_artifacts;
use crate::db::repositories::ai_snapshots;
use crate::db::repositories::source_files::{
    commit_prepared_source_file, prepare_source_file_from_bytes, SourceFileImportOptions,
};
use crate::error::{AppError, AppResult};
use crate::ids::create_id;
#[cfg(test)]
use crate::imaging::import_limits::MAX_IMPORT_FILE_BYTES;
use crate::imaging::import_limits::{decode_import_image, read_import_file_bytes};
use crate::models::{AiReviewStateDto, ImportImageFilePayload};
use crate::optimization::cache::hash_text;
use crate::paths::AppPaths;

const HANDOFF_KIND: &str = "static_icon_sheet";
const LAYOUT_MODE: &str = "single";
const OPERATION: &str = "edit";
const UPLOAD_FILE_NAME: &str = "upload.png";
const MANIFEST_FILE_NAME: &str = "manifest.json";
const PROMPT_FILE_NAME: &str = "prompt.txt";
const MAX_AI_HANDOFF_BYTES: usize = 16 * 1024 * 1024;
const MAX_USER_PROMPT_BYTES: usize = 2 * 1024;
const MAX_FINAL_PROMPT_BYTES: usize = 4 * 1024;
const ORPHAN_GRACE_PERIOD: Duration = Duration::from_secs(24 * 60 * 60);
pub const AI_WEB_HANDOFF_PAYLOAD_QUOTA_BYTES: u64 = 256 * 1024 * 1024;
const DEFAULT_RECENT_HANDOFF_LIMIT: u32 = 30;
const MAX_RECENT_HANDOFF_LIMIT: u32 = 100;
static AI_WEB_HANDOFF_STORAGE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareAiWebHandoffPayload {
    pub icon_id: Option<String>,
    #[serde(default)]
    pub icon_ids: Vec<String>,
    #[serde(default)]
    pub layout_mode: Option<String>,
    #[serde(default)]
    pub operation: Option<String>,
    pub service_surface: String,
    #[serde(default)]
    pub user_prompt: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AiWebHandoffSessionDto {
    pub request_id: String,
    pub kind: String,
    pub layout_mode: String,
    pub operation: String,
    pub service_surface: String,
    pub final_prompt: String,
    pub upload_file_name: String,
    pub upload_preview_path: String,
    pub expected_width: i64,
    pub expected_height: i64,
    pub expected_has_alpha: bool,
    pub created_at: String,
    pub expires_at: String,
    pub can_extend: bool,
    pub native_drag_supported: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AiWebHandoffIssueDto {
    pub code: String,
    pub severity: String,
    pub message: String,
    pub expected: Option<String>,
    pub actual: Option<String>,
    pub suggested_prompt: Option<String>,
    pub local_action: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiWebHandoffResultInspectionDto {
    pub accepted: bool,
    pub issues: Vec<AiWebHandoffIssueDto>,
    pub validation_signature: Option<String>,
    pub expected_width: i64,
    pub expected_height: i64,
    pub expected_has_alpha: bool,
    pub actual_width: Option<i64>,
    pub actual_height: Option<i64>,
    pub actual_has_alpha: Option<bool>,
    pub review_state: Option<AiReviewStateDto>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AiWebHandoffDragResultDto {
    pub started: bool,
    pub native_drag_supported: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AiWebHandoffDeleteResultDto {
    pub session_closed: bool,
    pub payload_deleted: bool,
    pub cleanup_deferred: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AiWebHandoffCleanupReport {
    pub removed: usize,
    pub deferred: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AiWebHandoffHistoryItemDto {
    pub request_id: String,
    pub request_scope: String,
    pub handoff_kind: String,
    pub collection_id: Option<String>,
    pub icon_id: Option<String>,
    pub collection_name: Option<String>,
    pub icon_name: Option<String>,
    pub service_surface: String,
    pub request_status: String,
    pub payload_state: String,
    pub has_result: bool,
    pub created_at: String,
    pub expires_at: String,
    pub result_received_at: Option<String>,
    pub cleanup_requested_at: Option<String>,
    pub payload_deleted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AiWebHandoffStorageStatusDto {
    pub quota_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
    pub retained_history_count: u64,
    pub live_payload_count: u64,
    pub cleanup_pending_count: u64,
    pub quota_reached: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AiWebHandoffMaintenanceReportDto {
    pub removed_count: u64,
    pub deferred_count: u64,
    pub storage: AiWebHandoffStorageStatusDto,
}

fn lock_ai_web_handoff_storage() -> AppResult<MutexGuard<'static, ()>> {
    AI_WEB_HANDOFF_STORAGE_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| {
            AppError::new(
                "ai_handoff_storage_lock",
                "AI \u{C6F9} \u{C804}\u{B2EC} \u{C784}\u{C2DC} \u{C800}\u{C7A5}\u{C18C}\u{B97C} \u{C7A0}\u{AE00} \u{C218} \u{C5C6}\u{C2B5}\u{B2C8}\u{B2E4}. \u{C571}\u{C744} \u{B2E4}\u{C2DC} \u{C2DC}\u{C791}\u{D574} \u{C8FC}\u{C138}\u{C694}.",
            )
        })
}

pub(crate) struct AiTransferStorageReservation {
    _guard: MutexGuard<'static, ()>,
}

pub(crate) fn reserve_ai_transfer_storage(
    connection: &Connection,
    paths: &AppPaths,
    planned_bytes: u64,
) -> AppResult<AiTransferStorageReservation> {
    reserve_ai_transfer_storage_with_quota(
        connection,
        paths,
        planned_bytes,
        AI_WEB_HANDOFF_PAYLOAD_QUOTA_BYTES,
    )
}

#[cfg(test)]
pub(crate) fn reserve_ai_transfer_storage_with_test_quota(
    connection: &Connection,
    paths: &AppPaths,
    planned_bytes: u64,
    quota_bytes: u64,
) -> AppResult<AiTransferStorageReservation> {
    reserve_ai_transfer_storage_with_quota(connection, paths, planned_bytes, quota_bytes)
}

fn reserve_ai_transfer_storage_with_quota(
    connection: &Connection,
    paths: &AppPaths,
    planned_bytes: u64,
    quota_bytes: u64,
) -> AppResult<AiTransferStorageReservation> {
    let guard = lock_ai_web_handoff_storage()?;
    cleanup_ai_web_handoffs_at(connection, paths, "+0 days")?;
    ensure_ai_web_handoff_quota(connection, paths, planned_bytes, quota_bytes)?;
    Ok(AiTransferStorageReservation { _guard: guard })
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HandoffManifest<'a> {
    schema: &'static str,
    request_id: &'a str,
    kind: &'static str,
    layout_mode: &'static str,
    operation: &'static str,
    service_surface: &'a str,
    upload_file_name: &'static str,
    upload_sha256: &'a str,
    prompt_sha256: &'a str,
    expected_width: i64,
    expected_height: i64,
    expected_has_alpha: bool,
    original_lineage_id: &'a str,
    original_lineage_generation: i64,
    original_source_sha256: &'a str,
    effective_source_sha256: &'a str,
    request_recipe_signature: &'a str,
    activation_revision: i64,
    created_at: &'a str,
    expires_at: &'a str,
}

#[derive(Debug, Clone)]
struct HandoffRecord {
    request_id: String,
    collection_id: String,
    icon_id: String,
    service_surface: String,
    request_status: String,
    capability_snapshot_json: String,
    original_lineage_id: String,
    original_lineage_generation: i64,
    original_source_sha256: String,
    effective_source_sha256: String,
    request_recipe_signature: String,
    activation_revision: i64,
    upload_sha256: String,
    manifest_sha256: String,
    prompt_sha256: String,
    expected_width: i64,
    expected_height: i64,
    expected_has_alpha: bool,
    result_sha256: Option<String>,
    candidate_id: Option<String>,
    result_received_at: Option<String>,
    cleanup_requested_at: Option<String>,
    payload_deleted_at: Option<String>,
    extended_at: Option<String>,
    created_at: String,
    expires_at: String,
    is_expired: bool,
}

#[derive(Debug)]
struct InspectedResult {
    dto: AiWebHandoffResultInspectionDto,
    sha256: String,
    extension: Option<&'static str>,
}

#[derive(Debug)]
struct VerifiedPackageFile {
    path: PathBuf,
    bytes: Vec<u8>,
}

pub fn prepare_ai_web_handoff(
    connection: &mut Connection,
    paths: &AppPaths,
    collection_id: &str,
    payload: PrepareAiWebHandoffPayload,
) -> AppResult<AiWebHandoffSessionDto> {
    prepare_ai_web_handoff_with_quota(
        connection,
        paths,
        collection_id,
        payload,
        AI_WEB_HANDOFF_PAYLOAD_QUOTA_BYTES,
    )
}

#[cfg(test)]
pub(crate) fn prepare_ai_web_handoff_with_test_quota(
    connection: &mut Connection,
    paths: &AppPaths,
    collection_id: &str,
    payload: PrepareAiWebHandoffPayload,
    quota_bytes: u64,
) -> AppResult<AiWebHandoffSessionDto> {
    prepare_ai_web_handoff_with_quota(connection, paths, collection_id, payload, quota_bytes)
}

fn prepare_ai_web_handoff_with_quota(
    connection: &mut Connection,
    paths: &AppPaths,
    collection_id: &str,
    payload: PrepareAiWebHandoffPayload,
    quota_bytes: u64,
) -> AppResult<AiWebHandoffSessionDto> {
    let _storage_guard = lock_ai_web_handoff_storage()?;
    let cleanup_report = cleanup_ai_web_handoffs_at(connection, paths, "+0 days")?;
    let icon_id = validate_prepare_scope(&payload)?;
    let service_surface = validate_service_surface(&payload.service_surface)?;
    validate_user_prompt(&payload.user_prompt)?;

    let current =
        ai_repository::resolve_effective_visual_source(connection, collection_id, icon_id)?;
    if current.render_source.is_animated {
        return Err(AppError::new(
            "ai_handoff_gif_unsupported",
            "GIF 웹 전달은 아직 준비 중입니다. 이번 단계에서는 정적 JPG/PNG 아이콘 한 개만 사용할 수 있습니다.",
        ));
    }
    let input_format = match current.render_source.mime_type.as_str() {
        "image/png" => ImageFormat::Png,
        "image/jpeg" => ImageFormat::Jpeg,
        _ => {
            return Err(AppError::new(
                "ai_handoff_static_format_unsupported",
                "웹 전달은 정적 JPG 또는 PNG 아이콘 한 개만 지원합니다.",
            ));
        }
    };
    let review_state = ai_repository::get_ai_review_state(connection, collection_id, icon_id)?;
    let source_path = validate_managed_source_path(paths, &current.render_source.path)?;
    let source_bytes = read_import_file_bytes(&source_path)?;
    if sha256_hex(&source_bytes) != current.render_source.sha256 {
        return Err(AppError::new(
            "ai_handoff_source_changed",
            "현재 이미지 파일이 준비 도중 변경되어 웹 전달을 중단했습니다.",
        ));
    }
    let source_image = decode_import_image(&source_bytes, input_format)?;
    if i64::from(source_image.width()) != current.render_source.width
        || i64::from(source_image.height()) != current.render_source.height
    {
        return Err(AppError::new(
            "ai_handoff_source_changed",
            "현재 이미지 크기가 저장된 정보와 달라 웹 전달을 중단했습니다.",
        ));
    }
    let upload_bytes = encode_png(&source_image)?;
    if upload_bytes.len() > MAX_AI_HANDOFF_BYTES {
        return Err(AppError::new(
            "ai_handoff_upload_too_large",
            "웹 전달용 PNG가 16MB를 넘습니다. 이미지를 줄이거나 최적화한 뒤 다시 시도해 주세요.",
        ));
    }

    let final_prompt = build_static_single_prompt(
        current.render_source.width,
        current.render_source.height,
        current.render_source.has_alpha,
        &payload.user_prompt,
    )?;
    let upload_sha256 = sha256_hex(&upload_bytes);
    let prompt_sha256 = sha256_hex(final_prompt.as_bytes());
    let request_id = create_id("ai_request");
    let (created_at, expires_at) = retention_times(connection)?;
    let provider = provider_for_surface(service_surface);
    let snapshots = build_snapshots(provider, service_surface)?;
    let payload_input_signature = hash_text(&[
        "pmtcon-ai-web-handoff-v1".to_string(),
        service_surface.to_string(),
        prompt_sha256.clone(),
        upload_sha256.clone(),
        current.original_lineage_id.clone(),
        current.original_lineage_generation.to_string(),
        current.original_source.sha256.clone(),
        current.render_source.sha256.clone(),
        review_state.native_recipe_signature.clone(),
        current.activation_revision.to_string(),
    ]);
    let manifest = HandoffManifest {
        schema: "pmtcon-ai-web-handoff-v1",
        request_id: &request_id,
        kind: HANDOFF_KIND,
        layout_mode: LAYOUT_MODE,
        operation: OPERATION,
        service_surface,
        upload_file_name: UPLOAD_FILE_NAME,
        upload_sha256: &upload_sha256,
        prompt_sha256: &prompt_sha256,
        expected_width: current.render_source.width,
        expected_height: current.render_source.height,
        expected_has_alpha: current.render_source.has_alpha,
        original_lineage_id: &current.original_lineage_id,
        original_lineage_generation: current.original_lineage_generation,
        original_source_sha256: &current.original_source.sha256,
        effective_source_sha256: &current.render_source.sha256,
        request_recipe_signature: &review_state.native_recipe_signature,
        activation_revision: current.activation_revision,
        created_at: &created_at,
        expires_at: &expires_at,
    };
    let manifest_bytes = serde_json::to_vec_pretty(&manifest).map_err(|_| {
        AppError::new(
            "ai_handoff_manifest",
            "웹 전달 패키지 설명 파일을 만들지 못했습니다.",
        )
    })?;
    let manifest_sha256 = sha256_hex(&manifest_bytes);

    let planned_package_bytes =
        checked_payload_bytes(&[upload_bytes.len(), manifest_bytes.len(), final_prompt.len()])?;
    ensure_ai_web_handoff_quota(connection, paths, planned_package_bytes, quota_bytes)?;

    let staging_root = paths.ai_handoffs_dir.join(".staging");
    let staging_dir = staging_root.join(&request_id);
    let final_dir = paths.ai_handoffs_dir.join(&request_id);
    let prepared_dir = ai_managed_artifacts::prepare_owned_directory(
        &paths.root,
        &paths.ai_handoffs_dir,
        &staging_dir,
    )?;
    let package_result = (|| -> AppResult<()> {
        write_new_file(&prepared_dir.join(UPLOAD_FILE_NAME), &upload_bytes)?;
        write_new_file(&prepared_dir.join(MANIFEST_FILE_NAME), &manifest_bytes)?;
        write_new_file(
            &prepared_dir.join(PROMPT_FILE_NAME),
            final_prompt.as_bytes(),
        )?;
        ai_managed_artifacts::promote_owned_directory(
            &paths.root,
            &staging_root,
            &paths.ai_handoffs_dir,
            &staging_dir,
            &final_dir,
        )?;
        Ok(())
    })();
    if let Err(error) = package_result {
        let _ = ai_managed_artifacts::remove_owned_directory_if_present(
            &paths.root,
            &paths.ai_handoffs_dir,
            &staging_dir,
        );
        return Err(error);
    }

    let mut superseded_request_ids = Vec::new();
    let insert_result = (|| -> AppResult<()> {
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current_state =
            ai_repository::get_ai_review_state(&transaction, collection_id, icon_id)?;
        if current_state.native_recipe_signature != review_state.native_recipe_signature {
            return Err(stale_error());
        }
        superseded_request_ids =
            live_ai_web_handoff_request_ids_for_icon(&transaction, collection_id, icon_id)?;
        for superseded_request_id in &superseded_request_ids {
            let request_rows = transaction.execute(
                "UPDATE ai_requests
                 SET status = 'cancelled',
                     updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE id = ?1
                   AND status = 'awaiting_result'",
                [superseded_request_id],
            )?;
            if request_rows == 0 {
                continue;
            }
            let package_rows = transaction.execute(
                "UPDATE ai_web_handoff_packages
                 SET cleanup_requested_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                     updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE request_id = ?1
                   AND cleanup_requested_at IS NULL
                   AND payload_deleted_at IS NULL",
                [superseded_request_id],
            )?;
            if package_rows != 1 {
                return Err(AppError::new(
                    "ai_handoff_cleanup_state",
                    "이전 웹 전달을 안전하게 닫지 못했습니다.",
                ));
            }
        }
        let request_rows = transaction.execute(
            "INSERT INTO ai_requests (
               id, origin_collection_id, origin_icon_id,
               origin_collection_name_snapshot, origin_icon_name_snapshot,
               provider_mode, service_surface, provider, adapter_id,
               adapter_contract_version, account_context, model, operation,
               provenance_trust, credential_mode_snapshot,
               capability_snapshot_json, data_tier_snapshot_json,
               retention_snapshot_json, consent_snapshot_json, policy_refs_json,
               prompt_options_snapshot_json, input_package_sha256,
               original_lineage_id, original_lineage_generation,
               original_source_sha256, effective_source_sha256,
               payload_input_signature, request_recipe_signature,
               activation_revision, status, expires_at, created_at, updated_at
             )
             SELECT
               ?1, c.id, i.id, c.name, i.display_name,
               'manual_web', ?2, ?3, 'pmtcon-web-handoff', '1',
               'unknown', NULL, 'static_image_edit_web_handoff',
               'manual_unverified', 'none',
               ?4, ?5, ?6, ?7, ?8, ?9, ?10,
               ?11, ?12, ?13, ?14, ?15, ?16, ?17,
               'awaiting_result', ?18, ?19, ?19
             FROM icons i
             JOIN collections c ON c.id = i.collection_id
             JOIN effective_visual_sources ev ON ev.icon_id = i.id
             WHERE i.id = ?20
               AND i.collection_id = ?21
               AND i.deleted_at IS NULL
               AND c.deleted_at IS NULL
               AND ev.original_lineage_id = ?22
               AND ev.original_lineage_generation = ?23
               AND ev.original_source_sha256 = ?24
               AND ev.effective_source_sha256 = ?25
               AND ev.activation_revision = ?26",
            params![
                request_id,
                service_surface,
                provider,
                snapshots.capability,
                snapshots.data_tier,
                snapshots.retention,
                snapshots.consent,
                snapshots.policy_refs,
                snapshots.prompt_options,
                upload_sha256,
                current.original_lineage_id,
                current.original_lineage_generation,
                current.original_source.sha256,
                current.render_source.sha256,
                payload_input_signature,
                review_state.native_recipe_signature,
                current.activation_revision,
                expires_at,
                created_at,
                icon_id,
                collection_id,
                current.original_lineage_id,
                current.original_lineage_generation,
                current.original_source.sha256,
                current.render_source.sha256,
                current.activation_revision,
            ],
        )?;
        if request_rows != 1 {
            return Err(stale_error());
        }
        transaction.execute(
            "INSERT INTO ai_web_handoff_packages (
               request_id, handoff_kind, layout_mode, operation, service_surface,
               upload_file_name, upload_sha256,
               manifest_file_name, manifest_sha256,
               prompt_file_name, prompt_sha256,
               expected_width, expected_height, expected_has_alpha,
               created_at, expires_at, updated_at
             ) VALUES (
               ?1, 'static_icon_sheet', 'single', 'edit', ?2,
               'upload.png', ?3, 'manifest.json', ?4, 'prompt.txt', ?5,
               ?6, ?7, ?8, ?9, ?10, ?9
             )",
            params![
                request_id,
                service_surface,
                upload_sha256,
                manifest_sha256,
                prompt_sha256,
                current.render_source.width,
                current.render_source.height,
                i64::from(current.render_source.has_alpha),
                created_at,
                expires_at,
            ],
        )?;
        transaction.commit()?;
        Ok(())
    })();
    if let Err(error) = insert_result {
        let _ = ai_managed_artifacts::remove_owned_directory_if_present(
            &paths.root,
            &paths.ai_handoffs_dir,
            &final_dir,
        );
        return Err(error);
    }

    let mut prepared_session = get_ai_web_handoff(connection, paths, &request_id)?;
    let mut cleanup_deferred = cleanup_report.deferred > 0;
    for superseded_request_id in superseded_request_ids {
        if superseded_request_id == request_id {
            continue;
        }
        match delete_ai_web_handoff_payload(connection, paths, &superseded_request_id) {
            Ok(result) if result.cleanup_deferred => cleanup_deferred = true,
            Err(_) => cleanup_deferred = true,
            Ok(_) => {}
        }
    }
    if cleanup_deferred {
        prepared_session.warnings.push(
            "이전 웹 전달을 닫는 중 일부 정리가 지연됐습니다. 다음 앱 시작 때 다시 정리합니다."
                .to_string(),
        );
    }
    Ok(prepared_session)
}

pub fn get_ai_web_handoff(
    connection: &mut Connection,
    paths: &AppPaths,
    request_id: &str,
) -> AppResult<AiWebHandoffSessionDto> {
    let record = load_handoff_record(connection, request_id)?;
    require_live_payload(connection, paths, &record)?;
    require_current_handoff_payload(connection, paths, &record)?;
    session_from_record(paths, &record)
}

pub fn get_latest_ai_web_handoff_for_icon(
    connection: &mut Connection,
    paths: &AppPaths,
    collection_id: &str,
    icon_id: &str,
) -> AppResult<Option<AiWebHandoffSessionDto>> {
    cleanup_ai_web_handoffs(connection, paths)?;
    let request_ids = live_ai_web_handoff_request_ids_for_icon(connection, collection_id, icon_id)?;
    for request_id in request_ids {
        match get_ai_web_handoff(connection, paths, &request_id) {
            Ok(session) => return Ok(Some(session)),
            Err(error)
                if matches!(
                    error.code.as_str(),
                    "ai_handoff_stale"
                        | "ai_handoff_expired"
                        | "ai_handoff_payload_deleted"
                        | "ai_handoff_closed"
                ) => {}
            Err(error) => return Err(error),
        }
    }
    Ok(None)
}

pub fn list_recent_ai_web_handoffs(
    connection: &Connection,
    limit: Option<u32>,
) -> AppResult<Vec<AiWebHandoffHistoryItemDto>> {
    let limit = limit.unwrap_or(DEFAULT_RECENT_HANDOFF_LIMIT);
    if limit == 0 {
        return Err(AppError::new(
            "ai_handoff_history_limit",
            "\u{CD5C}\u{ADFC} AI \u{C6F9} \u{C804}\u{B2EC} \u{BAA9}\u{B85D}\u{C740} \u{D55C} \u{AC74} \u{C774}\u{C0C1} \u{C694}\u{CCAD}\u{D574}\u{C57C} \u{D569}\u{B2C8}\u{B2E4}.",
        ));
    }
    let limit = i64::from(limit.min(MAX_RECENT_HANDOFF_LIMIT));
    let mut statement = connection.prepare(
        "WITH recent_handoffs AS (
           SELECT
             request.id AS request_id,
             request.request_scope,
             package.handoff_kind,
             request.origin_collection_id,
             request.origin_icon_id,
             request.origin_collection_name_snapshot,
             request.origin_icon_name_snapshot,
             package.service_surface,
             request.status AS request_status,
             CASE
               WHEN package.payload_deleted_at IS NOT NULL THEN 'deleted'
               WHEN package.cleanup_requested_at IS NOT NULL
                 OR package.result_received_at IS NOT NULL
                 OR request.status IN ('completed', 'failed', 'cancelled', 'expired')
                 THEN 'cleanup_pending'
               WHEN julianday(package.expires_at) <= julianday('now') THEN 'expired'
               WHEN request.status = 'awaiting_result' THEN 'available'
               ELSE 'closed'
             END AS payload_state,
             package.result_received_at IS NOT NULL AS has_result,
             package.created_at,
             package.expires_at,
             package.result_received_at,
             package.cleanup_requested_at,
             package.payload_deleted_at
           FROM ai_web_handoff_packages package
           JOIN ai_requests request ON request.id = package.request_id

           UNION ALL

           SELECT
             request.id,
             request.request_scope,
             'ai_grid_sheet',
             request.origin_collection_id,
             NULL,
             request.origin_collection_name_snapshot,
             CASE request.request_scope
               WHEN 'grid_edit' THEN printf(
                 '아이콘 %d개 그리드 편집',
                 (SELECT COUNT(*) FROM ai_request_items item
                  WHERE item.request_id = request.id)
               )
               WHEN 'single_generate' THEN 'AI 아이콘 1개 생성'
               ELSE printf(
                 'AI 아이콘 %d개 그리드 생성',
                 (SELECT COUNT(*) FROM ai_request_items item
                  WHERE item.request_id = request.id)
               )
             END,
             request.service_surface,
             request.status,
             CASE
               WHEN retention.payload_deleted_at IS NOT NULL THEN 'deleted'
               WHEN retention.cleanup_requested_at IS NOT NULL
                 OR request.status IN ('completed', 'failed', 'cancelled', 'expired')
                 THEN 'cleanup_pending'
               WHEN julianday(retention.expires_at) <= julianday('now') THEN 'expired'
               WHEN request.request_scope = 'grid_edit'
                 AND request.status IN ('prepared', 'awaiting_result')
                 AND EXISTS(
                  SELECT 1 FROM ai_request_artifacts artifact
                  WHERE artifact.request_id = request.id
                    AND artifact.role = 'input_sheet'
                )
                THEN 'available'
              ELSE 'closed'
            END,
            EXISTS(
              SELECT 1 FROM ai_request_artifacts artifact
              WHERE artifact.request_id = request.id
                AND artifact.role = 'output_sheet'
            ) OR EXISTS(
              SELECT 1 FROM ai_candidates candidate
              WHERE candidate.request_id = request.id
            ),
            retention.created_at,
            retention.expires_at,
            (SELECT MIN(artifact.created_at)
             FROM ai_request_artifacts artifact
             WHERE artifact.request_id = request.id
               AND artifact.role = 'output_sheet'),
            retention.cleanup_requested_at,
            retention.payload_deleted_at
          FROM ai_grid_payload_retention retention
          JOIN ai_requests request ON request.id = retention.request_id
        )
        SELECT
          request_id, request_scope, handoff_kind,
          origin_collection_id, origin_icon_id,
          origin_collection_name_snapshot, origin_icon_name_snapshot,
          service_surface, request_status, payload_state, has_result,
          created_at, expires_at, result_received_at,
          cleanup_requested_at, payload_deleted_at
        FROM recent_handoffs
        ORDER BY julianday(created_at) DESC, request_id DESC
        LIMIT ?1",
    )?;
    let items = statement
        .query_map([limit], |row| {
            Ok(AiWebHandoffHistoryItemDto {
                request_id: row.get(0)?,
                request_scope: row.get(1)?,
                handoff_kind: row.get(2)?,
                collection_id: row.get(3)?,
                icon_id: row.get(4)?,
                collection_name: row.get(5)?,
                icon_name: row.get(6)?,
                service_surface: row.get(7)?,
                request_status: row.get(8)?,
                payload_state: row.get(9)?,
                has_result: row.get::<_, i64>(10)? != 0,
                created_at: row.get(11)?,
                expires_at: row.get(12)?,
                result_received_at: row.get(13)?,
                cleanup_requested_at: row.get(14)?,
                payload_deleted_at: row.get(15)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}

pub fn get_ai_web_handoff_storage_status(
    connection: &Connection,
    paths: &AppPaths,
) -> AppResult<AiWebHandoffStorageStatusDto> {
    let _storage_guard = lock_ai_web_handoff_storage()?;
    ai_web_handoff_storage_status_unlocked(connection, paths)
}

pub fn run_ai_web_handoff_maintenance(
    connection: &Connection,
    paths: &AppPaths,
) -> AppResult<AiWebHandoffMaintenanceReportDto> {
    let _storage_guard = lock_ai_web_handoff_storage()?;
    let cleanup = cleanup_ai_web_handoffs_at(connection, paths, "+0 days")?;
    let storage = ai_web_handoff_storage_status_unlocked(connection, paths)?;
    Ok(AiWebHandoffMaintenanceReportDto {
        removed_count: cleanup.removed as u64,
        deferred_count: cleanup.deferred as u64,
        storage,
    })
}

fn ai_web_handoff_storage_status_unlocked(
    connection: &Connection,
    paths: &AppPaths,
) -> AppResult<AiWebHandoffStorageStatusDto> {
    let web_counts = connection.query_row(
        "SELECT
           COUNT(*),
           COALESCE(SUM(CASE
             WHEN package.payload_deleted_at IS NULL
              AND package.cleanup_requested_at IS NULL
              AND package.result_received_at IS NULL
              AND request.status = 'awaiting_result'
              AND julianday(package.expires_at) > julianday('now')
             THEN 1 ELSE 0 END), 0),
           COALESCE(SUM(CASE
             WHEN package.payload_deleted_at IS NULL
              AND (
                package.cleanup_requested_at IS NOT NULL
                OR package.result_received_at IS NOT NULL
                OR request.status IN ('completed', 'failed', 'cancelled', 'expired')
                OR julianday(package.expires_at) <= julianday('now')
              )
             THEN 1 ELSE 0 END), 0)
         FROM ai_web_handoff_packages package
         JOIN ai_requests request ON request.id = package.request_id",
        [],
        |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
            ))
        },
    )?;
    let grid_counts = connection.query_row(
        "SELECT
           COUNT(*),
           COALESCE(SUM(CASE
             WHEN retention.payload_deleted_at IS NULL
              AND retention.cleanup_requested_at IS NULL
              AND request.status IN ('prepared', 'awaiting_result', 'layout_review_pending')
              AND julianday(retention.expires_at) > julianday('now')
              AND EXISTS(
                SELECT 1 FROM ai_request_artifacts artifact
                WHERE artifact.request_id = request.id
              )
             THEN 1 ELSE 0 END), 0),
           COALESCE(SUM(CASE
             WHEN retention.payload_deleted_at IS NULL
              AND (
                retention.cleanup_requested_at IS NOT NULL
                OR request.status IN ('completed', 'failed', 'cancelled', 'expired')
                OR julianday(retention.expires_at) <= julianday('now')
              )
             THEN 1 ELSE 0 END), 0)
         FROM ai_grid_payload_retention retention
         JOIN ai_requests request ON request.id = retention.request_id",
        [],
        |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
            ))
        },
    )?;
    let used_bytes = managed_ai_transfer_storage_bytes(connection, paths)?;
    Ok(AiWebHandoffStorageStatusDto {
        quota_bytes: AI_WEB_HANDOFF_PAYLOAD_QUOTA_BYTES,
        used_bytes,
        available_bytes: AI_WEB_HANDOFF_PAYLOAD_QUOTA_BYTES.saturating_sub(used_bytes),
        retained_history_count: checked_count_sum(web_counts.0, grid_counts.0)?,
        live_payload_count: checked_count_sum(web_counts.1, grid_counts.1)?,
        cleanup_pending_count: checked_count_sum(web_counts.2, grid_counts.2)?,
        quota_reached: used_bytes >= AI_WEB_HANDOFF_PAYLOAD_QUOTA_BYTES,
    })
}
fn live_ai_web_handoff_request_ids_for_icon(
    connection: &Connection,
    collection_id: &str,
    icon_id: &str,
) -> AppResult<Vec<String>> {
    let mut statement = connection.prepare(
        "SELECT package.request_id
         FROM ai_web_handoff_packages package
         JOIN ai_requests request ON request.id = package.request_id
         WHERE request.origin_collection_id = ?1
           AND request.origin_icon_id = ?2
           AND request.status = 'awaiting_result'
           AND package.result_received_at IS NULL
           AND package.cleanup_requested_at IS NULL
           AND package.payload_deleted_at IS NULL
           AND julianday(package.expires_at) > julianday('now')
         ORDER BY julianday(package.created_at) DESC, package.request_id DESC",
    )?;
    let request_ids = statement
        .query_map(params![collection_id, icon_id], |row| {
            row.get::<_, String>(0)
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(request_ids)
}

#[cfg(test)]
pub(crate) fn get_ai_web_handoff_after_days(
    connection: &mut Connection,
    paths: &AppPaths,
    request_id: &str,
    days: u32,
) -> AppResult<AiWebHandoffSessionDto> {
    let cutoff_modifier = format!("+{days} days");
    let record = load_handoff_record_at(connection, request_id, &cutoff_modifier)?;
    require_live_payload(connection, paths, &record)?;
    session_from_record(paths, &record)
}

pub fn reveal_ai_web_handoff_upload(
    connection: &mut Connection,
    paths: &AppPaths,
    request_id: &str,
) -> AppResult<()> {
    let record = load_handoff_record(connection, request_id)?;
    require_live_payload(connection, paths, &record)?;
    require_current_handoff_payload(connection, paths, &record)?;
    let upload = verified_package_file(
        paths,
        request_id,
        UPLOAD_FILE_NAME,
        &record.upload_sha256,
        MAX_AI_HANDOFF_BYTES,
    )?;
    crate::export::open_export_path(upload.path.to_string_lossy().as_ref())
}

pub(crate) fn verified_ai_web_handoff_upload_path(
    connection: &mut Connection,
    paths: &AppPaths,
    request_id: &str,
) -> AppResult<PathBuf> {
    let record = load_handoff_record(connection, request_id)?;
    require_live_payload(connection, paths, &record)?;
    require_current_handoff_payload(connection, paths, &record)?;
    let upload = verified_package_file(
        paths,
        request_id,
        UPLOAD_FILE_NAME,
        &record.upload_sha256,
        MAX_AI_HANDOFF_BYTES,
    )?;
    Ok(upload.path)
}

pub fn validate_ai_web_handoff_result(
    connection: &mut Connection,
    paths: &AppPaths,
    request_id: &str,
    file: &ImportImageFilePayload,
) -> AppResult<AiWebHandoffResultInspectionDto> {
    let record = load_handoff_record(connection, request_id)?;
    require_live_payload(connection, paths, &record)?;
    ensure_package_integrity(paths, &record)?;
    let is_current = request_is_current(connection, &record)?;
    Ok(inspect_result(&record, file, is_current)?.dto)
}

pub fn commit_ai_web_handoff_result(
    connection: &mut Connection,
    paths: &AppPaths,
    request_id: &str,
    file: ImportImageFilePayload,
    expected_validation_signature: &str,
) -> AppResult<AiWebHandoffResultInspectionDto> {
    if expected_validation_signature.trim().is_empty() {
        return Err(AppError::new(
            "ai_handoff_validation_required",
            "결과를 적용하기 전에 현재 파일 검사를 먼저 완료해 주세요.",
        ));
    }
    let record = load_handoff_record(connection, request_id)?;
    require_live_payload(connection, paths, &record)?;
    ensure_package_integrity(paths, &record)?;
    let is_current = request_is_current(connection, &record)?;
    let mut inspected = inspect_result(&record, &file, is_current)?;
    if !inspected.dto.accepted {
        return Ok(inspected.dto);
    }
    if inspected.dto.validation_signature.as_deref() != Some(expected_validation_signature) {
        push_blocking_issue(
            &mut inspected.dto,
            AiWebHandoffIssueDto {
                code: "ai_handoff_result_signature_mismatch".to_string(),
                severity: "blocking".to_string(),
                message: "검사한 뒤 결과 파일 또는 편집 상태가 바뀌었습니다. 다시 검사해 주세요."
                    .to_string(),
                expected: None,
                actual: None,
                suggested_prompt: None,
                local_action: Some("결과 검사 버튼을 다시 누르세요.".to_string()),
            },
        );
        return Ok(inspected.dto);
    }
    let extension = inspected.extension.ok_or_else(|| {
        AppError::new(
            "ai_handoff_result_format",
            "결과 이미지 형식을 확인할 수 없습니다.",
        )
    })?;
    let normalized_file = ImportImageFilePayload {
        original_filename: format!("ai-web-result.{extension}"),
        bytes: file.bytes,
    };
    let prepared = prepare_source_file_from_bytes(
        &normalized_file,
        SourceFileImportOptions {
            allow_gif: false,
            exact_dimensions: Some((record.expected_width, record.expected_height)),
        },
    )?;
    let artifact_snapshot = prepared.artifact_snapshot(connection, paths)?;
    let candidate_id = create_id("ai_candidate");
    let commit_result = (|| -> AppResult<()> {
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let transaction_record = load_handoff_record(&transaction, request_id)?;
        if transaction_record.payload_deleted_at.is_some()
            || transaction_record.cleanup_requested_at.is_some()
            || transaction_record.request_status != "awaiting_result"
            || transaction_record.is_expired
            || transaction_record.result_received_at.is_some()
            || transaction_record.result_sha256.is_some()
            || transaction_record.candidate_id.is_some()
        {
            return Err(AppError::new(
                "ai_handoff_result_already_committed",
                "이 웹 전달 요청에는 이미 결과가 등록됐거나 더 이상 결과를 받을 수 없습니다.",
            ));
        }
        if !request_is_current(&transaction, &transaction_record)? {
            return Err(stale_error());
        }
        let stored = commit_prepared_source_file(&transaction, paths, &prepared)?;
        let has_alpha = stored.has_alpha.ok_or_else(|| {
            AppError::new(
                "ai_handoff_result_metadata",
                "결과 이미지의 투명도 정보를 확인하지 못했습니다.",
            )
        })?;
        transaction.execute(
            "INSERT INTO ai_candidates (
               id, request_id, candidate_index, raw_source_file_id,
               raw_source_sha256, output_format, width, height, is_animated,
               has_alpha, provider_capabilities_snapshot_json, created_at
             ) VALUES (
               ?1, ?2, 0, ?3, ?4, ?5, ?6, ?7, 0, ?8, ?9,
               strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             )",
            params![
                candidate_id,
                request_id,
                stored.id,
                stored.sha256,
                stored.original_extension,
                stored.width,
                stored.height,
                i64::from(has_alpha),
                transaction_record.capability_snapshot_json,
            ],
        )?;
        let request_rows = transaction.execute(
            "UPDATE ai_requests
             SET status = 'completed',
                 completed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?1
               AND status = 'awaiting_result'",
            [request_id],
        )?;
        if request_rows != 1 {
            return Err(AppError::new(
                "ai_handoff_result_stale",
                "결과를 저장하는 동안 AI 요청 상태가 변경됐습니다.",
            ));
        }
        let package_rows = transaction.execute(
            "UPDATE ai_web_handoff_packages
             SET result_sha256 = ?1,
                 candidate_id = ?2,
                 result_received_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                 cleanup_requested_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE request_id = ?3
               AND candidate_id IS NULL
               AND result_received_at IS NULL
               AND cleanup_requested_at IS NULL
               AND payload_deleted_at IS NULL
               AND julianday(expires_at) > julianday('now')",
            params![inspected.sha256, candidate_id, request_id],
        )?;
        if package_rows != 1 {
            return Err(AppError::new(
                "ai_handoff_result_stale",
                "결과를 저장하는 동안 웹 전달 세션이 변경됐습니다.",
            ));
        }
        transaction.commit()?;
        Ok(())
    })();
    if let Err(error) = commit_result {
        let _ = artifact_snapshot.cleanup_if_unreferenced(connection);
        if error.code == "ai_handoff_stale" {
            push_blocking_issue(
                &mut inspected.dto,
                stale_issue(record.expected_width, record.expected_height),
            );
            return Ok(inspected.dto);
        }
        return Err(error);
    }

    let package_dir = paths.ai_handoffs_dir.join(request_id);
    match ai_managed_artifacts::remove_owned_directory_if_present(
        &paths.root,
        &paths.ai_handoffs_dir,
        &package_dir,
    ) {
        Ok(()) => {
            if finish_payload_deletion(connection, request_id).is_err() {
                inspected.dto.issues.push(cleanup_tracking_warning());
            }
        }
        Err(_) => inspected.dto.issues.push(cleanup_deferred_warning()),
    }
    match ai_repository::get_ai_review_state(connection, &record.collection_id, &record.icon_id) {
        Ok(review_state) => inspected.dto.review_state = Some(review_state),
        Err(_) => inspected.dto.issues.push(AiWebHandoffIssueDto {
            code: "ai_handoff_review_refresh_deferred".to_string(),
            severity: "warning".to_string(),
            message: "결과 후보는 저장됐지만 검토 목록을 새로 읽지 못했습니다. AI 검토 영역을 다시 열어 주세요."
                .to_string(),
            expected: None,
            actual: None,
            suggested_prompt: None,
            local_action: Some("AI 검토 영역을 닫았다가 다시 여세요.".to_string()),
        }),
    }
    Ok(inspected.dto)
}

fn finish_payload_deletion(connection: &Connection, request_id: &str) -> AppResult<()> {
    let rows = connection.execute(
        "UPDATE ai_web_handoff_packages
         SET payload_deleted_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE request_id = ?1
           AND cleanup_requested_at IS NOT NULL
           AND payload_deleted_at IS NULL",
        [request_id],
    )?;
    if rows > 1 {
        return Err(AppError::new(
            "ai_handoff_cleanup_state",
            "웹 전달 파일 정리 상태가 일관되지 않습니다.",
        ));
    }
    Ok(())
}

fn cleanup_deferred_warning() -> AiWebHandoffIssueDto {
    AiWebHandoffIssueDto {
        code: "ai_handoff_payload_cleanup_deferred".to_string(),
        severity: "warning".to_string(),
        message: "결과는 저장됐지만 임시 전달 파일 정리는 다음 실행 때 다시 시도합니다."
            .to_string(),
        expected: None,
        actual: None,
        suggested_prompt: None,
        local_action: None,
    }
}

fn cleanup_tracking_warning() -> AiWebHandoffIssueDto {
    AiWebHandoffIssueDto {
        code: "ai_handoff_payload_cleanup_tracking_deferred".to_string(),
        severity: "warning".to_string(),
        message: "결과와 후보는 저장됐지만 파일 정리 완료 표시를 나중에 다시 확인합니다."
            .to_string(),
        expected: None,
        actual: None,
        suggested_prompt: None,
        local_action: None,
    }
}

fn request_payload_cleanup(
    connection: &Connection,
    request_id: &str,
    terminal_status: Option<&str>,
) -> AppResult<()> {
    if terminal_status.is_some_and(|status| !matches!(status, "cancelled" | "expired")) {
        return Err(AppError::new(
            "ai_handoff_cleanup_state",
            "허용되지 않은 웹 전달 종료 상태입니다.",
        ));
    }
    let transaction = connection.unchecked_transaction()?;
    if let Some(status) = terminal_status {
        transaction.execute(
            "UPDATE ai_requests
             SET status = ?1,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?2
               AND status = 'awaiting_result'",
            params![status, request_id],
        )?;
    }
    transaction.execute(
        "UPDATE ai_web_handoff_packages
         SET cleanup_requested_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE request_id = ?1
           AND cleanup_requested_at IS NULL
           AND payload_deleted_at IS NULL",
        [request_id],
    )?;
    transaction.commit()?;
    Ok(())
}
pub fn extend_ai_web_handoff_retention(
    connection: &mut Connection,
    paths: &AppPaths,
    request_id: &str,
) -> AppResult<AiWebHandoffSessionDto> {
    validate_request_id(request_id)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let rows = transaction.execute(
        "UPDATE ai_web_handoff_packages
         SET expires_at = strftime(
               '%Y-%m-%dT%H:%M:%fZ',
               expires_at,
               '+30 days'
             ),
             extended_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE request_id = ?1
           AND extended_at IS NULL
           AND cleanup_requested_at IS NULL
           AND payload_deleted_at IS NULL
           AND result_received_at IS NULL
           AND julianday(expires_at) > julianday('now')
           AND EXISTS (
             SELECT 1 FROM ai_requests request
             WHERE request.id = ai_web_handoff_packages.request_id
               AND request.status = 'awaiting_result'
           )",
        [request_id],
    )?;
    if rows != 1 {
        return Err(AppError::new(
            "ai_handoff_retention_unavailable",
            "이 전달 세션은 이미 연장했거나 만료되어 더 연장할 수 없습니다.",
        ));
    }
    transaction.execute(
        "UPDATE ai_requests
         SET expires_at = (
               SELECT expires_at
               FROM ai_web_handoff_packages
               WHERE request_id = ?1
             ),
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?1
           AND status = 'awaiting_result'",
        [request_id],
    )?;
    transaction.commit()?;
    get_ai_web_handoff(connection, paths, request_id)
}

pub fn delete_ai_web_handoff_payload(
    connection: &mut Connection,
    paths: &AppPaths,
    request_id: &str,
) -> AppResult<AiWebHandoffDeleteResultDto> {
    let record = load_handoff_record(connection, request_id)?;
    if record.payload_deleted_at.is_some() {
        return Ok(AiWebHandoffDeleteResultDto {
            session_closed: true,
            payload_deleted: true,
            cleanup_deferred: false,
        });
    }
    if record.cleanup_requested_at.is_none() {
        request_payload_cleanup(connection, request_id, Some("cancelled"))?;
    }
    let package_dir = paths.ai_handoffs_dir.join(request_id);
    let payload_deleted = ai_managed_artifacts::remove_owned_directory_if_present(
        &paths.root,
        &paths.ai_handoffs_dir,
        &package_dir,
    )
    .is_ok();
    let cleanup_deferred = !payload_deleted
        || (payload_deleted && finish_payload_deletion(connection, request_id).is_err());
    Ok(AiWebHandoffDeleteResultDto {
        session_closed: true,
        payload_deleted,
        cleanup_deferred,
    })
}
pub fn cleanup_ai_web_handoffs(
    connection: &Connection,
    paths: &AppPaths,
) -> AppResult<AiWebHandoffCleanupReport> {
    let _storage_guard = lock_ai_web_handoff_storage()?;
    cleanup_ai_web_handoffs_at(connection, paths, "+0 days")
}

#[cfg(test)]
pub(crate) fn cleanup_ai_web_handoffs_after_days(
    connection: &Connection,
    paths: &AppPaths,
    days: u32,
) -> AppResult<AiWebHandoffCleanupReport> {
    let cutoff_modifier = format!("+{days} days");
    let _storage_guard = lock_ai_web_handoff_storage()?;
    cleanup_ai_web_handoffs_at(connection, paths, &cutoff_modifier)
}

fn cleanup_ai_web_handoffs_at(
    connection: &Connection,
    paths: &AppPaths,
    cutoff_modifier: &str,
) -> AppResult<AiWebHandoffCleanupReport> {
    let mut statement = connection.prepare(
        "SELECT package.request_id,
                julianday(package.expires_at) <= julianday('now', ?1) AS is_expired
         FROM ai_web_handoff_packages package
         JOIN ai_requests request ON request.id = package.request_id
         WHERE package.payload_deleted_at IS NULL
           AND (
             package.cleanup_requested_at IS NOT NULL
             OR julianday(package.expires_at) <= julianday('now', ?1)
             OR package.result_received_at IS NOT NULL
             OR request.status IN ('completed', 'failed', 'cancelled', 'expired')
           )",
    )?;
    let candidates = statement
        .query_map([cutoff_modifier], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? != 0))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    drop(statement);

    let mut report = AiWebHandoffCleanupReport::default();
    for (request_id, is_expired) in candidates {
        if validate_request_id(&request_id).is_err() {
            report.deferred += 1;
            continue;
        }
        let terminal_status = is_expired.then_some("expired");
        if request_payload_cleanup(connection, &request_id, terminal_status).is_err() {
            report.deferred += 1;
            continue;
        }
        let package_dir = paths.ai_handoffs_dir.join(&request_id);
        if ai_managed_artifacts::remove_owned_directory_if_present(
            &paths.root,
            &paths.ai_handoffs_dir,
            &package_dir,
        )
        .is_err()
        {
            report.deferred += 1;
            continue;
        }
        if finish_payload_deletion(connection, &request_id).is_err() {
            report.deferred += 1;
            continue;
        }
        report.removed += 1;
    }
    let grid_report =
        ai_grid_retention::cleanup_ai_grid_payloads_at(connection, paths, cutoff_modifier)?;
    report.removed += grid_report.removed;
    report.deferred += grid_report.deferred;
    let orphan_report = cleanup_orphan_directories(connection, paths)?;
    report.removed += orphan_report.removed;
    report.deferred += orphan_report.deferred;
    Ok(report)
}
fn validate_prepare_scope(payload: &PrepareAiWebHandoffPayload) -> AppResult<&str> {
    if payload
        .layout_mode
        .as_deref()
        .is_some_and(|mode| mode != LAYOUT_MODE)
        || !payload.icon_ids.is_empty()
    {
        return Err(AppError::new(
            "ai_handoff_grid_unsupported",
            "여러 아이콘 그리드 전달은 아직 준비 중입니다. 이번 단계에서는 아이콘 한 개만 사용할 수 있습니다.",
        ));
    }
    if payload
        .operation
        .as_deref()
        .is_some_and(|operation| operation != OPERATION)
    {
        return Err(AppError::new(
            "ai_handoff_generation_unsupported",
            "원본 없는 AI 생성은 아직 준비 중입니다. 이번 단계에서는 기존 정적 아이콘 편집만 사용할 수 있습니다.",
        ));
    }
    payload
        .icon_id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            AppError::new(
                "ai_handoff_icon_required",
                "웹으로 전달할 아이콘 한 개를 선택해 주세요.",
            )
        })
}

fn validate_service_surface(value: &str) -> AppResult<&str> {
    match value {
        "novelai_web" | "chatgpt_web" | "gemini_web" | "other_manual" => Ok(value),
        _ => Err(AppError::new(
            "ai_handoff_service_surface_unsupported",
            "지원하는 AI 웹 화면을 선택해 주세요.",
        )),
    }
}

fn provider_for_surface(surface: &str) -> &'static str {
    match surface {
        "novelai_web" => "novelai",
        "chatgpt_web" => "openai",
        "gemini_web" => "gemini",
        _ => "manual",
    }
}

fn validate_user_prompt(prompt: &str) -> AppResult<()> {
    if prompt.len() > MAX_USER_PROMPT_BYTES {
        return Err(AppError::new(
            "ai_handoff_prompt_too_long",
            "사용자 프롬프트는 UTF-8 기준 2KB까지 입력할 수 있습니다.",
        ));
    }
    if prompt
        .chars()
        .any(|character| character.is_control() && !matches!(character, '\r' | '\n' | '\t'))
    {
        return Err(AppError::new(
            "ai_handoff_prompt_invalid",
            "프롬프트에 사용할 수 없는 제어 문자가 있습니다.",
        ));
    }
    let lower = prompt.to_ascii_lowercase();
    if [
        "authorization:",
        "bearer ",
        "cookie:",
        "api_key=",
        "apikey=",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
    {
        return Err(AppError::new(
            "ai_handoff_prompt_secret",
            "프롬프트에 API 키, 토큰, 쿠키처럼 보이는 값이 있습니다. 비밀값을 지운 뒤 다시 시도해 주세요.",
        ));
    }
    Ok(())
}

fn build_static_single_prompt(
    width: i64,
    height: i64,
    has_alpha: bool,
    user_prompt: &str,
) -> AppResult<String> {
    let alpha_rule = if has_alpha {
        "Preserve the transparent background and every transparent region."
    } else {
        "Do not add an unintended border, frame, or padding."
    };
    let user_prompt = user_prompt.trim();
    let user_section = if user_prompt.is_empty() {
        "(추가 요청 없음)"
    } else {
        user_prompt
    };
    let prompt = format!(
        "PMTCONCON Studio 단일 정적 이모티콘 편집 작업입니다.\n\
첨부된 upload.png는 스프라이트 시트가 아닌 이미지 한 장입니다.\n\
\n\
필수 출력 조건:\n\
- Return exactly one PNG image only.\n\
- Keep the canvas exactly {width}×{height}px.\n\
- Do not crop, resize, reorder, split, merge, or add margins.\n\
- {alpha_rule}\n\
- Keep the main character/object identity and placement unless the user explicitly asks otherwise.\n\
- Do not include captions, explanations, grids, guides, labels, or watermarks in the image.\n\
\n\
사용자 편집 요청:\n\
{user_section}"
    );
    if prompt.len() > MAX_FINAL_PROMPT_BYTES {
        return Err(AppError::new(
            "ai_handoff_prompt_too_long",
            "기본 지시를 포함한 최종 프롬프트가 4KB를 넘습니다. 사용자 요청을 줄여 주세요.",
        ));
    }
    Ok(prompt)
}

struct RequestSnapshots {
    capability: String,
    data_tier: String,
    retention: String,
    consent: String,
    policy_refs: String,
    prompt_options: String,
}

fn build_snapshots(provider: &str, service_surface: &str) -> AppResult<RequestSnapshots> {
    Ok(RequestSnapshots {
        capability: ai_snapshots::canonicalize(
            "capability",
            &json!({
                "schema": "pmtcon-ai-capability-v1",
                "provider": provider,
                "serviceSurface": service_surface,
                "source": "manual-web-handoff",
                "supports": ["image-output"],
                "limitations": [
                    "single-static-input",
                    "manual-result-download",
                    "provider-upload-not-observable"
                ]
            })
            .to_string(),
        )?,
        data_tier: ai_snapshots::canonicalize(
            "data_tier",
            r#"{"schema":"pmtcon-ai-data-tier-v1","source":"manual-web-handoff","tier":"unknown"}"#,
        )?,
        retention: ai_snapshots::canonicalize(
            "retention",
            r#"{"schema":"pmtcon-ai-retention-v1","source":"local-package","retention":"7-days-one-time-30-day-extension"}"#,
        )?,
        consent: ai_snapshots::canonicalize(
            "consent",
            r#"{"schema":"pmtcon-ai-consent-v1","source":"manual-web-handoff","confirmed":false,"humanActionConfirmed":true,"requestContentConfirmed":true}"#,
        )?,
        policy_refs: ai_snapshots::canonicalize("policy_refs", "[]")?,
        prompt_options: ai_snapshots::canonicalize(
            "prompt_options",
            &json!({
                "schema": "pmtcon-ai-prompt-options-v1",
                "operation": "static_image_edit_web_handoff",
                "provider": provider,
                "outputCount": 1
            })
            .to_string(),
        )?,
    })
}

fn retention_times(connection: &Connection) -> AppResult<(String, String)> {
    connection
        .query_row(
            "SELECT
               strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
               strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '+7 days')",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(Into::into)
}

fn encode_png(image: &DynamicImage) -> AppResult<Vec<u8>> {
    let mut cursor = Cursor::new(Vec::new());
    image.write_to(&mut cursor, ImageFormat::Png).map_err(|_| {
        AppError::new(
            "ai_handoff_upload_encode",
            "웹 전달용 PNG를 만들지 못했습니다.",
        )
    })?;
    Ok(cursor.into_inner())
}

fn write_new_file(path: &Path, bytes: &[u8]) -> AppResult<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn load_handoff_record(connection: &Connection, request_id: &str) -> AppResult<HandoffRecord> {
    load_handoff_record_at(connection, request_id, "+0 days")
}

fn load_handoff_record_at(
    connection: &Connection,
    request_id: &str,
    cutoff_modifier: &str,
) -> AppResult<HandoffRecord> {
    validate_request_id(request_id)?;
    connection
        .query_row(
            "SELECT
               package.request_id,
               request.origin_collection_id,
               request.origin_icon_id,
               package.service_surface,
               request.status,
               request.capability_snapshot_json,
               request.original_lineage_id,
               request.original_lineage_generation,
               request.original_source_sha256,
               request.effective_source_sha256,
               request.request_recipe_signature,
               request.activation_revision,
               package.upload_sha256,
               package.manifest_sha256,
               package.prompt_sha256,
               package.expected_width,
               package.expected_height,
               package.expected_has_alpha,
               package.result_sha256,
               package.candidate_id,
               package.result_received_at,
               package.cleanup_requested_at,
               package.payload_deleted_at,
               package.extended_at,
               package.created_at,
               package.expires_at,
               julianday(package.expires_at) <= julianday('now', ?2) AS is_expired
             FROM ai_web_handoff_packages package
             JOIN ai_requests request ON request.id = package.request_id
             WHERE package.request_id = ?1
               AND package.handoff_kind = 'static_icon_sheet'
               AND package.layout_mode = 'single'
               AND package.operation = 'edit'",
            params![request_id, cutoff_modifier],
            |row| {
                let collection_id = row.get::<_, Option<String>>(1)?;
                let icon_id = row.get::<_, Option<String>>(2)?;
                Ok((
                    row.get::<_, String>(0)?,
                    collection_id,
                    icon_id,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, String>(10)?,
                    row.get::<_, i64>(11)?,
                    row.get::<_, String>(12)?,
                    row.get::<_, String>(13)?,
                    row.get::<_, String>(14)?,
                    row.get::<_, i64>(15)?,
                    row.get::<_, i64>(16)?,
                    row.get::<_, i64>(17)? != 0,
                    row.get::<_, Option<String>>(18)?,
                    row.get::<_, Option<String>>(19)?,
                    row.get::<_, Option<String>>(20)?,
                    row.get::<_, Option<String>>(21)?,
                    row.get::<_, Option<String>>(22)?,
                    row.get::<_, Option<String>>(23)?,
                    row.get::<_, String>(24)?,
                    row.get::<_, String>(25)?,
                    row.get::<_, i64>(26)? != 0,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("해당 AI 웹 전달 세션을 찾을 수 없습니다."))
        .and_then(
            |(
                request_id,
                collection_id,
                icon_id,
                service_surface,
                request_status,
                capability_snapshot_json,
                original_lineage_id,
                original_lineage_generation,
                original_source_sha256,
                effective_source_sha256,
                request_recipe_signature,
                activation_revision,
                upload_sha256,
                manifest_sha256,
                prompt_sha256,
                expected_width,
                expected_height,
                expected_has_alpha,
                result_sha256,
                candidate_id,
                result_received_at,
                cleanup_requested_at,
                payload_deleted_at,
                extended_at,
                created_at,
                expires_at,
                is_expired,
            )| {
                let collection_id = collection_id.ok_or_else(|| {
                    AppError::new(
                        "ai_handoff_origin_missing",
                        "원본 컬렉션이 삭제되어 이 전달 세션을 사용할 수 없습니다.",
                    )
                })?;
                let icon_id = icon_id.ok_or_else(|| {
                    AppError::new(
                        "ai_handoff_origin_missing",
                        "원본 아이콘이 삭제되어 이 전달 세션을 사용할 수 없습니다.",
                    )
                })?;
                Ok(HandoffRecord {
                    request_id,
                    collection_id,
                    icon_id,
                    service_surface,
                    request_status,
                    capability_snapshot_json,
                    original_lineage_id,
                    original_lineage_generation,
                    original_source_sha256,
                    effective_source_sha256,
                    request_recipe_signature,
                    activation_revision,
                    upload_sha256,
                    manifest_sha256,
                    prompt_sha256,
                    expected_width,
                    expected_height,
                    expected_has_alpha,
                    result_sha256,
                    candidate_id,
                    result_received_at,
                    cleanup_requested_at,
                    payload_deleted_at,
                    extended_at,
                    created_at,
                    expires_at,
                    is_expired,
                })
            },
        )
}

fn require_live_payload(
    connection: &Connection,
    paths: &AppPaths,
    record: &HandoffRecord,
) -> AppResult<()> {
    if record.is_expired {
        expire_handoff_payload(connection, paths, record);
        return Err(AppError::new(
            "ai_handoff_expired",
            "웹 전달 세션이 만료됐습니다. 새 전달 패키지를 만들어 주세요.",
        ));
    }
    if record.cleanup_requested_at.is_some() || record.payload_deleted_at.is_some() {
        return Err(AppError::new(
            "ai_handoff_payload_deleted",
            "이 웹 전달 패키지는 닫혔거나 임시 파일 정리 중입니다.",
        ));
    }
    if record.request_status != "awaiting_result" {
        return Err(AppError::new(
            "ai_handoff_closed",
            "이 웹 전달 세션은 더 이상 결과를 받을 수 없습니다.",
        ));
    }
    Ok(())
}

fn require_current_handoff_payload(
    connection: &Connection,
    paths: &AppPaths,
    record: &HandoffRecord,
) -> AppResult<()> {
    if request_is_current(connection, record)? {
        return Ok(());
    }
    close_handoff_payload(connection, paths, record, "cancelled");
    Err(stale_error())
}

fn close_handoff_payload(
    connection: &Connection,
    paths: &AppPaths,
    record: &HandoffRecord,
    terminal_status: &str,
) {
    if request_payload_cleanup(connection, &record.request_id, Some(terminal_status)).is_err() {
        return;
    }
    let package_dir = paths.ai_handoffs_dir.join(&record.request_id);
    if ai_managed_artifacts::remove_owned_directory_if_present(
        &paths.root,
        &paths.ai_handoffs_dir,
        &package_dir,
    )
    .is_ok()
    {
        let _ = finish_payload_deletion(connection, &record.request_id);
    }
}

fn expire_handoff_payload(connection: &Connection, paths: &AppPaths, record: &HandoffRecord) {
    close_handoff_payload(connection, paths, record, "expired");
}
fn session_from_record(
    paths: &AppPaths,
    record: &HandoffRecord,
) -> AppResult<AiWebHandoffSessionDto> {
    let prompt = verified_package_file(
        paths,
        &record.request_id,
        PROMPT_FILE_NAME,
        &record.prompt_sha256,
        MAX_FINAL_PROMPT_BYTES,
    )?;
    let final_prompt = String::from_utf8(prompt.bytes).map_err(|_| {
        AppError::new(
            "ai_handoff_payload_corrupt",
            "웹 전달 프롬프트가 UTF-8이 아닙니다.",
        )
    })?;
    verified_package_file(
        paths,
        &record.request_id,
        MANIFEST_FILE_NAME,
        &record.manifest_sha256,
        MAX_AI_HANDOFF_BYTES,
    )?;
    let upload = verified_package_file(
        paths,
        &record.request_id,
        UPLOAD_FILE_NAME,
        &record.upload_sha256,
        MAX_AI_HANDOFF_BYTES,
    )?;
    Ok(AiWebHandoffSessionDto {
        request_id: record.request_id.clone(),
        kind: HANDOFF_KIND.to_string(),
        layout_mode: LAYOUT_MODE.to_string(),
        operation: OPERATION.to_string(),
        service_surface: record.service_surface.clone(),
        final_prompt,
        upload_file_name: UPLOAD_FILE_NAME.to_string(),
        upload_preview_path: upload.path.to_string_lossy().to_string(),
        expected_width: record.expected_width,
        expected_height: record.expected_height,
        expected_has_alpha: record.expected_has_alpha,
        created_at: record.created_at.clone(),
        expires_at: record.expires_at.clone(),
        can_extend: record.extended_at.is_none() && record.cleanup_requested_at.is_none(),
        native_drag_supported: crate::native_drag::NATIVE_FILE_DRAG_SUPPORTED,
        warnings: vec![
            "웹 화면의 실제 첨부 완료 여부는 PMTCONCON Studio가 확인할 수 없습니다.".to_string(),
            "결과를 내려받은 뒤 PMTCONCON Studio의 결과 놓기 영역에 끌어 놓아 검사하세요."
                .to_string(),
        ],
    })
}
fn ensure_package_integrity(paths: &AppPaths, record: &HandoffRecord) -> AppResult<()> {
    verified_package_file(
        paths,
        &record.request_id,
        UPLOAD_FILE_NAME,
        &record.upload_sha256,
        MAX_AI_HANDOFF_BYTES,
    )?;
    verified_package_file(
        paths,
        &record.request_id,
        MANIFEST_FILE_NAME,
        &record.manifest_sha256,
        64 * 1024,
    )?;
    verified_package_file(
        paths,
        &record.request_id,
        PROMPT_FILE_NAME,
        &record.prompt_sha256,
        MAX_FINAL_PROMPT_BYTES,
    )?;
    Ok(())
}

fn verified_package_file(
    paths: &AppPaths,
    request_id: &str,
    file_name: &str,
    expected_sha256: &str,
    max_bytes: usize,
) -> AppResult<VerifiedPackageFile> {
    validate_request_id(request_id)?;
    let package_dir = validate_existing_owned_directory(
        &paths.root,
        &paths.ai_handoffs_dir,
        &paths.ai_handoffs_dir.join(request_id),
    )?;
    let path = package_dir.join(file_name);
    let bytes = read_regular_file_no_follow(&paths.root, &path, max_bytes)?;
    if sha256_hex(&bytes) != expected_sha256 {
        return Err(AppError::new(
            "ai_handoff_payload_corrupt",
            "웹 전달 패키지 파일의 무결성 검사가 실패했습니다. 새 패키지를 만들어 주세요.",
        ));
    }
    Ok(VerifiedPackageFile { path, bytes })
}
fn validate_managed_source_path(paths: &AppPaths, stored_path: &str) -> AppResult<PathBuf> {
    let path = PathBuf::from(stored_path);
    validate_existing_managed_file(&paths.root, &path)?;
    Ok(path)
}

fn validate_existing_managed_file(app_root: &Path, path: &Path) -> AppResult<()> {
    let canonical_root = app_root.canonicalize()?;
    let canonical_path = path.canonicalize()?;
    let canonical_relative = canonical_path
        .strip_prefix(&canonical_root)
        .map_err(|_| managed_path_error())?;
    validate_relative_components(canonical_relative)?;

    if let Ok(raw_relative) = path.strip_prefix(app_root) {
        validate_relative_components(raw_relative)?;
        let mut raw_current = app_root.to_path_buf();
        for component in raw_relative.components() {
            let Component::Normal(component) = component else {
                return Err(managed_path_error());
            };
            raw_current.push(component);
            if is_link_or_reparse_point(&fs::symlink_metadata(&raw_current)?) {
                return Err(managed_path_error());
            }
        }
    }

    let component_count = canonical_relative.components().count();
    let mut current = canonical_root.clone();
    for (index, component) in canonical_relative.components().enumerate() {
        let Component::Normal(component) = component else {
            return Err(managed_path_error());
        };
        current.push(component);
        let metadata = fs::symlink_metadata(&current)?;
        if is_link_or_reparse_point(&metadata) {
            return Err(managed_path_error());
        }
        let is_last = index + 1 == component_count;
        if (is_last && !metadata.file_type().is_file())
            || (!is_last && !metadata.file_type().is_dir())
        {
            return Err(managed_path_error());
        }
    }
    Ok(())
}
fn validate_existing_owned_directory(
    app_root: &Path,
    allowed_root: &Path,
    target: &Path,
) -> AppResult<PathBuf> {
    let allowed_relative = allowed_root
        .strip_prefix(app_root)
        .map_err(|_| managed_path_error())?;
    let target_relative = target
        .strip_prefix(app_root)
        .map_err(|_| managed_path_error())?;
    let target_relative_to_allowed = target
        .strip_prefix(allowed_root)
        .map_err(|_| managed_path_error())?;
    validate_relative_components(allowed_relative)?;
    validate_relative_components(target_relative)?;
    if !target.starts_with(allowed_root) {
        return Err(managed_path_error());
    }
    let canonical_app_root = app_root.canonicalize()?;
    let canonical_allowed = allowed_root.canonicalize()?;
    if !canonical_allowed.starts_with(&canonical_app_root) {
        return Err(managed_path_error());
    }
    let mut current = app_root.to_path_buf();
    for component in target_relative.components() {
        let Component::Normal(component) = component else {
            return Err(managed_path_error());
        };
        current.push(component);
        let metadata = fs::symlink_metadata(&current)?;
        if !metadata.file_type().is_dir() || is_link_or_reparse_point(&metadata) {
            return Err(managed_path_error());
        }
    }
    let canonical = target.canonicalize()?;
    if canonical != canonical_allowed.join(target_relative_to_allowed) {
        return Err(managed_path_error());
    }
    Ok(canonical)
}

fn read_regular_file_no_follow(
    app_root: &Path,
    path: &Path,
    max_bytes: usize,
) -> AppResult<Vec<u8>> {
    validate_existing_managed_file(app_root, path)?;
    let metadata = fs::symlink_metadata(path)?;
    let size = usize::try_from(metadata.len()).map_err(|_| managed_path_error())?;
    if size > max_bytes {
        return Err(AppError::new(
            "ai_handoff_payload_corrupt",
            "웹 전달 패키지 파일 크기가 허용 범위를 넘었습니다.",
        ));
    }
    let mut bytes = Vec::with_capacity(size);
    File::open(path)?
        .take(max_bytes.saturating_add(1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > max_bytes {
        return Err(AppError::new(
            "ai_handoff_payload_corrupt",
            "웹 전달 패키지 파일 크기가 허용 범위를 넘었습니다.",
        ));
    }
    Ok(bytes)
}

fn validate_relative_components(path: &Path) -> AppResult<()> {
    if path
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
    {
        Ok(())
    } else {
        Err(managed_path_error())
    }
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
        "ai_handoff_managed_path",
        "웹 전달 파일 경로가 안전한 앱 관리 경로가 아닙니다.",
    )
}

fn validate_request_id(request_id: &str) -> AppResult<()> {
    if request_id.starts_with("ai_request_")
        && request_id.len() <= 96
        && request_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        Ok(())
    } else {
        Err(AppError::new(
            "ai_handoff_request_id",
            "웹 전달 요청 ID 형식이 올바르지 않습니다.",
        ))
    }
}

fn request_is_current(connection: &Connection, record: &HandoffRecord) -> AppResult<bool> {
    let current = match ai_repository::resolve_effective_visual_source(
        connection,
        &record.collection_id,
        &record.icon_id,
    ) {
        Ok(current) => current,
        Err(_) => return Ok(false),
    };
    if current.original_lineage_id != record.original_lineage_id
        || current.original_lineage_generation != record.original_lineage_generation
        || current.original_source.sha256 != record.original_source_sha256
        || current.render_source.sha256 != record.effective_source_sha256
        || current.activation_revision != record.activation_revision
    {
        return Ok(false);
    }
    let review_state =
        ai_repository::get_ai_review_state(connection, &record.collection_id, &record.icon_id)?;
    Ok(review_state.native_recipe_signature == record.request_recipe_signature)
}

fn inspect_result(
    record: &HandoffRecord,
    file: &ImportImageFilePayload,
    is_current: bool,
) -> AppResult<InspectedResult> {
    let sha256 = sha256_hex(&file.bytes);
    let mut dto = AiWebHandoffResultInspectionDto {
        accepted: false,
        issues: Vec::new(),
        validation_signature: None,
        expected_width: record.expected_width,
        expected_height: record.expected_height,
        expected_has_alpha: record.expected_has_alpha,
        actual_width: None,
        actual_height: None,
        actual_has_alpha: None,
        review_state: None,
    };
    if !is_current {
        push_blocking_issue(
            &mut dto,
            stale_issue(record.expected_width, record.expected_height),
        );
    }
    if file.bytes.len() > MAX_AI_HANDOFF_BYTES {
        push_blocking_issue(
            &mut dto,
            AiWebHandoffIssueDto {
                code: "ai_handoff_result_too_large".to_string(),
                severity: "blocking".to_string(),
                message: "결과 파일이 16MB를 넘습니다.".to_string(),
                expected: Some("16MB 이하".to_string()),
                actual: Some(format!("{} bytes", file.bytes.len())),
                suggested_prompt: Some(
                    "Return one optimized PNG under 16 MB without changing the canvas size."
                        .to_string(),
                ),
                local_action: Some(
                    "웹에서 PNG로 다시 내려받거나 파일 크기를 줄이세요.".to_string(),
                ),
            },
        );
        return Ok(InspectedResult {
            dto,
            sha256,
            extension: None,
        });
    }
    let format = match image::guess_format(&file.bytes) {
        Ok(ImageFormat::Gif) => {
            push_blocking_issue(
                &mut dto,
                AiWebHandoffIssueDto {
                    code: "ai_handoff_result_animated".to_string(),
                    severity: "blocking".to_string(),
                    message: "이번 전달 세션은 정적 결과만 받을 수 있지만 GIF가 들어왔습니다."
                        .to_string(),
                    expected: Some("정적 PNG 또는 JPG".to_string()),
                    actual: Some("GIF".to_string()),
                    suggested_prompt: Some("Return exactly one static PNG image.".to_string()),
                    local_action: Some("정적 PNG/JPG 결과를 내려받아 다시 놓으세요.".to_string()),
                },
            );
            return Ok(InspectedResult {
                dto,
                sha256,
                extension: None,
            });
        }
        Ok(ImageFormat::Png) => Some((ImageFormat::Png, "png")),
        Ok(ImageFormat::Jpeg) => Some((ImageFormat::Jpeg, "jpg")),
        Ok(_) => {
            push_blocking_issue(
                &mut dto,
                AiWebHandoffIssueDto {
                    code: "ai_handoff_result_format".to_string(),
                    severity: "blocking".to_string(),
                    message: "결과는 PNG 또는 JPG 파일이어야 합니다.".to_string(),
                    expected: Some("PNG 또는 JPG".to_string()),
                    actual: Some("지원하지 않는 이미지 형식".to_string()),
                    suggested_prompt: Some("Return exactly one PNG image only.".to_string()),
                    local_action: Some("웹 결과를 PNG/JPG로 다시 내려받으세요.".to_string()),
                },
            );
            return Ok(InspectedResult {
                dto,
                sha256,
                extension: None,
            });
        }
        Err(_) => {
            push_blocking_issue(
                &mut dto,
                AiWebHandoffIssueDto {
                    code: "ai_handoff_result_corrupt".to_string(),
                    severity: "blocking".to_string(),
                    message: "결과 파일을 이미지로 읽을 수 없습니다.".to_string(),
                    expected: Some("정상적인 PNG 또는 JPG".to_string()),
                    actual: Some("손상되었거나 이미지가 아닌 파일".to_string()),
                    suggested_prompt: None,
                    local_action: Some("웹에서 이미지를 다시 내려받아 놓으세요.".to_string()),
                },
            );
            return Ok(InspectedResult {
                dto,
                sha256,
                extension: None,
            });
        }
    };
    let (format, extension) = format.expect("format is present after early returns");
    let image = match decode_import_image(&file.bytes, format) {
        Ok(image) => image,
        Err(_) => {
            push_blocking_issue(
                &mut dto,
                AiWebHandoffIssueDto {
                    code: "ai_handoff_result_corrupt".to_string(),
                    severity: "blocking".to_string(),
                    message: "결과 이미지가 손상됐거나 안전한 크기 제한을 넘었습니다.".to_string(),
                    expected: Some("최대 12,000px, 3,200만 픽셀 이하".to_string()),
                    actual: None,
                    suggested_prompt: None,
                    local_action: Some(
                        "웹에서 이미지를 다시 내려받거나 크기를 줄이세요.".to_string(),
                    ),
                },
            );
            return Ok(InspectedResult {
                dto,
                sha256,
                extension: Some(extension),
            });
        }
    };
    let actual_width = i64::from(image.width());
    let actual_height = i64::from(image.height());
    let actual_has_alpha =
        extension == "png" && image.to_rgba8().pixels().any(|pixel| pixel[3] < 255);
    dto.actual_width = Some(actual_width);
    dto.actual_height = Some(actual_height);
    dto.actual_has_alpha = Some(actual_has_alpha);
    if actual_width != record.expected_width || actual_height != record.expected_height {
        push_blocking_issue(
            &mut dto,
            AiWebHandoffIssueDto {
                code: "ai_handoff_result_dimensions".to_string(),
                severity: "blocking".to_string(),
                message: "결과 이미지의 캔버스 크기가 전달 패키지와 다릅니다.".to_string(),
                expected: Some(format!(
                    "{}×{}px",
                    record.expected_width, record.expected_height
                )),
                actual: Some(format!("{actual_width}×{actual_height}px")),
                suggested_prompt: Some(format!(
                    "Keep the canvas exactly {}×{}px. Do not crop, resize, or add margins.",
                    record.expected_width, record.expected_height
                )),
                local_action: Some("제안 프롬프트로 웹에서 다시 요청하세요.".to_string()),
            },
        );
    }
    if record.expected_has_alpha && !actual_has_alpha {
        push_blocking_issue(
            &mut dto,
            AiWebHandoffIssueDto {
                code: "ai_handoff_result_alpha_lost".to_string(),
                severity: "blocking".to_string(),
                message: "원본에 있던 투명 영역이 결과에서 사라졌습니다.".to_string(),
                expected: Some("투명 배경 유지".to_string()),
                actual: Some("투명 픽셀 없음".to_string()),
                suggested_prompt: Some(
                    "Preserve the transparent background and all transparent regions. Return a PNG with alpha."
                        .to_string(),
                ),
                local_action: Some("제안 프롬프트로 투명 PNG를 다시 요청하세요.".to_string()),
            },
        );
    }
    let has_blocker = dto.issues.iter().any(|issue| issue.severity == "blocking");
    if !has_blocker {
        dto.accepted = true;
        dto.validation_signature = Some(hash_text(&[
            "pmtcon-ai-web-handoff-result-v1".to_string(),
            record.request_id.clone(),
            record.upload_sha256.clone(),
            sha256.clone(),
            record.expected_width.to_string(),
            record.expected_height.to_string(),
            record.expected_has_alpha.to_string(),
            record.original_lineage_id.clone(),
            record.original_lineage_generation.to_string(),
            record.effective_source_sha256.clone(),
            record.request_recipe_signature.clone(),
            record.activation_revision.to_string(),
        ]));
        dto.issues.push(AiWebHandoffIssueDto {
            code: "ai_handoff_result_manual_review".to_string(),
            severity: "manual_review".to_string(),
            message: "파일 구조 검사는 통과했습니다. 캐릭터 정체성, 의도한 그림체와 세부 내용은 직접 확인해 주세요."
                .to_string(),
            expected: None,
            actual: None,
            suggested_prompt: None,
            local_action: Some("미리보기를 확인한 뒤 후보로 등록하세요.".to_string()),
        });
    }
    Ok(InspectedResult {
        dto,
        sha256,
        extension: Some(extension),
    })
}

fn push_blocking_issue(
    inspection: &mut AiWebHandoffResultInspectionDto,
    issue: AiWebHandoffIssueDto,
) {
    inspection.accepted = false;
    inspection.validation_signature = None;
    inspection.issues.push(issue);
}

fn stale_issue(expected_width: i64, expected_height: i64) -> AiWebHandoffIssueDto {
    AiWebHandoffIssueDto {
        code: "ai_handoff_result_stale".to_string(),
        severity: "blocking".to_string(),
        message: "전달 패키지를 만든 뒤 아이콘 소스나 편집 설정이 변경됐습니다.".to_string(),
        expected: Some(format!(
            "{expected_width}×{expected_height}px 패키지 생성 시점 상태"
        )),
        actual: Some("현재 편집 상태가 다름".to_string()),
        suggested_prompt: None,
        local_action: Some("현재 상태로 새 웹 전달 패키지를 만드세요.".to_string()),
    }
}

fn stale_error() -> AppError {
    AppError::new(
        "ai_handoff_stale",
        "웹 전달을 준비하는 동안 아이콘 소스나 편집 설정이 변경됐습니다. 다시 시도해 주세요.",
    )
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn checked_payload_bytes(lengths: &[usize]) -> AppResult<u64> {
    lengths.iter().try_fold(0_u64, |total, length| {
        let length = u64::try_from(*length).map_err(|_| storage_size_error())?;
        total.checked_add(length).ok_or_else(storage_size_error)
    })
}

fn ensure_ai_web_handoff_quota(
    connection: &Connection,
    paths: &AppPaths,
    planned_package_bytes: u64,
    quota_bytes: u64,
) -> AppResult<()> {
    let used_bytes = managed_ai_transfer_storage_bytes(connection, paths)?;
    let fits = used_bytes
        .checked_add(planned_package_bytes)
        .is_some_and(|total| total <= quota_bytes);
    if fits {
        return Ok(());
    }
    Err(AppError::new(
        "ai_handoff_payload_quota_exceeded",
        format!(
            "AI \u{C6F9} \u{C804}\u{B2EC} \u{C784}\u{C2DC} \u{C800}\u{C7A5}\u{C18C} \u{C6A9}\u{B7C9}\u{C774} \u{BD80}\u{C871}\u{D569}\u{B2C8}\u{B2E4}. \u{C644}\u{B8CC}\u{B418}\u{AC70}\u{B098} \u{B9CC}\u{B8CC}\u{B41C} \u{C804}\u{B2EC} \u{D30C}\u{C77C}\u{C744} \u{C815}\u{B9AC}\u{D55C} \u{B4A4} \u{B2E4}\u{C2DC} \u{C2DC}\u{B3C4}\u{D574} \u{C8FC}\u{C138}\u{C694}. (\u{D604}\u{C7AC} {used_bytes}B, \u{C0C8} \u{D328}\u{D0A4}\u{C9C0} {planned_package_bytes}B, \u{D55C}\u{B3C4} {quota_bytes}B)"
        ),
    ))
}

fn managed_ai_transfer_storage_bytes(connection: &Connection, paths: &AppPaths) -> AppResult<u64> {
    let handoffs_root = validate_existing_owned_directory(
        &paths.root,
        &paths.ai_handoffs_dir,
        &paths.ai_handoffs_dir,
    )?;
    let handoff_bytes = managed_directory_bytes(&handoffs_root, 0)?;
    let grid_bytes = ai_grid_retention::managed_ai_grid_payload_bytes(connection, paths)?;
    handoff_bytes
        .checked_add(grid_bytes)
        .ok_or_else(storage_size_error)
}

fn managed_directory_bytes(path: &Path, depth: u8) -> AppResult<u64> {
    if depth > 8 {
        return Err(storage_size_error());
    }
    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error.into()),
    };
    let mut total = 0_u64;
    for entry in entries {
        let entry = entry?;
        let metadata = match fs::symlink_metadata(entry.path()) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        };
        if is_link_or_reparse_point(&metadata) {
            return Err(managed_path_error());
        }
        let entry_bytes = if metadata.file_type().is_file() {
            metadata.len()
        } else if metadata.file_type().is_dir() {
            managed_directory_bytes(&entry.path(), depth.saturating_add(1))?
        } else {
            return Err(managed_path_error());
        };
        total = total
            .checked_add(entry_bytes)
            .ok_or_else(storage_size_error)?;
    }
    Ok(total)
}

fn checked_count_sum(left: i64, right: i64) -> AppResult<u64> {
    nonnegative_count(left)?
        .checked_add(nonnegative_count(right)?)
        .ok_or_else(storage_size_error)
}
fn nonnegative_count(value: i64) -> AppResult<u64> {
    u64::try_from(value).map_err(|_| storage_size_error())
}

fn storage_size_error() -> AppError {
    AppError::new(
        "ai_handoff_storage_size",
        "AI \u{C6F9} \u{C804}\u{B2EC} \u{C784}\u{C2DC} \u{C800}\u{C7A5}\u{C18C} \u{D06C}\u{AE30}\u{B97C} \u{ACC4}\u{C0B0}\u{D560} \u{C218} \u{C5C6}\u{C2B5}\u{B2C8}\u{B2E4}.",
    )
}

fn cleanup_orphan_directories(
    connection: &Connection,
    paths: &AppPaths,
) -> AppResult<AiWebHandoffCleanupReport> {
    let now = SystemTime::now();
    let mut report = AiWebHandoffCleanupReport::default();
    let handoffs_root = validate_existing_owned_directory(
        &paths.root,
        &paths.ai_handoffs_dir,
        &paths.ai_handoffs_dir,
    )?;
    let staging_root = handoffs_root.join(".staging");
    if fs::symlink_metadata(&staging_root)
        .ok()
        .is_some_and(|metadata| {
            metadata.file_type().is_dir() && !is_link_or_reparse_point(&metadata)
        })
        && validate_existing_owned_directory(&paths.root, &paths.ai_handoffs_dir, &staging_root)
            .is_ok()
    {
        for entry in fs::read_dir(&staging_root)? {
            let entry = entry?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            if validate_request_id(name).is_err()
                || !is_regular_directory_older_than(&entry.path(), now, ORPHAN_GRACE_PERIOD)
            {
                continue;
            }
            if ai_managed_artifacts::remove_owned_directory_if_present(
                &paths.root,
                &staging_root,
                &entry.path(),
            )
            .is_ok()
            {
                report.removed += 1;
            } else {
                report.deferred += 1;
            }
        }
    }
    for entry in fs::read_dir(&handoffs_root)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if name == ".staging"
            || validate_request_id(name).is_err()
            || !is_regular_directory_older_than(&entry.path(), now, ORPHAN_GRACE_PERIOD)
        {
            continue;
        }
        let referenced = connection.query_row(
            "SELECT EXISTS(
               SELECT 1 FROM ai_web_handoff_packages WHERE request_id = ?1
             )",
            [name],
            |row| row.get::<_, i64>(0),
        )? != 0;
        if referenced {
            continue;
        }
        if ai_managed_artifacts::remove_owned_directory_if_present(
            &paths.root,
            &paths.ai_handoffs_dir,
            &entry.path(),
        )
        .is_ok()
        {
            report.removed += 1;
        } else {
            report.deferred += 1;
        }
    }
    Ok(report)
}

fn is_regular_directory_older_than(path: &Path, now: SystemTime, age: Duration) -> bool {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return false;
    };
    if !metadata.file_type().is_dir() || is_link_or_reparse_point(&metadata) {
        return false;
    }
    metadata
        .modified()
        .ok()
        .and_then(|modified| now.duration_since(modified).ok())
        .is_some_and(|elapsed| elapsed >= age)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png_bytes(width: u32, height: u32, alpha: u8) -> Vec<u8> {
        let image = DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            width,
            height,
            image::Rgba([10, 20, 30, alpha]),
        ));
        encode_png(&image).unwrap()
    }

    fn record(width: i64, height: i64, expected_has_alpha: bool) -> HandoffRecord {
        HandoffRecord {
            request_id: "ai_request_00000000000000000000000000000001_00000001".to_string(),
            collection_id: "collection_test".to_string(),
            icon_id: "icon_test".to_string(),
            service_surface: "gemini_web".to_string(),
            request_status: "awaiting_result".to_string(),
            capability_snapshot_json: "{}".to_string(),
            original_lineage_id: "lineage_test".to_string(),
            original_lineage_generation: 0,
            original_source_sha256: "a".repeat(64),
            effective_source_sha256: "b".repeat(64),
            request_recipe_signature: "recipe_test".to_string(),
            activation_revision: 0,
            upload_sha256: "c".repeat(64),
            manifest_sha256: "d".repeat(64),
            prompt_sha256: "e".repeat(64),
            expected_width: width,
            expected_height: height,
            expected_has_alpha,
            result_sha256: None,
            candidate_id: None,
            result_received_at: None,
            cleanup_requested_at: None,
            payload_deleted_at: None,
            extended_at: None,
            created_at: "2026-07-28T00:00:00.000Z".to_string(),
            expires_at: "2026-08-04T00:00:00.000Z".to_string(),
            is_expired: false,
        }
    }

    #[test]
    fn static_single_prompt_is_deterministic_and_never_describes_a_grid() {
        let first = build_static_single_prompt(200, 200, true, "표정을 더 밝게").unwrap();
        let second = build_static_single_prompt(200, 200, true, "표정을 더 밝게").unwrap();

        assert_eq!(first, second);
        assert!(first.contains("exactly 200×200px"));
        assert!(first.contains("스프라이트 시트가 아닌 이미지 한 장"));
        assert!(first.contains("transparent background"));
        assert!(first.len() <= MAX_FINAL_PROMPT_BYTES);
    }

    #[test]
    fn result_inspection_blocks_wrong_geometry_and_lost_alpha() {
        let file = ImportImageFilePayload {
            original_filename: "result.png".to_string(),
            bytes: png_bytes(128, 64, 255),
        };
        let inspected = inspect_result(&record(64, 64, true), &file, true).unwrap();

        assert!(!inspected.dto.accepted);
        assert!(inspected.dto.validation_signature.is_none());
        assert!(inspected
            .dto
            .issues
            .iter()
            .any(|issue| issue.code == "ai_handoff_result_dimensions"));
        assert!(inspected
            .dto
            .issues
            .iter()
            .any(|issue| issue.code == "ai_handoff_result_alpha_lost"));
    }

    #[test]
    fn valid_static_result_gets_signature_but_requires_manual_content_review() {
        let file = ImportImageFilePayload {
            original_filename: "download-without-trusted-extension.bin".to_string(),
            bytes: png_bytes(64, 64, 0),
        };
        let inspected = inspect_result(&record(64, 64, true), &file, true).unwrap();

        assert!(inspected.dto.accepted);
        assert!(inspected.dto.validation_signature.is_some());
        assert_eq!(inspected.extension, Some("png"));
        assert!(inspected
            .dto
            .issues
            .iter()
            .any(|issue| issue.severity == "manual_review"));
    }

    #[test]
    fn request_id_rejects_path_components() {
        assert!(
            validate_request_id("ai_request_00000000000000000000000000000001_00000001").is_ok()
        );
        assert!(validate_request_id("../ai_request_escape").is_err());
        assert!(validate_request_id("ai_request_..\\escape").is_err());
    }

    #[test]
    fn prompt_rejects_secret_like_material() {
        assert!(validate_user_prompt("표정을 부드럽게").is_ok());
        assert!(validate_user_prompt("Authorization: Bearer secret").is_err());
    }

    #[test]
    fn import_byte_limit_is_not_weaker_than_handoff_limit() {
        assert!(MAX_AI_HANDOFF_BYTES <= MAX_IMPORT_FILE_BYTES);
    }
}
