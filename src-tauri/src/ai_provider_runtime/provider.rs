use std::fs::File;
use std::io::{Cursor, Read};
use std::path::PathBuf;

use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use image::ImageFormat;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::ai_provider::transport::{
    AiImageTransport, ProviderAuthorization, ReqwestTransport, TransportFailure, TransportResponse,
};
use crate::db::repositories::ai::{self as ai_repository, EffectiveVisualSource};
use crate::db::repositories::ai_snapshots;
use crate::db::repositories::source_files::{
    commit_prepared_source_file, prepare_source_file_from_bytes, SourceFileImportOptions,
};
use crate::error::{AppError, AppResult};
use crate::ids::create_id;
use crate::models::{AiReviewStateDto, ExecuteAiImageEditPayload, ImportImageFilePayload};
use crate::optimization::cache::hash_text;
use crate::paths::AppPaths;

pub(crate) const NOVELAI_ENDPOINT: &str = "https://image.novelai.net:443/ai/generate-image";
pub(crate) const GEMINI_ENDPOINT: &str =
    "https://generativelanguage.googleapis.com/v1beta/interactions";

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_RESPONSE_BYTES: usize = 24 * 1024 * 1024;
const MAX_PROMPT_BYTES: usize = 4096;
const MAX_MODEL_BYTES: usize = 128;
const MAX_ACTION_BYTES: usize = 64;

const GEMINI_MODELS: [&str; 2] = ["gemini-2.5-flash-image", "gemini-3.1-flash-image"];

#[derive(Clone)]
pub(crate) struct PreparedImageEdit {
    pub request_id: String,
    pub collection_id: String,
    pub payload: ExecuteAiImageEditPayload,
    pub original_lineage_id: String,
    pub original_lineage_generation: i64,
    pub original_source_sha256: String,
    pub effective_source_sha256: String,
    pub activation_revision: i64,
    pub request_recipe_signature: String,
    pub input_mime_type: String,
    pub input_bytes: Vec<u8>,
    snapshots: RequestSnapshots,
}

#[derive(Debug)]
pub(crate) struct ProviderImage {
    pub bytes: Vec<u8>,
    pub original_filename: String,
    pub provider_usage: Option<Value>,
    pub provider_request_id: Option<String>,
}

#[derive(Debug, Clone)]
struct ProviderDescriptor {
    provider: &'static str,
    service_surface: &'static str,
    adapter_id: &'static str,
    adapter_contract_version: &'static str,
    policy_refs: Value,
}

#[derive(Clone)]
struct RequestSnapshots {
    capability: String,
    data_tier: String,
    retention: String,
    consent: String,
    policy_refs: String,
    prompt_options: String,
}

pub(crate) fn start_image_edit(
    connection: &mut Connection,
    paths: &AppPaths,
    collection_id: &str,
    payload: ExecuteAiImageEditPayload,
) -> AppResult<PreparedImageEdit> {
    validate_payload(&payload)?;
    let descriptor = descriptor_for(&payload.provider)?;
    let current = ai_repository::resolve_effective_visual_source(
        connection,
        collection_id,
        &payload.icon_id,
    )?;
    validate_static_source(&current)?;
    let state = ai_repository::get_ai_review_state(connection, collection_id, &payload.icon_id)?;
    let input_path = managed_input_path(paths, &current.render_source.path)?;
    let mut input_bytes = Vec::with_capacity(MAX_INPUT_BYTES.min(1024 * 1024));
    File::open(&input_path)?
        .take(MAX_INPUT_BYTES as u64 + 1)
        .read_to_end(&mut input_bytes)?;
    if input_bytes.len() > MAX_INPUT_BYTES {
        return Err(AppError::new(
            "ai_input_too_large",
            "AI 입력 파일은 최대 16MB까지 전송할 수 있습니다.",
        ));
    }
    let effective_source_sha256 = format!("{:x}", Sha256::digest(&input_bytes));
    if !effective_source_sha256.eq_ignore_ascii_case(&current.render_source.sha256) {
        return Err(AppError::new(
            "ai_input_changed",
            "저장된 현재 소스와 전송할 파일의 SHA-256이 달라 요청을 중단했습니다.",
        ));
    }

    let (input_mime_type, input_bytes) = prepare_provider_input(
        &payload.provider,
        &current.render_source.mime_type,
        input_bytes,
    )?;
    let input_package_sha256 = format!("{:x}", Sha256::digest(&input_bytes));
    let snapshots = build_request_snapshots(&descriptor, &payload)?;
    let payload_input_signature = hash_text(&[
        "pmtcon-provider-image-edit-v1".to_string(),
        descriptor.provider.to_string(),
        payload.model.clone(),
        payload.prompt.clone(),
        ai_snapshots::canonical_value(&json!(payload.options)),
        current.original_lineage_id.clone(),
        current.original_lineage_generation.to_string(),
        current.render_source.sha256.clone(),
        input_package_sha256.clone(),
    ]);
    let request_id = create_id("ai_request");

    let transaction =
        connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let transaction_state =
        ai_repository::get_ai_review_state(&transaction, collection_id, &payload.icon_id)?;
    if transaction_state.native_recipe_signature != state.native_recipe_signature {
        return Err(AppError::new(
            "ai_request_stale",
            "요청 준비 중 편집 상태가 변경되어 AI 요청을 시작하지 않았습니다.",
        ));
    }
    let inserted = transaction.execute(
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
           activation_revision, status, started_at, created_at, updated_at
         )
         SELECT
           ?1, c.id, i.id, c.name, i.display_name,
           'api', ?2, ?3, ?4, ?5, 'unknown', ?6, 'static_image_edit',
           'api_verified', 'session', ?7, ?8, ?9, ?10, ?11, ?12, ?13,
           ?14, ?15, ?16, ?17, ?18, ?19, ?20, 'running',
           strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
           strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
           strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         FROM icons i
         JOIN collections c ON c.id = i.collection_id
         JOIN effective_visual_sources ev ON ev.icon_id = i.id
         WHERE i.id = ?21
           AND i.collection_id = ?22
           AND i.deleted_at IS NULL
           AND c.deleted_at IS NULL
           AND ev.original_lineage_id = ?23
           AND ev.original_lineage_generation = ?24
           AND ev.original_source_sha256 = ?25
           AND ev.effective_source_sha256 = ?26
           AND ev.activation_revision = ?27",
        params![
            request_id,
            descriptor.service_surface,
            descriptor.provider,
            descriptor.adapter_id,
            descriptor.adapter_contract_version,
            payload.model,
            snapshots.capability,
            snapshots.data_tier,
            snapshots.retention,
            snapshots.consent,
            snapshots.policy_refs,
            snapshots.prompt_options,
            input_package_sha256,
            current.original_lineage_id,
            current.original_lineage_generation,
            current.original_source.sha256,
            current.render_source.sha256,
            payload_input_signature,
            state.native_recipe_signature,
            current.activation_revision,
            payload.icon_id,
            collection_id,
            current.original_lineage_id,
            current.original_lineage_generation,
            current.original_source.sha256,
            current.render_source.sha256,
            current.activation_revision,
        ],
    )?;
    if inserted != 1 {
        return Err(AppError::new(
            "ai_request_stale",
            "요청 준비 중 원본 또는 현재 소스가 변경되어 AI 요청을 시작하지 않았습니다.",
        ));
    }
    transaction.commit()?;

    Ok(PreparedImageEdit {
        request_id,
        collection_id: collection_id.to_string(),
        payload,
        original_lineage_id: current.original_lineage_id,
        original_lineage_generation: current.original_lineage_generation,
        original_source_sha256: current.original_source.sha256,
        effective_source_sha256: current.render_source.sha256,
        activation_revision: current.activation_revision,
        request_recipe_signature: state.native_recipe_signature,
        input_mime_type,
        input_bytes,
        snapshots,
    })
}

pub(crate) fn execute_started_http(
    connection: &mut Connection,
    paths: &AppPaths,
    prepared: &PreparedImageEdit,
    credential: &str,
) -> AppResult<AiReviewStateDto> {
    let transport = match ReqwestTransport::new() {
        Ok(transport) => transport,
        Err(error) => return fail_started_request(connection, &prepared.request_id, error),
    };
    execute_started_with_transport(connection, paths, prepared, credential, &transport)
}

fn execute_started_with_transport(
    connection: &mut Connection,
    paths: &AppPaths,
    prepared: &PreparedImageEdit,
    credential: &str,
    transport: &dyn AiImageTransport,
) -> AppResult<AiReviewStateDto> {
    if let Err(error) = claim_started_request_for_dispatch(connection, &prepared.request_id) {
        return fail_started_request(connection, &prepared.request_id, error);
    }
    let provider_image = match execute_with_transport(transport, prepared, credential) {
        Ok(image) => image,
        Err(error) => return fail_started_request(connection, &prepared.request_id, error),
    };
    match finalize_provider_candidate(connection, paths, prepared, provider_image) {
        Ok(state) => Ok(state),
        Err(error) => fail_started_request(connection, &prepared.request_id, error),
    }
}

fn claim_started_request_for_dispatch(connection: &Connection, request_id: &str) -> AppResult<()> {
    let claimed = connection.execute(
        "UPDATE ai_requests
         SET status = 'awaiting_result',
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?1
           AND status = 'running'",
        [request_id],
    )?;
    if claimed == 1 {
        return Ok(());
    }

    let status = connection
        .query_row(
            "SELECT status FROM ai_requests WHERE id = ?1",
            [request_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    match status.as_deref() {
        Some("cancelled") => Err(AppError::new(
            "ai_request_cancelled",
            "AI 요청이 전송 전에 취소되어 공급자 호출을 만들지 않았습니다.",
        )),
        Some(_) => Err(AppError::new(
            "ai_request_not_running",
            "AI 요청이 더 이상 전송 가능한 상태가 아닙니다.",
        )),
        None => Err(AppError::new(
            "ai_request_missing",
            "AI 요청 기록을 찾을 수 없습니다.",
        )),
    }
}

fn execute_with_transport(
    transport: &dyn AiImageTransport,
    prepared: &PreparedImageEdit,
    credential: &str,
) -> AppResult<ProviderImage> {
    match prepared.payload.provider.as_str() {
        "novelai" => execute_novelai(transport, prepared, credential),
        "gemini" => execute_gemini(transport, prepared, credential),
        _ => Err(invalid_provider()),
    }
}

pub(crate) fn finalize_provider_candidate(
    connection: &mut Connection,
    paths: &AppPaths,
    prepared_request: &PreparedImageEdit,
    provider_image: ProviderImage,
) -> AppResult<AiReviewStateDto> {
    let ProviderImage {
        bytes,
        original_filename,
        provider_usage,
        provider_request_id,
    } = provider_image;
    let file = ImportImageFilePayload {
        original_filename,
        bytes,
    };
    let prepared_source = prepare_source_file_from_bytes(
        &file,
        SourceFileImportOptions {
            allow_gif: false,
            exact_dimensions: None,
        },
    )?;
    let source_artifact_snapshot = prepared_source.artifact_snapshot(connection, paths)?;
    let provider_usage_json = provider_usage
        .as_ref()
        .map(|value| ai_snapshots::canonicalize("provider_usage", &value.to_string()))
        .transpose()?;
    let candidate_id = create_id("ai_candidate");

    let finalize_result = (|| -> AppResult<AiReviewStateDto> {
        let transaction =
            connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let status = transaction
            .query_row(
                "SELECT status FROM ai_requests WHERE id = ?1",
                [prepared_request.request_id.as_str()],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or_else(|| {
                AppError::new("ai_request_missing", "AI 요청 기록을 찾을 수 없습니다.")
            })?;
        if status != "awaiting_result" {
            return Err(AppError::new(
                if status == "cancelled" {
                    "ai_request_cancelled"
                } else {
                    "ai_request_not_running"
                },
                "AI 요청이 더 이상 실행 중이 아니므로 결과를 저장하지 않았습니다.",
            ));
        }

        let current = ai_repository::resolve_effective_visual_source(
            &transaction,
            &prepared_request.collection_id,
            &prepared_request.payload.icon_id,
        )?;
        let current_state = ai_repository::get_ai_review_state(
            &transaction,
            &prepared_request.collection_id,
            &prepared_request.payload.icon_id,
        )?;
        if current.original_lineage_id != prepared_request.original_lineage_id
            || current.original_lineage_generation != prepared_request.original_lineage_generation
            || current.original_source.sha256 != prepared_request.original_source_sha256
            || current.render_source.sha256 != prepared_request.effective_source_sha256
            || current.activation_revision != prepared_request.activation_revision
            || current_state.native_recipe_signature != prepared_request.request_recipe_signature
        {
            return Err(AppError::new(
                "ai_request_stale",
                "요청 중 원본 또는 편집 상태가 변경되어 AI 결과를 저장하지 않았습니다.",
            ));
        }

        let stored = commit_prepared_source_file(&transaction, paths, &prepared_source)?;
        let has_alpha = stored.has_alpha.ok_or_else(|| {
            AppError::new(
                "ai_candidate_metadata",
                "AI 후보의 알파 정보를 확인할 수 없습니다.",
            )
        })?;
        transaction.execute(
            "INSERT INTO ai_candidates (
               id, request_id, candidate_index, raw_source_file_id,
               raw_source_sha256, output_format, width, height, is_animated,
               has_alpha, provider_capabilities_snapshot_json, created_at
             ) VALUES (
               ?1, ?2, 0, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
               strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             )",
            params![
                candidate_id,
                prepared_request.request_id,
                stored.id,
                stored.sha256,
                stored.original_extension,
                stored.width,
                stored.height,
                i64::from(stored.is_animated),
                i64::from(has_alpha),
                prepared_request.snapshots.capability,
            ],
        )?;
        let completed = transaction.execute(
            "UPDATE ai_requests
             SET status = 'completed',
                 provider_request_id = ?1,
                 provider_usage_json = ?2,
                 error_code = NULL,
                 error_message = NULL,
                 completed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?3
               AND status = 'awaiting_result'",
            params![
                provider_request_id,
                provider_usage_json,
                prepared_request.request_id,
            ],
        )?;
        if completed != 1 {
            return Err(AppError::new(
                "ai_request_cancelled",
                "AI 요청이 취소되어 결과를 저장하지 않았습니다.",
            ));
        }
        let review_state = ai_repository::get_ai_review_state(
            &transaction,
            &prepared_request.collection_id,
            &prepared_request.payload.icon_id,
        )?;
        transaction.commit()?;
        Ok(review_state)
    })();

    if let Err(error) = finalize_result {
        let _ = source_artifact_snapshot.cleanup_if_unreferenced(connection);
        return Err(error);
    }
    finalize_result
}

pub(crate) fn record_started_request_failure(
    connection: &Connection,
    request_id: &str,
    error: &AppError,
) -> AppResult<bool> {
    let safe = safe_lifecycle_error(error);
    let updated = connection.execute(
        "UPDATE ai_requests
         SET status = 'failed',
             error_code = ?1,
             error_message = ?2,
             completed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?3
           AND status IN ('running', 'awaiting_result')",
        params![safe.code, safe.message, request_id],
    )?;
    Ok(updated == 1)
}

pub(crate) fn recover_interrupted_session_requests(connection: &Connection) -> AppResult<usize> {
    Ok(connection.execute(
        "UPDATE ai_requests
         SET status = 'failed',
             error_code = 'ai_request_interrupted',
             error_message = 'PMTCONCON Studio가 종료되어 세션 API 요청이 중단되었습니다. 자동 재시도하지 않았습니다.',
             completed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE request_scope = 'icon_edit'
           AND status IN ('running', 'awaiting_result')
           AND provider_mode = 'api'
           AND credential_mode_snapshot = 'session'",
        [],
    )?)
}

fn fail_started_request<T>(
    connection: &Connection,
    request_id: &str,
    error: AppError,
) -> AppResult<T> {
    let safe = safe_lifecycle_error(&error);
    let _ = record_started_request_failure(connection, request_id, &safe);
    Err(safe)
}

fn safe_lifecycle_error(error: &AppError) -> AppError {
    if error.code == "ai_gemini_paid_tier_required" {
        return AppError::new("ai_gemini_paid_tier_required", GEMINI_PAID_TIER_MESSAGE);
    }
    if error.code == "ai_unauthorized" && error.message == GEMINI_UNAUTHORIZED_MESSAGE {
        return AppError::new("ai_unauthorized", GEMINI_UNAUTHORIZED_MESSAGE);
    }
    if error.code == "ai_invalid_request" {
        return AppError::new(
            "ai_invalid_request",
            safe_invalid_request_message(&error.message),
        );
    }
    let (code, message) = match error.code.as_str() {
        "ai_unauthorized" => ("ai_unauthorized", "AI API 인증이 거부되었습니다."),
        "ai_rate_limited" => (
            "ai_rate_limited",
            "AI 공급자가 요청을 제한했습니다. 자동 재시도하지 않았습니다.",
        ),
        "ai_forbidden_or_tier" => (
            "ai_forbidden_or_tier",
            "AI 계정 권한 또는 이용 등급을 확인해 주세요.",
        ),
        "ai_redirect_rejected" => (
            "ai_redirect_rejected",
            "AI 공급자의 리디렉션 응답을 거부했습니다.",
        ),
        "ai_provider_error" => ("ai_provider_error", "AI 공급자가 오류를 반환했습니다."),
        "ai_network" => (
            "ai_network",
            "AI 공급자에 연결하지 못했습니다. 자동 재시도하지 않았습니다.",
        ),
        "ai_response_too_large" => (
            "ai_response_too_large",
            "AI 공급자 응답이 허용 크기를 초과했습니다.",
        ),
        "ai_response_schema" => (
            "ai_response_schema",
            "AI 공급자 응답 형식이 지원 계약과 다릅니다.",
        ),
        "ai_response_incomplete" => (
            "ai_response_incomplete",
            "AI 공급자 작업이 완료 상태가 아닙니다.",
        ),
        "ai_candidate_too_large" => (
            "ai_candidate_too_large",
            "AI 결과 이미지가 허용 크기를 초과했습니다.",
        ),
        "ai_request_cancelled" => (
            "ai_request_cancelled",
            "AI 요청이 취소되어 결과를 저장하지 않았습니다.",
        ),
        "ai_request_not_running" => (
            "ai_request_not_running",
            "AI 요청이 실행 중이 아니므로 결과를 저장하지 않았습니다.",
        ),
        "ai_request_missing" => ("ai_request_missing", "AI 요청 기록을 찾을 수 없습니다."),
        "ai_provider_worker" => (
            "ai_provider_worker",
            "AI 공급자 작업이 예기치 않게 중단되어 결과를 저장하지 않았습니다.",
        ),
        "ai_request_stale" | "not_found" => (
            "ai_request_stale",
            "요청 중 원본 또는 편집 상태가 변경되어 결과를 저장하지 않았습니다.",
        ),
        _ => (
            "ai_finalize_failed",
            "AI 결과를 안전하게 저장하지 못했습니다.",
        ),
    };
    AppError::new(code, message)
}

fn build_request_snapshots(
    descriptor: &ProviderDescriptor,
    payload: &ExecuteAiImageEditPayload,
) -> AppResult<RequestSnapshots> {
    Ok(RequestSnapshots {
        capability: ai_snapshots::canonicalize(
            "capability",
            &json!({
                "schema": "pmtcon-ai-capability-v1",
                "provider": descriptor.provider,
                "serviceSurface": descriptor.service_surface,
                "source": "direct-api-image-edit",
                "supports": ["static-image-input", "static-image-output", "single-sample"],
                "limitations": ["no-animation", "no-batch", "no-automatic-retry", "no-fallback"]
            })
            .to_string(),
        )?,
        data_tier: ai_snapshots::canonicalize(
            "data_tier",
            &json!({
                "schema": "pmtcon-ai-data-tier-v1",
                "source": "user-confirmed",
                "tier": if descriptor.provider == "google" { "paid-service" } else { "account-subscription-or-anlas" }
            })
            .to_string(),
        )?,
        retention: ai_snapshots::canonicalize(
            "retention",
            r#"{"schema":"pmtcon-ai-retention-v1","source":"provider-policy-2026-07-28","retention":"provider-controlled"}"#,
        )?,
        consent: ai_snapshots::canonicalize(
            "consent",
            &json!({
                "schema": "pmtcon-ai-consent-v1",
                "source": "direct-api-image-edit",
                "confirmed": true,
                "humanActionConfirmed": payload.consent.human_action_confirmed,
                "rightsConfirmed": payload.consent.rights_confirmed,
                "costConfirmed": payload.consent.cost_confirmed,
                "requestContentConfirmed": payload.consent.request_content_confirmed,
                "contractOverrideConfirmed": payload.consent.contract_override_confirmed,
                "adultConfirmed": payload.consent.adult_confirmed,
                "under18AudienceExcludedConfirmed": payload.consent.under18_audience_excluded_confirmed,
                "professionalBusinessConfirmed": payload.consent.professional_business_confirmed,
                "supportedRegionConfirmed": payload.consent.supported_region_confirmed,
                "paidServiceConfirmed": payload.consent.paid_service_confirmed
            })
            .to_string(),
        )?,
        policy_refs: ai_snapshots::canonicalize(
            "policy_refs",
            &descriptor.policy_refs.to_string(),
        )?,
        prompt_options: ai_snapshots::canonicalize(
            "prompt_options",
            &json!({
                "schema": "pmtcon-ai-prompt-options-v1",
                "operation": "static_image_edit",
                "provider": descriptor.provider,
                "model": payload.model,
                "action": payload.options.action,
                "prompt": payload.prompt,
                "negativePrompt": payload.options.negative_prompt,
                "width": payload.options.width,
                "height": payload.options.height,
                "steps": payload.options.steps,
                "scale": payload.options.scale,
                "strength": payload.options.strength,
                "noise": payload.options.noise,
                "outputCount": 1
            })
            .to_string(),
        )?,
    })
}
fn execute_novelai(
    transport: &dyn AiImageTransport,
    prepared: &PreparedImageEdit,
    credential: &str,
) -> AppResult<ProviderImage> {
    let options = &prepared.payload.options;
    let action = options.action.as_deref().ok_or_else(|| {
        AppError::new(
            "ai_contract_required",
            "NovelAI action의 정확한 계약 값을 입력해 주세요.",
        )
    })?;
    if prepared.input_mime_type != "image/png" {
        return Err(AppError::new(
            "ai_input_contract",
            "NovelAI 전송 입력이 네트워크 전에 PNG로 확정되지 않았습니다.",
        ));
    }
    let input_png = &prepared.input_bytes;
    let mut parameters = serde_json::Map::from_iter([
        ("width".to_string(), json!(options.width)),
        ("height".to_string(), json!(options.height)),
        ("steps".to_string(), json!(options.steps)),
        ("scale".to_string(), json!(options.scale)),
        ("n_samples".to_string(), json!(1)),
        (
            "image".to_string(),
            json!(BASE64_STANDARD.encode(input_png)),
        ),
        ("image_format".to_string(), json!("png")),
        ("strength".to_string(), json!(options.strength)),
        ("noise".to_string(), json!(options.noise)),
    ]);
    if let Some(negative_prompt) = options.negative_prompt.as_ref() {
        parameters.insert(
            "negative_prompt".to_string(),
            Value::String(negative_prompt.clone()),
        );
    }
    let body = json!({
        "action": action,
        "input": prepared.payload.prompt,
        "model": prepared.payload.model,
        "parameters": parameters
    });
    let response = post_json(
        transport,
        NOVELAI_ENDPOINT,
        ProviderAuthorization::Bearer(credential),
        &body,
    )?;
    require_success(&response, "novelai")?;
    require_json_content_type(&response)?;
    let value: Value =
        serde_json::from_slice(&response.body).map_err(|_| response_schema_error("NovelAI"))?;
    let first = value
        .get("images")
        .and_then(Value::as_array)
        .and_then(|images| images.first())
        .and_then(Value::as_object)
        .ok_or_else(|| response_schema_error("NovelAI"))?;
    let encoded = first
        .get("image")
        .and_then(Value::as_str)
        .ok_or_else(|| response_schema_error("NovelAI"))?;
    let bytes = decode_image_base64(encoded)?;
    let usage = json!({
        "schema": "pmtcon-ai-provider-usage-v1",
        "provider": "novelai",
        "candidateIndex": first.get("index").and_then(Value::as_i64),
        "seed": first.get("seed").and_then(Value::as_i64)
    });
    Ok(ProviderImage {
        bytes,
        original_filename: "novelai-result.png".to_string(),
        provider_usage: Some(usage),
        provider_request_id: None,
    })
}

fn prepare_provider_input(
    provider: &str,
    input_mime_type: &str,
    input_bytes: Vec<u8>,
) -> AppResult<(String, Vec<u8>)> {
    match provider {
        "novelai" => Ok((
            "image/png".to_string(),
            novelai_png_input(input_mime_type, input_bytes)?,
        )),
        "gemini" => Ok((input_mime_type.to_string(), input_bytes)),
        _ => Err(invalid_provider()),
    }
}

fn novelai_png_input(input_mime_type: &str, input_bytes: Vec<u8>) -> AppResult<Vec<u8>> {
    if input_mime_type == "image/png" {
        return Ok(input_bytes);
    }
    if input_mime_type != "image/jpeg" {
        return Err(AppError::new(
            "ai_input_contract",
            "NovelAI 정적 편집은 PNG 또는 JPEG 입력만 전송할 수 있습니다.",
        ));
    }
    let image = image::load_from_memory(&input_bytes).map_err(|_| {
        AppError::new(
            "ai_input_decode",
            "NovelAI 전송 전에 JPEG 입력을 PNG로 변환하지 못했습니다.",
        )
    })?;
    let mut cursor = Cursor::new(Vec::new());
    image
        .write_to(&mut cursor, ImageFormat::Png)
        .map_err(|_| AppError::new("ai_input_encode", "NovelAI 전송용 PNG를 만들지 못했습니다."))?;
    let bytes = cursor.into_inner();
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(AppError::new(
            "ai_input_too_large",
            "NovelAI 전송용 PNG가 16MB 제한을 넘었습니다.",
        ));
    }
    Ok(bytes)
}
fn execute_gemini(
    transport: &dyn AiImageTransport,
    prepared: &PreparedImageEdit,
    credential: &str,
) -> AppResult<ProviderImage> {
    let mut response_format = serde_json::Map::from_iter([
        ("type".to_string(), json!("image")),
        ("mime_type".to_string(), json!("image/jpeg")),
        ("aspect_ratio".to_string(), json!("1:1")),
        ("delivery".to_string(), json!("inline")),
    ]);
    if prepared.payload.model == "gemini-3.1-flash-image" {
        response_format.insert("image_size".to_string(), json!("1K"));
    }
    let body = json!({
        "model": prepared.payload.model,
        "input": [
            { "type": "text", "text": prepared.payload.prompt },
            {
                "type": "image",
                "mime_type": prepared.input_mime_type,
                "data": BASE64_STANDARD.encode(&prepared.input_bytes)
            }
        ],
        "response_format": response_format,
        "store": false
    });
    let response = post_json(
        transport,
        GEMINI_ENDPOINT,
        ProviderAuthorization::GoogleApiKey(credential),
        &body,
    )?;
    require_success(&response, "gemini")?;
    require_json_content_type(&response)?;
    let value: Value =
        serde_json::from_slice(&response.body).map_err(|_| response_schema_error("Gemini"))?;
    if value.get("status").and_then(Value::as_str) != Some("completed") {
        return Err(AppError::new(
            "ai_response_incomplete",
            "Gemini 작업이 완료 상태가 아니어서 결과를 저장하지 않았습니다.",
        ));
    }
    let image = value
        .get("steps")
        .and_then(Value::as_array)
        .and_then(|steps| {
            steps
                .iter()
                .filter(|step| step.get("type").and_then(Value::as_str) == Some("model_output"))
                .filter_map(|step| step.get("content").and_then(Value::as_array))
                .flat_map(|content| content.iter())
                .filter(|part| {
                    part.get("type").and_then(Value::as_str) == Some("image")
                        && part.get("mime_type").and_then(Value::as_str) == Some("image/jpeg")
                })
                .last()
        })
        .ok_or_else(|| response_schema_error("Gemini"))?;
    let encoded = image
        .get("data")
        .and_then(Value::as_str)
        .ok_or_else(|| response_schema_error("Gemini"))?;
    let bytes = decode_image_base64(encoded)?;
    let provider_request_id = parse_provider_request_id(value.get("id"))?;
    let usage = sanitized_gemini_usage(value.get("usage"));
    Ok(ProviderImage {
        bytes,
        original_filename: "gemini-result.jpg".to_string(),
        provider_usage: usage,
        provider_request_id: Some(provider_request_id),
    })
}

fn validate_payload(payload: &ExecuteAiImageEditPayload) -> AppResult<()> {
    validate_bounded_text("프롬프트", &payload.prompt, 1, MAX_PROMPT_BYTES)?;
    validate_bounded_identifier("모델 ID", &payload.model, MAX_MODEL_BYTES)?;
    if !payload.consent.human_action_confirmed
        || !payload.consent.rights_confirmed
        || !payload.consent.cost_confirmed
        || !payload.consent.request_content_confirmed
    {
        return Err(AppError::new(
            "ai_consent_required",
            "사람이 시작하는 1회 요청, 권리, 비용, 외부 전송 확인이 모두 필요합니다.",
        ));
    }
    match payload.provider.as_str() {
        "novelai" => {
            if !payload.consent.contract_override_confirmed {
                return Err(AppError::new(
                    "ai_contract_confirmation_required",
                    "NovelAI 모델 ID와 action 계약을 직접 확인해야 합니다.",
                ));
            }
            let action = payload.options.action.as_deref().unwrap_or_default();
            validate_bounded_identifier("NovelAI action", action, MAX_ACTION_BYTES)?;
            let width = payload.options.width.unwrap_or_default();
            let height = payload.options.height.unwrap_or_default();
            let steps = payload.options.steps.unwrap_or_default();
            let scale = payload.options.scale.unwrap_or(f64::NAN);
            let strength = payload.options.strength.unwrap_or(f64::NAN);
            let noise = payload.options.noise.unwrap_or(f64::NAN);
            if !(64..=4096).contains(&width)
                || width % 64 != 0
                || !(64..=4096).contains(&height)
                || height % 64 != 0
                || !(1..=50).contains(&steps)
                || !finite_between(scale, 0.0, 20.0)
                || !finite_between(strength, 0.0, 1.0)
                || !finite_between(noise, 0.0, 1.0)
            {
                return Err(AppError::new(
                    "ai_options_invalid",
                    "NovelAI 크기·스텝·강도 옵션이 허용 범위를 벗어났습니다.",
                ));
            }
            if let Some(negative_prompt) = payload.options.negative_prompt.as_deref() {
                validate_bounded_text("제외 프롬프트", negative_prompt, 0, MAX_PROMPT_BYTES)?;
            }
        }
        "gemini" => {
            if !GEMINI_MODELS.contains(&payload.model.as_str())
                || !payload.consent.adult_confirmed
                || !payload.consent.under18_audience_excluded_confirmed
                || !payload.consent.professional_business_confirmed
                || !payload.consent.supported_region_confirmed
                || !payload.consent.paid_service_confirmed
            {
                return Err(AppError::new(
                    "gemini_eligibility_required",
                    "Gemini 실험실 호출에는 허용된 모델과 연령·전문/사업 목적·지원 지역·유료 서비스 확인이 모두 필요합니다.",
                ));
            }
            if payload.options != Default::default() {
                return Err(AppError::new(
                    "ai_options_invalid",
                    "Gemini 1K 이미지 편집 파일럿은 별도 고급 옵션을 받지 않습니다.",
                ));
            }
        }
        _ => return Err(invalid_provider()),
    }
    Ok(())
}

fn validate_static_source(current: &EffectiveVisualSource) -> AppResult<()> {
    if current.render_source.is_animated
        || !matches!(
            current.render_source.mime_type.as_str(),
            "image/png" | "image/jpeg"
        )
    {
        return Err(AppError::new(
            "ai_static_input_required",
            "이번 API 단계에서는 정적 JPG 또는 PNG 현재 소스만 수정할 수 있습니다.",
        ));
    }
    Ok(())
}

fn managed_input_path(paths: &AppPaths, stored_path: &str) -> AppResult<PathBuf> {
    let root = paths.root.canonicalize().map_err(|_| {
        AppError::new(
            "ai_input_path",
            "앱 데이터 경로를 확인할 수 없어 AI 전송을 중단했습니다.",
        )
    })?;
    let canonical = PathBuf::from(stored_path).canonicalize().map_err(|_| {
        AppError::new(
            "ai_input_path",
            "AI로 전송할 현재 소스 파일을 확인할 수 없습니다.",
        )
    })?;
    if !canonical.starts_with(&root) || !canonical.is_file() {
        return Err(AppError::new(
            "ai_input_path",
            "AI 전송 소스가 앱의 관리 경로 밖에 있어 요청을 중단했습니다.",
        ));
    }
    Ok(canonical)
}
fn descriptor_for(provider: &str) -> AppResult<ProviderDescriptor> {
    match provider {
        "novelai" => Ok(ProviderDescriptor {
            provider: "novelai",
            service_surface: "novelai_api",
            adapter_id: "pmtcon-novelai-image-json",
            adapter_contract_version: "2026-07-28-experimental-1",
            policy_refs: json!([
                "https://image.novelai.net/docs/index.html",
                "https://image.novelai.net/docs/doc.json",
                "https://docs.novelai.net/en/text/usersettings/account/",
                "https://docs.novelai.net/en/subscription/",
                "https://novelai.net/terms"
            ]),
        }),
        "gemini" => Ok(ProviderDescriptor {
            provider: "google",
            service_surface: "gemini_api",
            adapter_id: "pmtcon-gemini-interactions-image",
            adapter_contract_version: "2026-07-29-private-pilot-3",
            policy_refs: json!([
                "https://ai.google.dev/gemini-api/docs/image-generation",
                "https://ai.google.dev/gemini-api/docs/pricing",
                "https://ai.google.dev/gemini-api/docs/api-key",
                "https://ai.google.dev/gemini-api/terms"
            ]),
        }),
        _ => Err(invalid_provider()),
    }
}

fn post_json(
    transport: &dyn AiImageTransport,
    endpoint: &'static str,
    authorization: ProviderAuthorization<'_>,
    value: &Value,
) -> AppResult<TransportResponse> {
    let body = serde_json::to_vec(value).map_err(|_| {
        AppError::new(
            "ai_request_encoding",
            "AI 요청을 안전하게 직렬화할 수 없습니다.",
        )
    })?;
    transport
        .post_json(endpoint, authorization, &body, MAX_RESPONSE_BYTES)
        .map_err(|failure| match failure {
            TransportFailure::Network => AppError::new(
                "ai_network",
                "AI 공급자에 연결하지 못했습니다. 자동 재시도하지 않았습니다.",
            ),
            TransportFailure::ResponseTooLarge => AppError::new(
                "ai_response_too_large",
                "AI 공급자 응답이 24MB 제한을 넘었습니다. 결과를 저장하지 않았습니다.",
            ),
        })
}

fn require_success(response: &TransportResponse, provider: &str) -> AppResult<()> {
    let expected_status = if provider == "novelai" { 201 } else { 200 };
    if response.status == expected_status {
        return Ok(());
    }
    match response.status {
        401 => Err(AppError::new(
            "ai_unauthorized",
            if provider == "novelai" {
                "NovelAI PAT가 거부되었습니다. 새 PAT 발급 시 이전 PAT는 무효화됩니다. 키를 다시 연결해 주세요."
            } else {
                "Gemini API 키가 거부되었습니다. 새 Auth key와 프로젝트 권한을 확인해 주세요."
            },
        )),
        429 => Err(AppError::new(
            "ai_rate_limited",
            "공급자가 요청을 제한했습니다. 자동 재시도하지 않았습니다. 나중에 사용자가 직접 다시 시도해 주세요.",
        )),
        400 if provider == "gemini" => Err(gemini_bad_request_error(&response.body)),
        400 => Err(AppError::new(
            "ai_invalid_request",
            invalid_request_message_from_body(&response.body),
        )),
        403 => Err(AppError::new(
            "ai_forbidden_or_tier",
            "공급자가 계정 권한 또는 이용 등급 때문에 요청을 거부했습니다.",
        )),
        300..=399 => Err(AppError::new(
            "ai_redirect_rejected",
            "공급자가 다른 주소로 이동을 요구해 요청을 중단했습니다.",
        )),
        _ => Err(AppError::new(
            "ai_provider_error",
            format!(
                "공급자가 HTTP {} 오류를 반환했습니다. 응답 본문은 기록하지 않았고 자동 재시도하지 않았습니다.",
                response.status
            ),
        )),
    }
}

const GEMINI_PAID_TIER_MESSAGE: &str =
    "Gemini 이미지 API에는 무료 등급이 없습니다. 비용을 원치 않으면 API 요청을 중단하고 Google AI Studio 웹에서 현재 계정·모델의 과금 표시를 직접 확인해 주세요. API를 사용하려면 프로젝트 결제를 사용 설정해야 합니다.";
const GEMINI_UNAUTHORIZED_MESSAGE: &str =
    "Gemini API 키가 거부되었습니다. 키가 올바른지와 API 키의 프로젝트·API 제한 설정을 확인해 주세요.";

fn gemini_bad_request_error(body: &[u8]) -> AppError {
    let value = serde_json::from_slice::<Value>(body).ok();
    let status = value
        .as_ref()
        .and_then(|value| value.pointer("/error/status"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let searchable = value
        .as_ref()
        .and_then(|value| value.get("error"))
        .map(Value::to_string)
        .unwrap_or_default()
        .to_ascii_lowercase();

    if status.eq_ignore_ascii_case("failed_precondition")
        || contains_any(
            &searchable,
            &[
                "free tier is not available",
                "free-tier is not available",
                "enable billing",
                "billing is not enabled",
                "paid plan",
                "paid tier",
            ],
        )
    {
        return AppError::new("ai_gemini_paid_tier_required", GEMINI_PAID_TIER_MESSAGE);
    }

    if status.eq_ignore_ascii_case("unauthenticated")
        || contains_any(
            &searchable,
            &[
                "api key not valid",
                "api key is invalid",
                "invalid api key",
                "api_key_invalid",
                "api key was reported as leaked",
            ],
        )
    {
        return AppError::new("ai_unauthorized", GEMINI_UNAUTHORIZED_MESSAGE);
    }

    AppError::new(
        "ai_invalid_request",
        invalid_request_message_from_body(body),
    )
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

const INVALID_REQUEST_MESSAGE: &str = "AI 공급자가 요청 형식을 거부했습니다.";
const INVALID_REQUEST_GENERIC_HINT: &str = "선택한 모델과 공급자 요청 옵션을 확인해 주세요.";

fn invalid_request_message_from_body(body: &[u8]) -> String {
    let searchable = serde_json::from_slice::<Value>(body)
        .map(|value| value.to_string())
        .unwrap_or_default();
    invalid_request_message(&searchable)
}

fn safe_invalid_request_message(message: &str) -> String {
    invalid_request_message(message)
}

fn invalid_request_message(searchable: &str) -> String {
    let searchable = searchable.to_ascii_lowercase();
    let mut hints = Vec::new();
    if searchable.contains("image_size") || searchable.contains("imagesize") {
        hints.push("이미지 크기(image_size, Gemini 2.5에서는 생략)");
    }
    if searchable.contains("model") {
        hints.push("모델 ID(model)");
    }
    if searchable.contains("response_format") || searchable.contains("responseformat") {
        hints.push("응답 형식(response_format)");
    }
    if searchable.contains("mime_type") || searchable.contains("mimetype") {
        hints.push("이미지 형식(mime_type)");
    }
    if searchable.contains("action") {
        hints.push("NovelAI 작업(action)");
    }

    if hints.is_empty() {
        format!("{INVALID_REQUEST_MESSAGE} {INVALID_REQUEST_GENERIC_HINT}")
    } else {
        format!("{INVALID_REQUEST_MESSAGE} 확인 항목: {}.", hints.join(", "))
    }
}

fn require_json_content_type(response: &TransportResponse) -> AppResult<()> {
    if response
        .content_type
        .as_deref()
        .is_some_and(|value| value.to_ascii_lowercase().starts_with("application/json"))
    {
        Ok(())
    } else {
        Err(response_schema_error("AI 공급자"))
    }
}

fn decode_image_base64(encoded: &str) -> AppResult<Vec<u8>> {
    if encoded.len() > MAX_RESPONSE_BYTES * 2 {
        return Err(AppError::new(
            "ai_response_too_large",
            "AI 이미지 응답이 허용 크기를 넘었습니다.",
        ));
    }
    let bytes = BASE64_STANDARD
        .decode(encoded.as_bytes())
        .map_err(|_| response_schema_error("AI 공급자"))?;
    if bytes.is_empty() || bytes.len() > MAX_INPUT_BYTES {
        return Err(AppError::new(
            "ai_candidate_too_large",
            "AI 결과 이미지는 비어 있지 않은 16MB 이하 JPG 또는 PNG여야 합니다.",
        ));
    }
    Ok(bytes)
}

fn parse_provider_request_id(value: Option<&Value>) -> AppResult<String> {
    let id = value
        .and_then(Value::as_str)
        .ok_or_else(|| response_schema_error("Gemini"))?;
    if id.is_empty()
        || id.len() > 256
        || !id.is_ascii()
        || id
            .bytes()
            .any(|byte| !(byte.is_ascii_alphanumeric() || b"-_.:/".contains(&byte)))
    {
        return Err(response_schema_error("Gemini"));
    }
    Ok(id.to_string())
}
fn sanitized_gemini_usage(value: Option<&Value>) -> Option<Value> {
    let usage = value?.as_object()?;
    let allowed = [
        "input_tokens_by_modality",
        "output_tokens_by_modality",
        "total_thought_tokens",
        "total_tokens",
    ];
    let mut sanitized = serde_json::Map::new();
    sanitized.insert(
        "schema".to_string(),
        Value::String("pmtcon-ai-provider-usage-v1".to_string()),
    );
    sanitized.insert("provider".to_string(), Value::String("google".to_string()));
    for key in allowed {
        if let Some(value) = usage.get(key) {
            if value.is_number() || value.is_object() || value.is_array() {
                sanitized.insert(key.to_string(), value.clone());
            }
        }
    }
    Some(Value::Object(sanitized))
}

fn validate_bounded_text(
    label: &str,
    value: &str,
    minimum: usize,
    maximum: usize,
) -> AppResult<()> {
    let trimmed = value.trim();
    if trimmed.chars().count() < minimum
        || value.len() > maximum
        || value.chars().any(char::is_control)
    {
        return Err(AppError::new(
            "ai_prompt_invalid",
            format!("{label}은 {maximum}자 이내의 일반 텍스트여야 합니다."),
        ));
    }
    Ok(())
}

fn validate_bounded_identifier(label: &str, value: &str, maximum: usize) -> AppResult<()> {
    if value.is_empty()
        || value.len() > maximum
        || !value.is_ascii()
        || value
            .bytes()
            .any(|byte| !(byte.is_ascii_alphanumeric() || b"-_.:/".contains(&byte)))
    {
        return Err(AppError::new(
            "ai_contract_invalid",
            format!("{label} 형식이 올바르지 않습니다."),
        ));
    }
    Ok(())
}

fn finite_between(value: f64, minimum: f64, maximum: f64) -> bool {
    value.is_finite() && value >= minimum && value <= maximum
}

fn response_schema_error(provider: &str) -> AppError {
    AppError::new(
        "ai_response_schema",
        format!("{provider} 응답 형식이 현재 어댑터 계약과 다릅니다. 결과를 저장하지 않았습니다."),
    )
}

fn invalid_provider() -> AppError {
    AppError::new(
        "ai_provider_invalid",
        "지원하지 않는 AI 공급자입니다. NovelAI 또는 Gemini를 선택해 주세요.",
    )
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    };
    use std::time::{SystemTime, UNIX_EPOCH};

    use image::{DynamicImage, ImageBuffer, ImageFormat, Rgb, Rgba};

    use super::*;
    use crate::ai_provider::transport::{ProviderAuthorization, TransportFailure};
    use crate::db::connection::open_database_with_paths;
    use crate::db::repositories::{collections, imports};
    use crate::models::{AiImageEditConsentPayload, AiImageEditOptionsPayload};

    #[derive(Default)]
    struct FakeTransport {
        calls: Mutex<Vec<(String, String, Value)>>,
        response: Mutex<Option<Result<TransportResponse, TransportFailure>>>,
    }

    impl FakeTransport {
        fn with_json(status: u16, value: Value) -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
                response: Mutex::new(Some(Ok(TransportResponse {
                    status,
                    content_type: Some("application/json".to_string()),
                    body: serde_json::to_vec(&value).unwrap(),
                }))),
            }
        }
    }

    impl AiImageTransport for FakeTransport {
        fn post_json(
            &self,
            endpoint: &'static str,
            authorization: ProviderAuthorization<'_>,
            body: &[u8],
            _max_response_bytes: usize,
        ) -> Result<TransportResponse, TransportFailure> {
            let auth = match authorization {
                ProviderAuthorization::Bearer(value) => format!("Bearer {value}"),
                ProviderAuthorization::GoogleApiKey(value) => format!("x-goog-api-key {value}"),
            };
            self.calls.lock().unwrap().push((
                endpoint.to_string(),
                auth,
                serde_json::from_slice(body).unwrap(),
            ));
            self.response.lock().unwrap().take().unwrap()
        }
    }

    fn consent() -> AiImageEditConsentPayload {
        AiImageEditConsentPayload {
            human_action_confirmed: true,
            rights_confirmed: true,
            cost_confirmed: true,
            request_content_confirmed: true,
            contract_override_confirmed: true,
            adult_confirmed: true,
            under18_audience_excluded_confirmed: true,
            professional_business_confirmed: true,
            supported_region_confirmed: true,
            paid_service_confirmed: true,
        }
    }

    fn prepared(provider: &str) -> PreparedImageEdit {
        PreparedImageEdit {
            request_id: "ai_request_test".to_string(),
            collection_id: "collection".to_string(),
            payload: ExecuteAiImageEditPayload {
                icon_id: "icon".to_string(),
                provider: provider.to_string(),
                prompt: "표정을 밝게".to_string(),
                model: if provider == "novelai" {
                    "nai-diffusion-3".to_string()
                } else {
                    "gemini-2.5-flash-image".to_string()
                },
                options: if provider == "novelai" {
                    AiImageEditOptionsPayload {
                        negative_prompt: Some("text".to_string()),
                        action: Some("img2img".to_string()),
                        width: Some(1024),
                        height: Some(1024),
                        steps: Some(28),
                        scale: Some(5.0),
                        strength: Some(0.7),
                        noise: Some(0.0),
                    }
                } else {
                    AiImageEditOptionsPayload::default()
                },
                consent: consent(),
            },
            original_lineage_id: "lineage".to_string(),
            original_lineage_generation: 0,
            original_source_sha256: "a".repeat(64),
            effective_source_sha256: "b".repeat(64),
            activation_revision: 0,
            request_recipe_signature: "recipe".to_string(),
            input_mime_type: "image/png".to_string(),
            input_bytes: b"source".to_vec(),
            snapshots: RequestSnapshots {
                capability: "{}".to_string(),
                data_tier: "{}".to_string(),
                retention: "{}".to_string(),
                consent: "{}".to_string(),
                policy_refs: "[]".to_string(),
                prompt_options: "{}".to_string(),
            },
        }
    }

    #[test]
    fn novelai_contract_is_one_exact_json_request() {
        let encoded = BASE64_STANDARD.encode(b"png");
        let transport = FakeTransport::with_json(
            201,
            json!({"images":[{"image":encoded,"index":0,"seed":7}]}),
        );
        let result =
            execute_with_transport(&transport, &prepared("novelai"), "pst-unit-secret").unwrap();
        assert_eq!(result.bytes, b"png");
        let calls = transport.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, NOVELAI_ENDPOINT);
        assert_eq!(calls[0].1, "Bearer pst-unit-secret");
        assert_eq!(calls[0].2["parameters"]["n_samples"], 1);
        assert_eq!(calls[0].2["parameters"]["image_format"], "png");
        assert!(calls[0].2["parameters"].get("img2img").is_none());
    }

    fn gemini_success_response(id: &str, jpeg: &[u8]) -> Value {
        json!({
            "id": id,
            "status": "completed",
            "steps": [{
                "type": "model_output",
                "content": [{
                    "type": "image",
                    "mime_type": "image/jpeg",
                    "data": BASE64_STANDARD.encode(jpeg)
                }]
            }],
            "usage": {"total_tokens": 12}
        })
    }

    #[test]
    fn gemini_2_5_exact_interactions_body_omits_image_size() {
        let jpeg = jpeg_bytes([7, 8, 9]);
        let transport =
            FakeTransport::with_json(200, gemini_success_response("interaction_2_5", &jpeg));
        let prepared = prepared("gemini");
        let expected_body = json!({
            "model": "gemini-2.5-flash-image",
            "input": [
                {"type": "text", "text": prepared.payload.prompt.clone()},
                {
                    "type": "image",
                    "mime_type": prepared.input_mime_type.clone(),
                    "data": BASE64_STANDARD.encode(&prepared.input_bytes)
                }
            ],
            "response_format": {
                "type": "image",
                "mime_type": "image/jpeg",
                "aspect_ratio": "1:1",
                "delivery": "inline"
            },
            "store": false
        });

        let result = execute_with_transport(&transport, &prepared, "gemini-unit-secret").unwrap();

        assert_eq!(result.bytes, jpeg);
        assert_eq!(result.original_filename, "gemini-result.jpg");
        assert_eq!(
            result.provider_request_id.as_deref(),
            Some("interaction_2_5")
        );
        let calls = transport.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, GEMINI_ENDPOINT);
        assert_eq!(calls[0].1, "x-goog-api-key gemini-unit-secret");
        assert_eq!(calls[0].2, expected_body);
        assert!(calls[0].2["response_format"].get("image_size").is_none());
    }

    #[test]
    fn gemini_3_1_exact_interactions_body_includes_1k_image_size() {
        let jpeg = jpeg_bytes([10, 11, 12]);
        let transport =
            FakeTransport::with_json(200, gemini_success_response("interaction_3_1", &jpeg));
        let mut prepared = prepared("gemini");
        prepared.payload.model = "gemini-3.1-flash-image".to_string();
        let expected_body = json!({
            "model": "gemini-3.1-flash-image",
            "input": [
                {"type": "text", "text": prepared.payload.prompt.clone()},
                {
                    "type": "image",
                    "mime_type": prepared.input_mime_type.clone(),
                    "data": BASE64_STANDARD.encode(&prepared.input_bytes)
                }
            ],
            "response_format": {
                "type": "image",
                "mime_type": "image/jpeg",
                "aspect_ratio": "1:1",
                "image_size": "1K",
                "delivery": "inline"
            },
            "store": false
        });

        let result = execute_with_transport(&transport, &prepared, "gemini-unit-secret").unwrap();

        assert_eq!(result.bytes, jpeg);
        let calls = transport.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].2, expected_body);
        assert_eq!(
            descriptor_for("gemini").unwrap().adapter_contract_version,
            "2026-07-29-private-pilot-3"
        );
    }

    #[test]
    fn gemini_uses_last_jpeg_across_all_model_output_steps() {
        let first = jpeg_bytes([1, 2, 3]);
        let ignored_thought = jpeg_bytes([4, 5, 6]);
        let last = jpeg_bytes([7, 8, 9]);
        let transport = FakeTransport::with_json(
            200,
            json!({
                "id": "interaction_last_image",
                "status": "completed",
                "steps": [
                    {
                        "type": "model_output",
                        "content": [
                            {"type": "text", "text": "first"},
                            {
                                "type": "image",
                                "mime_type": "image/jpeg",
                                "data": BASE64_STANDARD.encode(&first)
                            }
                        ]
                    },
                    {
                        "type": "thought",
                        "summary": [{
                            "type": "image",
                            "mime_type": "image/jpeg",
                            "data": BASE64_STANDARD.encode(&ignored_thought)
                        }]
                    },
                    {
                        "type": "model_output",
                        "content": [
                            {
                                "type": "image",
                                "mime_type": "image/png",
                                "data": BASE64_STANDARD.encode(png_bytes([1, 2, 3, 255]))
                            },
                            {
                                "type": "image",
                                "mime_type": "image/jpeg",
                                "data": BASE64_STANDARD.encode(&last)
                            }
                        ]
                    },
                    {"type": "model_output", "content": [{"type": "text", "text": "done"}]}
                ]
            }),
        );

        let result =
            execute_with_transport(&transport, &prepared("gemini"), "gemini-unit-secret").unwrap();

        assert_eq!(result.bytes, last);
        assert_ne!(result.bytes, first);
        assert_ne!(result.bytes, ignored_thought);
        assert_eq!(transport.calls.lock().unwrap().len(), 1);
    }
    #[test]
    fn gemini_resource_name_prefix_is_rejected_before_dispatch() {
        let mut payload = prepared("gemini").payload;
        payload.model = "models/gemini-2.5-flash-image".to_string();
        let error = validate_payload(&payload).unwrap_err();

        assert_eq!(error.code, "gemini_eligibility_required");
    }

    #[test]
    fn invalid_request_lifecycle_message_is_actionable_and_redacted() {
        let safe = safe_lifecycle_error(&AppError::new(
            "ai_invalid_request",
            "provider raw body with secret",
        ));
        assert_eq!(safe.code, "ai_invalid_request");
        assert!(safe.message.contains("선택한 모델과 공급자 요청 옵션"));
        assert!(!safe.message.contains("secret"));
    }

    #[test]
    fn gemini_rejects_non_jpeg_response_without_retry() {
        let encoded = BASE64_STANDARD.encode(png_bytes([7, 8, 9, 255]));
        let transport = FakeTransport::with_json(
            200,
            json!({
                "id":"interaction_test_123",
                "status":"completed",
                "steps":[{
                    "type":"model_output",
                    "content":[{"type":"image","mime_type":"image/png","data":encoded}]
                }]
            }),
        );
        let error = execute_with_transport(&transport, &prepared("gemini"), "gemini-unit-secret")
            .unwrap_err();
        assert_eq!(error.code, "ai_response_schema");
        assert_eq!(transport.calls.lock().unwrap().len(), 1);
    }

    #[test]
    fn gemini_400_auth_and_paid_tier_errors_are_actionable_and_redacted() {
        let cases = [
            (
                json!({
                    "error": {
                        "code": 400,
                        "status": "FAILED_PRECONDITION",
                        "message": "Gemini API free tier is not available. Enable billing. hidden-billing-id"
                    }
                }),
                "ai_gemini_paid_tier_required",
                GEMINI_PAID_TIER_MESSAGE,
                "hidden-billing-id",
            ),
            (
                json!({
                    "error": {
                        "code": 400,
                        "status": "INVALID_ARGUMENT",
                        "message": "API key not valid. Please pass a valid API key. hidden-key-id",
                        "details": [{"reason": "API_KEY_INVALID"}]
                    }
                }),
                "ai_unauthorized",
                GEMINI_UNAUTHORIZED_MESSAGE,
                "hidden-key-id",
            ),
        ];

        for (body, expected_code, expected_message, private_marker) in cases {
            let transport = FakeTransport::with_json(400, body);
            let error =
                execute_with_transport(&transport, &prepared("gemini"), "gemini-unit-secret")
                    .unwrap_err();

            assert_eq!(error.code, expected_code);
            assert_eq!(error.message, expected_message);
            assert_eq!(transport.calls.lock().unwrap().len(), 1);
            assert!(!error.message.contains(private_marker));
            assert!(!error.message.contains("gemini-unit-secret"));
            let safe = safe_lifecycle_error(&error);
            assert_eq!(safe.code, expected_code);
            assert_eq!(safe.message, expected_message);
            assert!(!safe.message.contains(private_marker));
            assert!(!safe.message.contains("gemini-unit-secret"));
            for serialized in [
                serde_json::to_string(&error).unwrap(),
                serde_json::to_string(&safe).unwrap(),
            ] {
                assert!(!serialized.contains(private_marker));
                assert!(!serialized.contains("gemini-unit-secret"));
            }
        }
    }

    #[test]
    fn provider_400_known_fields_become_safe_hints_and_survive_lifecycle() {
        let gemini_transport = FakeTransport::with_json(
            400,
            json!({
                "error": {
                    "status": "INVALID_ARGUMENT",
                    "message": "secret-model-value",
                    "details": [{
                        "fieldViolations": [
                            {"field": "responseFormat.imageSize"},
                            {"field": "mime_type"}
                        ]
                    }]
                },
                "credential": "gemini-secret"
            }),
        );
        let gemini_error =
            execute_with_transport(&gemini_transport, &prepared("gemini"), "gemini-unit-secret")
                .unwrap_err();

        assert_eq!(gemini_error.code, "ai_invalid_request");
        for field in ["image_size", "model", "response_format", "mime_type"] {
            assert!(gemini_error.message.contains(field));
        }
        assert!(!gemini_error.message.contains("secret-model-value"));
        assert!(!gemini_error.message.contains("gemini-secret"));
        let safe_gemini = safe_lifecycle_error(&gemini_error);
        assert_eq!(safe_gemini.code, "ai_invalid_request");
        assert_eq!(safe_gemini.message, gemini_error.message);

        let novelai_transport = FakeTransport::with_json(
            400,
            json!({
                "error": {"field": "action", "message": "bad model pst-secret"},
                "unknown_debug": "do-not-echo"
            }),
        );
        let novelai_error =
            execute_with_transport(&novelai_transport, &prepared("novelai"), "pst-unit-secret")
                .unwrap_err();

        assert_eq!(novelai_error.code, "ai_invalid_request");
        assert!(novelai_error.message.contains("model"));
        assert!(novelai_error.message.contains("action"));
        assert!(!novelai_error.message.contains("pst-secret"));
        assert!(!novelai_error.message.contains("do-not-echo"));
        let safe_novelai = safe_lifecycle_error(&novelai_error);
        assert_eq!(safe_novelai.message, novelai_error.message);
    }

    #[test]
    fn provider_400_unknown_body_uses_generic_redacted_hint() {
        let transport = FakeTransport::with_json(
            400,
            json!({"error": {"field": "prompt", "message": "private-secret"}}),
        );
        let error = execute_with_transport(&transport, &prepared("gemini"), "gemini-unit-secret")
            .unwrap_err();

        assert_eq!(error.code, "ai_invalid_request");
        assert_eq!(
            error.message,
            "AI 공급자가 요청 형식을 거부했습니다. 선택한 모델과 공급자 요청 옵션을 확인해 주세요."
        );
        assert!(!error.message.contains("prompt"));
        assert!(!error.message.contains("private-secret"));
        assert_eq!(safe_lifecycle_error(&error).message, error.message);
    }
    #[test]
    fn authorization_and_rate_limit_fail_once_without_echoing_secret() {
        for status in [401, 429] {
            let transport = FakeTransport::with_json(status, json!({"message":"secret"}));
            let error = execute_with_transport(&transport, &prepared("novelai"), "pst-do-not-echo")
                .unwrap_err();
            assert_eq!(transport.calls.lock().unwrap().len(), 1);
            assert!(!serde_json::to_string(&error)
                .unwrap()
                .contains("pst-do-not-echo"));
        }
    }

    #[test]
    fn schema_drift_and_oversized_transport_are_rejected_without_retry() {
        let transport = FakeTransport::with_json(201, json!({"images":[]}));
        assert_eq!(
            execute_with_transport(&transport, &prepared("novelai"), "pst-unit-secret")
                .unwrap_err()
                .code,
            "ai_response_schema"
        );
        assert_eq!(transport.calls.lock().unwrap().len(), 1);

        let transport = FakeTransport {
            calls: Mutex::new(Vec::new()),
            response: Mutex::new(Some(Err(TransportFailure::ResponseTooLarge))),
        };
        assert_eq!(
            execute_with_transport(&transport, &prepared("novelai"), "pst-unit-secret")
                .unwrap_err()
                .code,
            "ai_response_too_large"
        );
        assert_eq!(transport.calls.lock().unwrap().len(), 1);
    }

    static FIXTURE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    struct LifecycleFixture {
        connection: Connection,
        paths: AppPaths,
        collection_id: String,
        icon_id: String,
    }

    impl LifecycleFixture {
        fn cleanup(self) {
            let root = self.paths.root.clone();
            drop(self.connection);
            let _ = std::fs::remove_dir_all(root);
        }
    }

    fn lifecycle_fixture() -> LifecycleFixture {
        lifecycle_fixture_with_source("source.png", png_bytes([10, 20, 30, 255]))
    }

    fn lifecycle_fixture_with_source(original_filename: &str, bytes: Vec<u8>) -> LifecycleFixture {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let sequence = FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let paths = AppPaths::prepare(std::env::temp_dir().join(format!(
            "pmtconcon-provider-lifecycle-{}-{suffix}-{sequence}",
            std::process::id()
        )))
        .unwrap();
        let mut connection = open_database_with_paths(&paths.database_path, &paths).unwrap();
        let collection =
            collections::create_collection(&mut connection, Some("AI lifecycle".to_string()))
                .unwrap();
        let imported = imports::import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![ImportImageFilePayload {
                original_filename: original_filename.to_string(),
                bytes,
            }],
        )
        .unwrap();
        LifecycleFixture {
            connection,
            paths,
            collection_id: collection.id,
            icon_id: imported.imported_icons[0].id.clone(),
        }
    }

    fn png_bytes(color: [u8; 4]) -> Vec<u8> {
        let image = ImageBuffer::from_pixel(16, 16, Rgba(color));
        let mut cursor = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image)
            .write_to(&mut cursor, ImageFormat::Png)
            .unwrap();
        cursor.into_inner()
    }

    fn jpeg_bytes(color: [u8; 3]) -> Vec<u8> {
        let image = ImageBuffer::from_pixel(16, 16, Rgb(color));
        let mut cursor = Cursor::new(Vec::new());
        DynamicImage::ImageRgb8(image)
            .write_to(&mut cursor, ImageFormat::Jpeg)
            .unwrap();
        cursor.into_inner()
    }

    fn novel_payload(icon_id: &str) -> ExecuteAiImageEditPayload {
        ExecuteAiImageEditPayload {
            icon_id: icon_id.to_string(),
            provider: "novelai".to_string(),
            prompt: "bright sticker".to_string(),
            model: "nai-diffusion-3".to_string(),
            options: AiImageEditOptionsPayload {
                negative_prompt: Some("blur".to_string()),
                action: Some("img2img".to_string()),
                width: Some(1024),
                height: Some(1024),
                steps: Some(28),
                scale: Some(5.0),
                strength: Some(0.7),
                noise: Some(0.0),
            },
            consent: consent(),
        }
    }

    fn successful_novel_transport(bytes: &[u8]) -> FakeTransport {
        FakeTransport::with_json(
            201,
            json!({
                "images":[{
                    "image":BASE64_STANDARD.encode(bytes),
                    "index":0,
                    "seed":7
                }]
            }),
        )
    }

    fn request_status(
        connection: &Connection,
        request_id: &str,
    ) -> (String, Option<String>, Option<String>) {
        connection
            .query_row(
                "SELECT status, error_code, error_message FROM ai_requests WHERE id = ?1",
                [request_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap()
    }

    #[test]
    fn start_persists_running_request_with_exact_input_and_canonical_snapshots() {
        let mut fixture = lifecycle_fixture();
        let prepared = start_image_edit(
            &mut fixture.connection,
            &fixture.paths,
            &fixture.collection_id,
            novel_payload(&fixture.icon_id),
        )
        .unwrap();
        let row = fixture
            .connection
            .query_row(
                "SELECT r.status, r.input_package_sha256, r.payload_input_signature,
                        r.capability_snapshot_json, r.prompt_options_snapshot_json, s.sha256
                 FROM ai_requests r
                 JOIN icons i ON i.id = r.origin_icon_id
                 JOIN source_files s ON s.id = i.source_file_id
                 WHERE r.id = ?1",
                [prepared.request_id.as_str()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(row.0, "running");
        assert_eq!(row.1, row.5);
        assert_eq!(row.2.len(), 64);
        assert_eq!(
            row.3,
            ai_snapshots::canonicalize("capability", &row.3).unwrap()
        );
        assert_eq!(
            row.4,
            ai_snapshots::canonicalize("prompt_options", &row.4).unwrap()
        );
        fixture.cleanup();
    }

    #[test]
    fn novelai_jpeg_records_the_exact_transmitted_png_hash_before_network() {
        let jpeg = jpeg_bytes([11, 22, 33]);
        let source_sha256 = format!("{:x}", Sha256::digest(&jpeg));
        let mut fixture = lifecycle_fixture_with_source("source.jpg", jpeg);
        let prepared = start_image_edit(
            &mut fixture.connection,
            &fixture.paths,
            &fixture.collection_id,
            novel_payload(&fixture.icon_id),
        )
        .unwrap();
        let transmitted_sha256 = format!("{:x}", Sha256::digest(&prepared.input_bytes));
        let persisted_sha256: String = fixture
            .connection
            .query_row(
                "SELECT input_package_sha256 FROM ai_requests WHERE id = ?1",
                [prepared.request_id.as_str()],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(prepared.input_mime_type, "image/png");
        assert_eq!(
            image::guess_format(&prepared.input_bytes).unwrap(),
            ImageFormat::Png
        );
        assert_ne!(transmitted_sha256, source_sha256);
        assert_eq!(persisted_sha256, transmitted_sha256);
        fixture.cleanup();
    }

    #[test]
    fn successful_request_registers_candidate_and_completes_in_one_finalize() {
        let mut fixture = lifecycle_fixture();
        let prepared = start_image_edit(
            &mut fixture.connection,
            &fixture.paths,
            &fixture.collection_id,
            novel_payload(&fixture.icon_id),
        )
        .unwrap();
        let transport = successful_novel_transport(&png_bytes([90, 80, 70, 255]));
        let state = execute_started_with_transport(
            &mut fixture.connection,
            &fixture.paths,
            &prepared,
            "pst-test-secret",
            &transport,
        )
        .unwrap();
        assert_eq!(transport.calls.lock().unwrap().len(), 1);
        assert_eq!(state.candidates.len(), 1);
        assert_eq!(
            request_status(&fixture.connection, &prepared.request_id).0,
            "completed"
        );
        let counts = fixture
            .connection
            .query_row(
                "SELECT
                   (SELECT COUNT(*) FROM ai_candidates WHERE request_id = ?1),
                   (SELECT COUNT(*) FROM source_files)",
                [prepared.request_id.as_str()],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
            )
            .unwrap();
        assert_eq!(counts, (1, 2));
        fixture.cleanup();
    }

    #[test]
    fn provider_failure_is_bounded_failed_and_never_retried() {
        let mut fixture = lifecycle_fixture();
        let prepared = start_image_edit(
            &mut fixture.connection,
            &fixture.paths,
            &fixture.collection_id,
            novel_payload(&fixture.icon_id),
        )
        .unwrap();
        let transport = FakeTransport::with_json(401, json!({"body":"pst-test-secret"}));
        let error = execute_started_with_transport(
            &mut fixture.connection,
            &fixture.paths,
            &prepared,
            "pst-test-secret",
            &transport,
        )
        .unwrap_err();
        assert_eq!(error.code, "ai_unauthorized");
        assert_eq!(transport.calls.lock().unwrap().len(), 1);
        let status = request_status(&fixture.connection, &prepared.request_id);
        assert_eq!(status.0, "failed");
        assert_eq!(status.1.as_deref(), Some("ai_unauthorized"));
        assert!(!status.2.unwrap().contains("pst-test-secret"));
        let candidates: i64 = fixture
            .connection
            .query_row(
                "SELECT COUNT(*) FROM ai_candidates WHERE request_id = ?1",
                [prepared.request_id.as_str()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(candidates, 0);
        fixture.cleanup();
    }

    #[test]
    fn failure_cas_covers_pre_dispatch_and_dispatched_requests() {
        let mut fixture = lifecycle_fixture();
        let running = start_image_edit(
            &mut fixture.connection,
            &fixture.paths,
            &fixture.collection_id,
            novel_payload(&fixture.icon_id),
        )
        .unwrap();
        let dispatched = start_image_edit(
            &mut fixture.connection,
            &fixture.paths,
            &fixture.collection_id,
            novel_payload(&fixture.icon_id),
        )
        .unwrap();
        claim_started_request_for_dispatch(&fixture.connection, &dispatched.request_id).unwrap();
        let error = AppError::new("ai_provider_worker", "worker database open failed");

        assert!(
            record_started_request_failure(&fixture.connection, &running.request_id, &error,)
                .unwrap()
        );
        assert!(record_started_request_failure(
            &fixture.connection,
            &dispatched.request_id,
            &error,
        )
        .unwrap());
        assert_eq!(
            request_status(&fixture.connection, &running.request_id).0,
            "failed"
        );
        assert_eq!(
            request_status(&fixture.connection, &dispatched.request_id).0,
            "failed"
        );
        fixture.cleanup();
    }

    #[test]
    fn cancelled_and_stale_requests_never_store_candidates_or_artifacts() {
        let mut fixture = lifecycle_fixture();
        let cancelled = start_image_edit(
            &mut fixture.connection,
            &fixture.paths,
            &fixture.collection_id,
            novel_payload(&fixture.icon_id),
        )
        .unwrap();
        fixture
            .connection
            .execute(
                "UPDATE ai_requests SET status = 'cancelled' WHERE id = ?1",
                [cancelled.request_id.as_str()],
            )
            .unwrap();
        let transport = successful_novel_transport(&png_bytes([100, 1, 2, 255]));
        let error = execute_started_with_transport(
            &mut fixture.connection,
            &fixture.paths,
            &cancelled,
            "pst-test-secret",
            &transport,
        )
        .unwrap_err();
        assert_eq!(error.code, "ai_request_cancelled");
        assert_eq!(transport.calls.lock().unwrap().len(), 0);
        assert_eq!(
            request_status(&fixture.connection, &cancelled.request_id).0,
            "cancelled"
        );

        let stale = start_image_edit(
            &mut fixture.connection,
            &fixture.paths,
            &fixture.collection_id,
            novel_payload(&fixture.icon_id),
        )
        .unwrap();
        fixture
            .connection
            .execute(
                "UPDATE icon_ai_state SET revision = revision + 1 WHERE icon_id = ?1",
                [fixture.icon_id.as_str()],
            )
            .unwrap();
        let transport = successful_novel_transport(&png_bytes([3, 4, 5, 255]));
        let error = execute_started_with_transport(
            &mut fixture.connection,
            &fixture.paths,
            &stale,
            "pst-test-secret",
            &transport,
        )
        .unwrap_err();
        assert_eq!(error.code, "ai_request_stale");
        assert_eq!(transport.calls.lock().unwrap().len(), 1);
        assert_eq!(
            request_status(&fixture.connection, &stale.request_id).0,
            "failed"
        );
        let counts = fixture
            .connection
            .query_row(
                "SELECT (SELECT COUNT(*) FROM ai_candidates), (SELECT COUNT(*) FROM source_files)",
                [],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
            )
            .unwrap();
        assert_eq!(counts, (0, 1));
        fixture.cleanup();
    }

    #[test]
    fn finalize_decode_failure_is_recorded_without_candidate() {
        let mut fixture = lifecycle_fixture();
        let prepared = start_image_edit(
            &mut fixture.connection,
            &fixture.paths,
            &fixture.collection_id,
            novel_payload(&fixture.icon_id),
        )
        .unwrap();
        let transport = successful_novel_transport(b"not-an-image");
        let error = execute_started_with_transport(
            &mut fixture.connection,
            &fixture.paths,
            &prepared,
            "pst-test-secret",
            &transport,
        )
        .unwrap_err();
        assert_eq!(error.code, "ai_finalize_failed");
        assert_eq!(
            request_status(&fixture.connection, &prepared.request_id).0,
            "failed"
        );
        let counts = fixture
            .connection
            .query_row(
                "SELECT (SELECT COUNT(*) FROM ai_candidates), (SELECT COUNT(*) FROM source_files)",
                [],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
            )
            .unwrap();
        assert_eq!(counts, (0, 1));
        fixture.cleanup();
    }

    #[test]
    fn startup_recovery_fails_running_and_dispatched_session_api_requests() {
        let mut fixture = lifecycle_fixture();
        let prepared = start_image_edit(
            &mut fixture.connection,
            &fixture.paths,
            &fixture.collection_id,
            novel_payload(&fixture.icon_id),
        )
        .unwrap();
        let dispatched = start_image_edit(
            &mut fixture.connection,
            &fixture.paths,
            &fixture.collection_id,
            novel_payload(&fixture.icon_id),
        )
        .unwrap();
        claim_started_request_for_dispatch(&fixture.connection, &dispatched.request_id).unwrap();

        assert_eq!(
            recover_interrupted_session_requests(&fixture.connection).unwrap(),
            2
        );
        for request_id in [&prepared.request_id, &dispatched.request_id] {
            let status = request_status(&fixture.connection, request_id);
            assert_eq!(status.0, "failed");
            assert_eq!(status.1.as_deref(), Some("ai_request_interrupted"));
        }
        assert_eq!(
            recover_interrupted_session_requests(&fixture.connection).unwrap(),
            0
        );
        fixture.cleanup();
    }

    fn gemini_payload(icon_id: &str) -> ExecuteAiImageEditPayload {
        ExecuteAiImageEditPayload {
            icon_id: icon_id.to_string(),
            provider: "gemini".to_string(),
            prompt: "bright sticker".to_string(),
            model: "gemini-2.5-flash-image".to_string(),
            options: AiImageEditOptionsPayload::default(),
            consent: consent(),
        }
    }

    #[test]
    fn gemini_finalize_persists_safe_provider_request_id_and_usage() {
        let mut fixture = lifecycle_fixture();
        let prepared = start_image_edit(
            &mut fixture.connection,
            &fixture.paths,
            &fixture.collection_id,
            gemini_payload(&fixture.icon_id),
        )
        .unwrap();
        let encoded = BASE64_STANDARD.encode(jpeg_bytes([7, 8, 9]));
        let transport = FakeTransport::with_json(
            200,
            json!({
                "id":"interaction_safe_123",
                "status":"completed",
                "steps":[{
                    "type":"model_output",
                    "content":[{"type":"image","mime_type":"image/jpeg","data":encoded}]
                }],
                "usage":{"total_tokens":12}
            }),
        );
        execute_started_with_transport(
            &mut fixture.connection,
            &fixture.paths,
            &prepared,
            "gemini-test-secret",
            &transport,
        )
        .unwrap();
        let row = fixture
            .connection
            .query_row(
                "SELECT provider_request_id, provider_usage_json
                 FROM ai_requests WHERE id = ?1",
                [prepared.request_id.as_str()],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .unwrap();
        assert_eq!(row.0, "interaction_safe_123");
        assert_eq!(
            serde_json::from_str::<Value>(&row.1).unwrap()["total_tokens"],
            12
        );
        fixture.cleanup();
    }

    #[test]
    fn gemini_rejects_unsafe_provider_request_id_without_retry() {
        let encoded = BASE64_STANDARD.encode(b"png");
        let transport = FakeTransport::with_json(
            200,
            json!({
                "id":"unsafe\nrequest",
                "status":"completed",
                "steps":[{
                    "type":"model_output",
                    "content":[{"type":"image","mime_type":"image/jpeg","data":encoded}]
                }]
            }),
        );
        let error = execute_with_transport(&transport, &prepared("gemini"), "gemini-test-secret")
            .unwrap_err();
        assert_eq!(error.code, "ai_response_schema");
        assert_eq!(transport.calls.lock().unwrap().len(), 1);
    }
}
