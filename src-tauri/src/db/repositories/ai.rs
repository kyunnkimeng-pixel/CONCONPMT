use std::fs::{self, File};
use std::io::Read;
use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::db::repositories::ai_activation;
use crate::db::repositories::ai_candidate_normalization;
use crate::db::repositories::ai_new_icon;
use crate::db::repositories::ai_snapshots;
use crate::db::repositories::icons;
use crate::db::repositories::source_files::{
    commit_prepared_source_file, ensure_source_file_has_alpha, prepare_source_file_from_bytes,
    SourceFileImportOptions,
};
use crate::error::{AppError, AppResult};
use crate::ids::create_id;
use crate::models::{
    ActivateAiCandidatePayload, AiCandidateDto, AiCandidateUsageSummaryDto,
    AiNormalizationCompatibilityDto, AiNormalizationPreviewDto, AiNormalizationSummaryDto,
    AiReviewStateDto, AiSourceMutationResultDto, AiVersionDto, CreateAiIconRootPayload,
    CreateAiIconRootResultDto, EffectiveVisualSourceDto, ImportAiCandidatePayload,
    PreviewAiCandidateNormalizationPayload, RepairAiToOriginalPayload, RestoreAiVersionPayload,
    SourceFileDto,
};
use crate::optimization::cache::hash_text;
use crate::paths::AppPaths;

pub const MAX_AI_CANDIDATE_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone)]
pub(crate) struct VisualSourceRecord {
    pub id: String,
    pub original_filename: String,
    pub path: String,
    pub extension: String,
    pub mime_type: String,
    pub width: i64,
    pub height: i64,
    pub byte_size: i64,
    pub sha256: String,
    pub has_alpha: bool,
    pub is_animated: bool,
    pub frame_count: Option<i64>,
    pub original_loop_mode: String,
    pub original_loop_count: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct EffectiveVisualSource {
    pub icon_id: String,
    pub original_source: VisualSourceRecord,
    pub render_source: VisualSourceRecord,
    pub original_lineage_id: String,
    pub original_lineage_generation: i64,
    pub active_version_id: Option<String>,
    pub active_candidate_id: Option<String>,
    pub activation_revision: i64,
    pub normalization_recipe_hash: Option<String>,
}

pub fn resolve_effective_visual_source(
    connection: &Connection,
    collection_id: &str,
    icon_id: &str,
) -> AppResult<EffectiveVisualSource> {
    let icon_exists = connection
        .query_row(
            "SELECT 1
             FROM icons i
             JOIN collections c ON c.id = i.collection_id
             WHERE i.id = ?1
               AND i.collection_id = ?2
               AND i.deleted_at IS NULL
               AND c.deleted_at IS NULL",
            params![icon_id, collection_id],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    if !icon_exists {
        return Err(AppError::not_found(
            "AI 소스를 확인할 아이콘을 찾을 수 없습니다.",
        ));
    }

    let projection = connection
        .query_row(
            "SELECT
               original_source_file_id,
               effective_source_file_id,
               original_lineage_id,
               original_lineage_generation,
               active_version_id,
               active_candidate_id,
               activation_revision,
               normalization_recipe_hash
             FROM effective_visual_sources
             WHERE icon_id = ?1
               AND collection_id = ?2",
            params![icon_id, collection_id],
            |row| {
                Ok((
                    row.get::<_, String>("original_source_file_id")?,
                    row.get::<_, String>("effective_source_file_id")?,
                    row.get::<_, String>("original_lineage_id")?,
                    row.get::<_, i64>("original_lineage_generation")?,
                    row.get::<_, Option<String>>("active_version_id")?,
                    row.get::<_, Option<String>>("active_candidate_id")?,
                    row.get::<_, i64>("activation_revision")?,
                    row.get::<_, Option<String>>("normalization_recipe_hash")?,
                ))
            },
        )
        .optional()?
        .ok_or_else(ai_repair_required)?;

    let original_source = load_and_validate_source(connection, &projection.0)?;
    let render_source = if projection.0 == projection.1 {
        original_source.clone()
    } else {
        load_and_validate_source(connection, &projection.1)?
    };

    Ok(EffectiveVisualSource {
        icon_id: icon_id.to_string(),
        original_source,
        render_source,
        original_lineage_id: projection.2,
        original_lineage_generation: projection.3,
        active_version_id: projection.4,
        active_candidate_id: projection.5,
        activation_revision: projection.6,
        normalization_recipe_hash: projection.7,
    })
}

pub fn cleanup_ai_crash_orphans(connection: &Connection, paths: &AppPaths) -> AppResult<usize> {
    ai_activation::cleanup_crash_orphans(connection, paths)
}

pub fn get_ai_review_state(
    connection: &Connection,
    collection_id: &str,
    icon_id: &str,
) -> AppResult<AiReviewStateDto> {
    let visual_source = resolve_effective_visual_source(connection, collection_id, icon_id)?;
    let native_recipe_signature = ai_activation::current_recipe_signature(
        connection,
        collection_id,
        icon_id,
        &visual_source.render_source,
        visual_source.activation_revision,
    )?;
    Ok(AiReviewStateDto {
        candidates: list_candidates(connection, collection_id, &visual_source)?,
        versions: list_versions(connection, &visual_source)?,
        visual_source: effective_source_dto(&visual_source),
        native_recipe_signature,
    })
}

pub fn import_local_ai_candidate(
    connection: &mut Connection,
    paths: &AppPaths,
    collection_id: &str,
    payload: ImportAiCandidatePayload,
) -> AppResult<AiReviewStateDto> {
    if payload.file.bytes.len() > MAX_AI_CANDIDATE_BYTES {
        return Err(AppError::new(
            "ai_candidate_too_large",
            "AI 후보 이미지는 최대 16MB까지 가져올 수 있습니다. 더 작은 파일로 다시 시도해 주세요.",
        ));
    }

    let service_surface = validate_manual_service_surface(&payload.service_surface)?;
    let current = resolve_effective_visual_source(connection, collection_id, &payload.icon_id)?;
    let request_recipe_signature = ai_activation::current_recipe_signature(
        connection,
        collection_id,
        &payload.icon_id,
        &current.render_source,
        current.activation_revision,
    )?;
    let request_id = create_id("ai_request");
    let candidate_id = create_id("ai_candidate");
    let prepared = prepare_source_file_from_bytes(
        &payload.file,
        SourceFileImportOptions {
            allow_gif: false,
            exact_dimensions: None,
        },
    )?;
    let source_artifact_snapshot = prepared.artifact_snapshot(connection, paths)?;
    let import_result = (|| -> AppResult<()> {
        let transaction = connection.transaction()?;
        let stored = commit_prepared_source_file(&transaction, paths, &prepared)?;
        let has_alpha = stored.has_alpha.ok_or_else(|| {
            AppError::new(
                "ai_candidate_metadata",
                "AI 후보의 알파 정보를 확인할 수 없습니다.",
            )
        })?;
        let provider = provider_for_surface(service_surface);
        let capability_snapshot = ai_snapshots::canonicalize(
            "capability",
            &json!({
                "schema": "pmtcon-ai-capability-v1",
                "provider": provider,
                "serviceSurface": service_surface,
                "source": "manual-result-import",
                "supports": ["image-output"]
            })
            .to_string(),
        )?;
        let data_tier_snapshot = ai_snapshots::canonicalize(
            "data_tier",
            r#"{"schema":"pmtcon-ai-data-tier-v1","source":"user-declared","tier":"unknown"}"#,
        )?;
        let retention_snapshot = ai_snapshots::canonicalize(
            "retention",
            r#"{"schema":"pmtcon-ai-retention-v1","source":"user-declared","retention":"unknown"}"#,
        )?;
        let consent_snapshot = ai_snapshots::canonicalize(
            "consent",
            r#"{"schema":"pmtcon-ai-consent-v1","confirmed":true,"source":"local-import"}"#,
        )?;
        let policy_refs = ai_snapshots::canonicalize("policy_refs", "[]")?;
        let prompt_options = ai_snapshots::canonicalize(
            "prompt_options",
            r#"{"schema":"pmtcon-ai-prompt-options-v1","importedResultOnly":true}"#,
        )?;
        let payload_input_signature = hash_text(&[
            "pmtcon-manual-result-v1".to_string(),
            current.original_lineage_id.clone(),
            current.original_lineage_generation.to_string(),
            current.render_source.sha256.clone(),
            stored.sha256.clone(),
        ]);

        let request_rows = transaction.execute(
            "INSERT INTO ai_requests (
           id, origin_collection_id, origin_icon_id,
           origin_collection_name_snapshot, origin_icon_name_snapshot,
           provider_mode, service_surface, provider, adapter_id,
           adapter_contract_version, account_context, model, operation,
           provenance_trust, credential_mode_snapshot,
           capability_snapshot_json, data_tier_snapshot_json,
           retention_snapshot_json, consent_snapshot_json, policy_refs_json,
           prompt_options_snapshot_json, original_lineage_id,
           original_lineage_generation, original_source_sha256,
           effective_source_sha256, payload_input_signature,
           request_recipe_signature, activation_revision, status,
           completed_at, created_at, updated_at
         )
         SELECT
           ?1, c.id, i.id, c.name, i.display_name,
           'manual_web', ?2, ?3, 'pmtcon-manual-result',
           '1', 'unknown', NULL, 'image_edit_result_import',
           'manual_unverified', 'none',
           ?4, ?5, ?6, ?7, ?8, ?9,
           ?10, ?11, ?12, ?13, ?14, ?15, ?16, 'completed',
           strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
           strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
           strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         FROM icons i
         JOIN collections c ON c.id = i.collection_id
         WHERE i.id = ?17
           AND i.collection_id = ?18
           AND i.deleted_at IS NULL
           AND c.deleted_at IS NULL",
            params![
                request_id,
                service_surface,
                provider,
                capability_snapshot,
                data_tier_snapshot,
                retention_snapshot,
                consent_snapshot,
                policy_refs,
                prompt_options,
                current.original_lineage_id,
                current.original_lineage_generation,
                current.original_source.sha256,
                current.render_source.sha256,
                payload_input_signature,
                request_recipe_signature,
                current.activation_revision,
                payload.icon_id,
                collection_id,
            ],
        )?;
        if request_rows != 1 {
            return Err(AppError::not_found(
                "AI 후보를 연결할 아이콘을 찾을 수 없습니다.",
            ));
        }
        transaction.execute(
            "INSERT INTO ai_candidates (
           id, request_id, candidate_index, raw_source_file_id,
           raw_source_sha256, output_format, width, height, is_animated,
           has_alpha, provider_capabilities_snapshot_json, created_at
         )
         VALUES (
           ?1, ?2, 0, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
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
                i64::from(stored.is_animated),
                i64::from(has_alpha),
                capability_snapshot,
            ],
        )?;
        transaction.commit()?;
        Ok(())
    })();
    if let Err(error) = import_result {
        let _ = source_artifact_snapshot.cleanup_if_unreferenced(connection);
        return Err(error);
    }
    get_ai_review_state(connection, collection_id, &payload.icon_id)
}

pub fn preview_ai_candidate_normalization(
    connection: &Connection,
    paths: &AppPaths,
    collection_id: &str,
    payload: PreviewAiCandidateNormalizationPayload,
) -> AppResult<AiNormalizationPreviewDto> {
    let operation_id = create_id("ai_normalization_preview");
    let operation_dir = paths.ai_activation_staging_dir.join(operation_id);
    let render_dir = operation_dir.join("render");
    let result = (|| {
        let normalization = ai_candidate_normalization::prepare_candidate_normalization(
            connection,
            paths,
            collection_id,
            &payload.icon_id,
            &payload.candidate_id,
            payload.expected_revision,
            &payload.normalization,
            &operation_dir,
        )?;
        let current = resolve_effective_visual_source(connection, collection_id, &payload.icon_id)?;
        let prepared_render = ai_activation::prepare_source_preview(
            connection,
            collection_id,
            &payload.icon_id,
            &payload.icon_id,
            &normalization.effective_source,
            &current.render_source,
            current.activation_revision,
            &current.original_lineage_id,
            current.original_lineage_generation,
            &normalization.native_recipe_signature,
        )?;
        let generated =
            ai_activation::render_prepared_source_preview(&render_dir, &prepared_render)?;
        let post_render =
            resolve_effective_visual_source(connection, collection_id, &payload.icon_id)?;
        let post_render_signature = ai_activation::current_recipe_signature(
            connection,
            collection_id,
            &payload.icon_id,
            &post_render.render_source,
            post_render.activation_revision,
        )?;
        if post_render_signature != normalization.native_recipe_signature {
            return Err(AppError::new(
                "ai_normalization_preview_stale",
                "미리보기 중 편집값이 변경되었습니다. 현재 설정으로 다시 확인해 주세요.",
            ));
        }
        let inspection =
            ai_activation::inspect_prepared_source_preview(&generated, &prepared_render)?;
        let current_icon_compatibility = output_size_compatibility(
            &normalization.current_icon_compatibility,
            "ai_current_icon_output_too_large",
            "현재 아이콘에 적용",
            &inspection,
        );
        let new_icon_compatibility = output_size_compatibility(
            &normalization.new_icon_compatibility,
            "ai_new_icon_output_too_large",
            "새 아이콘으로 추가",
            &inspection,
        );
        Ok(AiNormalizationPreviewDto {
            candidate_id: normalization.candidate_id.clone(),
            raw_source: ai_candidate_normalization::raw_source_dto(&normalization),
            normalized_preview_path: normalization
                .normalized_preview_path
                .to_string_lossy()
                .to_string(),
            final_preview_path: generated.current_preview_path.to_string_lossy().to_string(),
            target_canvas_width: normalization.effective_source.width,
            target_canvas_height: normalization.effective_source.height,
            final_render_width: inspection.final_render_width,
            final_render_height: inspection.final_render_height,
            piece_width: inspection.piece_width,
            piece_height: inspection.piece_height,
            normalization_recipe_hash: normalization.normalization_recipe_hash.clone(),
            preview_signature: normalization.preview_signature.clone(),
            native_recipe_signature: normalization.native_recipe_signature.clone(),
            geometry: normalization.geometry_dto(),
            normalized_has_alpha: normalization.normalized_has_alpha,
            current_icon_compatibility,
            new_icon_compatibility,
            warnings: normalization.warnings.clone(),
            existing_version_id: normalization.existing_version_id.clone(),
            is_current_recipe: normalization.is_current_recipe,
        })
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&operation_dir);
    }
    result
}
pub fn activate_ai_candidate(
    connection: &mut Connection,
    paths: &AppPaths,
    collection_id: &str,
    payload: ActivateAiCandidatePayload,
) -> AppResult<AiSourceMutationResultDto> {
    ai_activation::activate_candidate(connection, paths, collection_id, &payload)
}

pub fn create_ai_icon_root(
    connection: &mut Connection,
    paths: &AppPaths,
    collection_id: &str,
    payload: CreateAiIconRootPayload,
) -> AppResult<CreateAiIconRootResultDto> {
    ai_new_icon::create_ai_icon_root(connection, paths, collection_id, &payload)
}

pub fn restore_ai_version(
    connection: &mut Connection,
    paths: &AppPaths,
    collection_id: &str,
    payload: RestoreAiVersionPayload,
) -> AppResult<AiSourceMutationResultDto> {
    ai_activation::restore_version(connection, paths, collection_id, &payload)
}

pub fn repair_ai_to_original(
    connection: &mut Connection,
    paths: &AppPaths,
    collection_id: &str,
    payload: RepairAiToOriginalPayload,
) -> AppResult<AiReviewStateDto> {
    ai_activation::repair_to_original(connection, paths, collection_id, &payload)?;
    get_ai_review_state(connection, collection_id, &payload.icon_id)
}

fn list_candidates(
    connection: &Connection,
    collection_id: &str,
    visual_source: &EffectiveVisualSource,
) -> AppResult<Vec<AiCandidateDto>> {
    let mut statement = connection.prepare(
        "SELECT
           candidate.id,
           candidate.request_id,
           candidate.candidate_index,
           request.service_surface,
           candidate.raw_source_file_id,
           candidate.created_at,
           EXISTS (
             SELECT 1 FROM icon_ai_versions version
             WHERE version.icon_id = ?1
               AND version.candidate_id = candidate.id
               AND version.base_original_lineage_id = ?2
               AND version.base_original_lineage_generation = ?3
           ) AS is_materialized,
           COALESCE(
             (
               request_item.id IS NOT NULL
               AND request_item.origin_icon_id = ?1
               AND request.origin_collection_id = ?4
             ) OR (
               request_item.id IS NULL
               AND request.origin_icon_id = ?1
               AND request.origin_collection_id = ?4
             ),
             0
           ) AS is_direct_origin,
           EXISTS (
             SELECT 1
             FROM ai_icon_root_creations root_creation
             WHERE root_creation.icon_id = ?1
               AND root_creation.candidate_id = candidate.id
           ) AS is_owned_root
         FROM ai_candidates candidate
         JOIN ai_requests request ON request.id = candidate.request_id
         LEFT JOIN ai_request_items request_item
           ON request_item.id = candidate.request_item_id
         WHERE (
           request_item.id IS NOT NULL
           AND request_item.origin_icon_id = ?1
           AND request.origin_collection_id = ?4
         ) OR (
           request_item.id IS NULL
           AND request.origin_icon_id = ?1
           AND request.origin_collection_id = ?4
         ) OR EXISTS (
           SELECT 1
           FROM icon_ai_versions owned_version
           WHERE owned_version.icon_id = ?1
             AND owned_version.candidate_id = candidate.id
         ) OR EXISTS (
           SELECT 1
           FROM ai_icon_root_creations owned_root
           WHERE owned_root.icon_id = ?1
             AND owned_root.candidate_id = candidate.id
         )
         ORDER BY candidate.created_at DESC, candidate.candidate_index ASC",
    )?;
    let rows = statement
        .query_map(
            params![
                visual_source.icon_id,
                visual_source.original_lineage_id,
                visual_source.original_lineage_generation,
                collection_id,
            ],
            |row| {
                Ok((
                    row.get::<_, String>("id")?,
                    row.get::<_, String>("request_id")?,
                    row.get::<_, i64>("candidate_index")?,
                    row.get::<_, String>("service_surface")?,
                    row.get::<_, String>("raw_source_file_id")?,
                    row.get::<_, String>("created_at")?,
                    row.get::<_, i64>("is_materialized")? != 0,
                    row.get::<_, i64>("is_direct_origin")? != 0,
                    row.get::<_, i64>("is_owned_root")? != 0,
                ))
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;
    let mut candidates = Vec::with_capacity(rows.len());
    for row in rows {
        let (source, is_available, unavailable_reason) = load_review_source(connection, &row.4)?;
        let created_icon_usage = candidate_created_icon_usage(connection, collection_id, &row.0)?;
        let stale_reason = if row.6 || row.8 {
            None
        } else if !row.7 {
            Some(
                "이 후보는 복제된 이전 AI 계보에 속합니다. 현재 아이콘에는 적용할 수 없지만 새 아이콘으로 추가할 수 있습니다."
                    .to_string(),
            )
        } else {
            ai_activation::candidate_stale_reason(
                connection,
                collection_id,
                &visual_source.icon_id,
                &row.0,
                visual_source,
            )?
        };
        candidates.push(AiCandidateDto {
            id: row.0,
            request_id: row.1,
            candidate_index: row.2,
            service_surface: row.3,
            source,
            is_available,
            unavailable_reason,
            created_at: row.5,
            is_materialized: row.6,
            created_icon_usage,
            is_stale: stale_reason.is_some(),
            stale_reason,
        });
    }
    Ok(candidates)
}

pub(crate) fn candidate_created_icon_usage(
    connection: &Connection,
    collection_id: &str,
    candidate_id: &str,
) -> AppResult<AiCandidateUsageSummaryDto> {
    let created_icon_count = connection.query_row(
        "SELECT COUNT(*)
         FROM ai_icon_root_creations creation
         JOIN icons icon ON icon.id = creation.icon_id
         WHERE creation.candidate_id = ?1
           AND creation.creation_kind <> 'clone'
           AND icon.collection_id = ?2
           AND icon.deleted_at IS NULL",
        params![candidate_id, collection_id],
        |row| row.get::<_, i64>(0),
    )?;
    let latest_icon_id = connection
        .query_row(
            "SELECT creation.icon_id
             FROM ai_icon_root_creations creation
             JOIN icons icon ON icon.id = creation.icon_id
             WHERE creation.candidate_id = ?1
               AND creation.creation_kind <> 'clone'
               AND icon.collection_id = ?2
               AND icon.deleted_at IS NULL
             ORDER BY creation.creation_order DESC
             LIMIT 1",
            params![candidate_id, collection_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    let latest_created_icon = latest_icon_id
        .map(|icon_id| icons::get_icon(connection, collection_id, &icon_id))
        .transpose()?;
    Ok(AiCandidateUsageSummaryDto {
        created_icon_count,
        latest_created_icon,
    })
}

fn list_versions(
    connection: &Connection,
    visual_source: &EffectiveVisualSource,
) -> AppResult<Vec<AiVersionDto>> {
    let mut statement = connection.prepare(
        "SELECT id, candidate_id, parent_version_id, effective_source_file_id,
                normalization_recipe_json, normalization_recipe_hash, created_at
         FROM icon_ai_versions
         WHERE icon_id = ?1
           AND base_original_lineage_id = ?2
           AND base_original_lineage_generation = ?3
         ORDER BY created_at DESC, id DESC",
    )?;
    let rows = statement
        .query_map(
            params![
                visual_source.icon_id,
                visual_source.original_lineage_id,
                visual_source.original_lineage_generation,
            ],
            |row| {
                Ok((
                    row.get::<_, String>("id")?,
                    row.get::<_, String>("candidate_id")?,
                    row.get::<_, Option<String>>("parent_version_id")?,
                    row.get::<_, String>("effective_source_file_id")?,
                    row.get::<_, String>("normalization_recipe_json")?,
                    row.get::<_, String>("normalization_recipe_hash")?,
                    row.get::<_, String>("created_at")?,
                ))
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;
    let mut versions = Vec::with_capacity(rows.len());
    for row in rows {
        let (source, is_available, unavailable_reason) = load_review_source(connection, &row.3)?;
        versions.push(AiVersionDto {
            is_active: visual_source.active_version_id.as_deref() == Some(row.0.as_str()),
            id: row.0,
            candidate_id: row.1,
            parent_version_id: row.2,
            source,
            is_available,
            unavailable_reason,
            normalization_summary: normalization_summary(&row.4),
            normalization_recipe_hash: row.5,
            created_at: row.6,
        });
    }
    Ok(versions)
}

pub(crate) fn load_and_validate_source(
    connection: &Connection,
    source_file_id: &str,
) -> AppResult<VisualSourceRecord> {
    let source = load_source_metadata(connection, source_file_id)?;
    validate_source_record(connection, source).map_err(|_| ai_repair_required())
}

fn load_source_metadata(
    connection: &Connection,
    source_file_id: &str,
) -> AppResult<RawSourceRecord> {
    connection
        .query_row(
            "SELECT
               id, original_filename, original_path_in_library, original_extension,
               mime_type, width, height, byte_size, sha256, has_alpha,
               is_animated, frame_count,
               COALESCE(original_loop_mode, 'preserve') AS original_loop_mode,
               original_loop_count
             FROM source_files
             WHERE id = ?1",
            [source_file_id],
            raw_source_from_row,
        )
        .optional()?
        .ok_or_else(ai_repair_required)
}

fn load_review_source(
    connection: &Connection,
    source_file_id: &str,
) -> AppResult<(SourceFileDto, bool, Option<String>)> {
    let metadata = load_source_metadata(connection, source_file_id)?;
    match validate_source_record(connection, metadata.clone()) {
        Ok(source) => Ok((source_dto(&source), true, None)),
        Err(reason) => Ok((source_metadata_dto(&metadata), false, Some(reason.message))),
    }
}

fn validate_source_record(
    connection: &Connection,
    mut source: RawSourceRecord,
) -> Result<VisualSourceRecord, SourceUnavailableReason> {
    let path = Path::new(&source.path);
    if !path.is_file() {
        return Err(SourceUnavailableReason::new(
            "저장된 AI 소스 파일을 찾을 수 없어 이 항목을 사용할 수 없습니다.",
        ));
    }
    let metadata = fs::metadata(path).map_err(|_| {
        SourceUnavailableReason::new(
            "저장된 AI 소스 파일 정보를 읽을 수 없어 이 항목을 사용할 수 없습니다.",
        )
    })?;
    let expected_size = u64::try_from(source.byte_size).map_err(|_| {
        SourceUnavailableReason::new(
            "저장된 AI 소스의 파일 크기 기록이 올바르지 않아 이 항목을 사용할 수 없습니다.",
        )
    })?;
    if metadata.len() != expected_size {
        return Err(SourceUnavailableReason::new(
            "저장된 AI 소스 파일 크기가 기록과 일치하지 않아 이 항목을 사용할 수 없습니다.",
        ));
    }
    let actual_sha256 = sha256_file(path).map_err(|_| {
        SourceUnavailableReason::new(
            "저장된 AI 소스 파일의 무결성을 확인할 수 없어 이 항목을 사용할 수 없습니다.",
        )
    })?;
    if actual_sha256 != source.sha256 {
        return Err(SourceUnavailableReason::new(
            "저장된 AI 소스 파일의 해시가 기록과 일치하지 않아 이 항목을 사용할 수 없습니다.",
        ));
    }
    let dimensions = image::image_dimensions(path).map_err(|_| {
        SourceUnavailableReason::new(
            "저장된 AI 소스 이미지를 디코드할 수 없어 이 항목을 사용할 수 없습니다.",
        )
    })?;
    if i64::from(dimensions.0) != source.width || i64::from(dimensions.1) != source.height {
        return Err(SourceUnavailableReason::new(
            "저장된 AI 소스 이미지 크기가 기록과 일치하지 않아 이 항목을 사용할 수 없습니다.",
        ));
    }
    if source.has_alpha.is_none() {
        source.has_alpha = Some(
            ensure_source_file_has_alpha(connection, &source.id, path, &source.extension).map_err(
                |_| {
                    SourceUnavailableReason::new(
                        "저장된 AI 소스의 투명도 정보를 검증할 수 없어 이 항목을 사용할 수 없습니다.",
                    )
                },
            )?,
        );
    }
    Ok(VisualSourceRecord {
        id: source.id,
        original_filename: source.original_filename,
        path: source.path,
        extension: source.extension,
        mime_type: source.mime_type,
        width: source.width,
        height: source.height,
        byte_size: source.byte_size,
        sha256: source.sha256,
        has_alpha: source.has_alpha.unwrap_or(false),
        is_animated: source.is_animated,
        frame_count: source.frame_count,
        original_loop_mode: source.original_loop_mode,
        original_loop_count: source.original_loop_count,
    })
}

#[derive(Debug, Clone)]
struct RawSourceRecord {
    id: String,
    original_filename: String,
    path: String,
    extension: String,
    mime_type: String,
    width: i64,
    height: i64,
    byte_size: i64,
    sha256: String,
    has_alpha: Option<bool>,
    is_animated: bool,
    frame_count: Option<i64>,
    original_loop_mode: String,
    original_loop_count: Option<i64>,
}

#[derive(Debug)]
struct SourceUnavailableReason {
    message: String,
}

impl SourceUnavailableReason {
    fn new(message: &str) -> Self {
        Self {
            message: message.to_string(),
        }
    }
}

fn raw_source_from_row(row: &Row<'_>) -> rusqlite::Result<RawSourceRecord> {
    Ok(RawSourceRecord {
        id: row.get("id")?,
        original_filename: row.get("original_filename")?,
        path: row.get("original_path_in_library")?,
        extension: row.get("original_extension")?,
        mime_type: row.get("mime_type")?,
        width: row.get("width")?,
        height: row.get("height")?,
        byte_size: row.get("byte_size")?,
        sha256: row.get("sha256")?,
        has_alpha: row
            .get::<_, Option<i64>>("has_alpha")?
            .map(|value| value != 0),
        is_animated: row.get::<_, i64>("is_animated")? != 0,
        frame_count: row.get("frame_count")?,
        original_loop_mode: row.get("original_loop_mode")?,
        original_loop_count: row.get("original_loop_count")?,
    })
}

fn source_metadata_dto(source: &RawSourceRecord) -> SourceFileDto {
    SourceFileDto {
        id: source.id.clone(),
        original_filename: source.original_filename.clone(),
        original_image_url: source.path.clone(),
        original_extension: source.extension.clone(),
        mime_type: source.mime_type.clone(),
        sha256: source.sha256.clone(),
        has_alpha: source.has_alpha,
        width: source.width,
        height: source.height,
        byte_size: source.byte_size,
        is_animated: source.is_animated,
        frame_count: source.frame_count,
        original_loop_mode: source.original_loop_mode.clone(),
        original_loop_count: source.original_loop_count,
    }
}

pub(crate) fn source_dto(source: &VisualSourceRecord) -> SourceFileDto {
    SourceFileDto {
        id: source.id.clone(),
        original_filename: source.original_filename.clone(),
        original_image_url: source.path.clone(),
        original_extension: source.extension.clone(),
        mime_type: source.mime_type.clone(),
        sha256: source.sha256.clone(),
        has_alpha: Some(source.has_alpha),
        width: source.width,
        height: source.height,
        byte_size: source.byte_size,
        is_animated: source.is_animated,
        frame_count: source.frame_count,
        original_loop_mode: source.original_loop_mode.clone(),
        original_loop_count: source.original_loop_count,
    }
}

pub(crate) fn effective_source_dto(source: &EffectiveVisualSource) -> EffectiveVisualSourceDto {
    EffectiveVisualSourceDto {
        original_source: source_dto(&source.original_source),
        effective_render_source: source_dto(&source.render_source),
        original_lineage_id: source.original_lineage_id.clone(),
        original_lineage_generation: source.original_lineage_generation,
        active_version_id: source.active_version_id.clone(),
        active_candidate_id: source.active_candidate_id.clone(),
        activation_revision: source.activation_revision,
        normalization_recipe_hash: source.normalization_recipe_hash.clone(),
    }
}

fn output_size_compatibility(
    compatibility: &AiNormalizationCompatibilityDto,
    reason_code: &str,
    action_label: &str,
    inspection: &ai_activation::RenderedPreviewInspection,
) -> AiNormalizationCompatibilityDto {
    if !compatibility.allowed || inspection.largest_piece_bytes <= inspection.max_piece_bytes {
        return compatibility.clone();
    }
    AiNormalizationCompatibilityDto {
        allowed: false,
        reason_code: Some(reason_code.to_string()),
        reason: Some(format!(
            "{action_label} 결과 조각이 모음의 파일 용량 제한을 초과합니다 ({} > {} bytes).",
            inspection.largest_piece_bytes, inspection.max_piece_bytes
        )),
    }
}

fn normalization_summary(recipe_json: &str) -> Option<AiNormalizationSummaryDto> {
    let value: serde_json::Value = serde_json::from_str(recipe_json).ok()?;
    let object = value.as_object()?;
    if object.get("schema")?.as_str()? != "pmtcon-ai-normalization-v1" {
        return None;
    }
    let kind = object.get("kind")?.as_str()?;
    let target_canvas_width = object.get("targetCanvasWidth")?.as_i64()?;
    let target_canvas_height = object.get("targetCanvasHeight")?.as_i64()?;
    if target_canvas_width <= 0 || target_canvas_height <= 0 {
        return None;
    }
    let (mode, alignment, resize_filter) = match kind {
        "identity" => {
            if ["mode", "alignment", "resizeFilter"]
                .iter()
                .any(|key| object.get(*key).is_some_and(|value| !value.is_null()))
            {
                return None;
            }
            (None, None, None)
        }
        "contain_pad" | "cover_crop" => {
            let mode = object.get("mode")?.as_str()?;
            let alignment = object.get("alignment")?.as_str()?;
            let resize_filter = object.get("resizeFilter")?.as_str()?;
            if mode != kind
                || ![
                    "top_left",
                    "top",
                    "top_right",
                    "left",
                    "center",
                    "right",
                    "bottom_left",
                    "bottom",
                    "bottom_right",
                ]
                .contains(&alignment)
                || !["lanczos3", "nearest"].contains(&resize_filter)
            {
                return None;
            }
            (
                Some(mode.to_string()),
                Some(alignment.to_string()),
                Some(resize_filter.to_string()),
            )
        }
        _ => return None,
    };
    Some(AiNormalizationSummaryDto {
        kind: kind.to_string(),
        mode,
        alignment,
        resize_filter,
        target_canvas_width,
        target_canvas_height,
    })
}

fn validate_manual_service_surface(value: &str) -> AppResult<&str> {
    match value {
        "gemini_web" | "novelai_web" | "other_manual" => Ok(value),
        _ => Err(AppError::new(
            "validation",
            "수동 결과 출처는 Gemini 웹, NovelAI 웹 또는 기타 수동 작업만 선택할 수 있습니다.",
        )),
    }
}

fn provider_for_surface(surface: &str) -> &'static str {
    match surface {
        "gemini_web" => "google",
        "novelai_web" => "novelai",
        _ => "manual",
    }
}

fn sha256_file(path: &Path) -> AppResult<String> {
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
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

pub(crate) fn ai_repair_required() -> AppError {
    AppError::new(
        "ai_source_repair_required",
        "AI 소스 이력 또는 파일이 손상되었습니다. 원본 복원이나 이력 정리가 필요합니다.",
    )
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalization_summary_parses_canonical_v1_recipe() {
        let recipe = r#"{
          "schema":"pmtcon-ai-normalization-v1",
          "kind":"cover_crop",
          "mode":"cover_crop",
          "alignment":"bottom_right",
          "resizeFilter":"nearest",
          "targetCanvasWidth":400,
          "targetCanvasHeight":200
        }"#;

        assert_eq!(
            normalization_summary(recipe),
            Some(AiNormalizationSummaryDto {
                kind: "cover_crop".to_string(),
                mode: Some("cover_crop".to_string()),
                alignment: Some("bottom_right".to_string()),
                resize_filter: Some("nearest".to_string()),
                target_canvas_width: 400,
                target_canvas_height: 200,
            })
        );
    }

    #[test]
    fn normalization_summary_marks_legacy_or_inconsistent_recipe_unknown() {
        assert_eq!(normalization_summary(r#"{"kind":"contain_pad"}"#), None);
        assert_eq!(
            normalization_summary(
                r#"{
                  "schema":"pmtcon-ai-normalization-v1",
                  "kind":"contain_pad",
                  "mode":"cover_crop",
                  "alignment":"center",
                  "resizeFilter":"lanczos3",
                  "targetCanvasWidth":200,
                  "targetCanvasHeight":200
                }"#
            ),
            None
        );
    }
}
