use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use image::ImageFormat;
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::db::repositories::source_files::{
    commit_prepared_source_file, prepare_source_file_from_bytes, source_thumbnail_path,
    PreparedSourceArtifactSnapshot, PreparedSourceFile, SourceFileImportOptions,
};
use crate::db::repositories::{ai, ai_activation, ai_handoff, ai_snapshots, icons};
use crate::error::{AppError, AppResult};
use crate::ids::create_id;
use crate::imaging::import_limits::{decode_import_image, read_import_file_bytes};
use crate::models::{IconDto, ImportImageFilePayload};
use crate::optimization::cache::hash_text;
use crate::paths::AppPaths;
use crate::sheet::composer::{
    compose_ai_edit_grid, compose_ai_reference_sheet, ensure_ai_reference_targets_current,
    AiGridLayout, AiGridRect, ComposeAiEditGridRequest, ComposeAiReferenceSheetRequest,
    ComposedAiGrid, AI_GRID_SCHEMA,
};
use crate::sheet::grid::{
    alpha_warning_for_extension, analyze_rgba_grid, SheetGridAnalysis, SheetGridSettings,
};
use crate::sheet::splitter::{
    split_reviewed_grid, ReviewedGridDecision, SplitGridCell, SplitReviewedGridRequest,
    StaticGridImageFormat,
};

const MAX_GRID_ITEMS: usize = 16;
const MAX_GRID_OUTPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_GRID_MANIFEST_BYTES: usize = 64 * 1024;
const MAX_GRID_CANVAS_DIMENSION: u32 = 2_048;
const MAX_GRID_CANVAS_PIXELS: u64 = 4_194_304;
const GRID_THUMBNAIL_RESERVATION_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PreparedAiGridRequest {
    pub request_id: String,
    pub request_scope: String,
    pub status: String,
    pub item_count: i64,
    pub input_sheet_sha256: Option<String>,
    pub input_manifest_sha256: Option<String>,
    pub retry_of_request_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AiGridRequestState {
    pub request_id: String,
    pub request_scope: String,
    pub status: String,
    pub retry_of_request_id: Option<String>,
    pub item_count: i64,
    pub candidate_count: i64,
    pub input_sheet_sha256: Option<String>,
    pub output_sheet_sha256: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct PrepareAiGenerationRequest {
    pub target_names: Vec<String>,
    pub layout: AiGridLayout,
    pub payload_input_signature: String,
    pub retry_of_request_id: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct PrepareAiGenerationReferences {
    pub selected_icon_ids: Vec<String>,
    pub external_files: Vec<ImportImageFilePayload>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommitAiGridCandidatesResult {
    pub request_id: String,
    pub candidate_ids: Vec<String>,
    pub rejected_item_indexes: Vec<i64>,
    pub review_signature: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AiGridArtifactDto {
    pub role: String,
    pub source_file_id: String,
    pub original_filename: String,
    pub file_path: String,
    pub extension: String,
    pub mime_type: String,
    pub width: i64,
    pub height: i64,
    pub byte_size: i64,
    pub sha256: String,
    pub has_alpha: bool,
    pub manifest_json: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AiGridWorkspaceItemDto {
    pub id: String,
    pub item_index: i64,
    pub origin_icon_id: Option<String>,
    pub origin_icon_id_snapshot: Option<String>,
    pub target_name_snapshot: String,
    pub shape: String,
    pub row_index: i64,
    pub column_index: i64,
    pub input_rect: AiGridRect,
    pub review_status: String,
    pub output_candidate_id: Option<String>,
    pub created_icon_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AiGridWorkspaceDto {
    pub request_id: String,
    pub collection_id: String,
    pub request_scope: String,
    pub status: String,
    pub retry_of_request_id: Option<String>,
    pub layout: AiGridLayout,
    pub item_count: i64,
    pub candidate_count: i64,
    pub created_icon_count: i64,
    pub input_artifact: Option<AiGridArtifactDto>,
    pub output_artifact: Option<AiGridArtifactDto>,
    pub items: Vec<AiGridWorkspaceItemDto>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FinalizeGeneratedIconInput {
    pub item_index: i64,
    pub display_name: String,
    #[serde(default)]
    pub alt_text: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommitGeneratedIconsResult {
    pub request_id: String,
    pub created_icons: Vec<IconDto>,
}

#[derive(Debug, Clone)]
struct GridRequestItem {
    id: String,
    item_index: i64,
    origin_icon_id: Option<String>,
    origin_icon_id_snapshot: Option<String>,
    original_lineage_id: Option<String>,
    original_lineage_generation: Option<i64>,
    original_source_sha256: Option<String>,
    effective_source_sha256: Option<String>,
    activation_revision: Option<i64>,
    native_recipe_signature: Option<String>,
    review_status: String,
    output_candidate_id: Option<String>,
}

struct OutputArtifact {
    request_scope: String,
    collection_id: String,
    source_path: String,
    source_extension: String,
    sha256: String,
}

struct PreparedCandidateCell {
    cell: SplitGridCell,
    source: PreparedSourceFile,
    artifact_snapshot: PreparedSourceArtifactSnapshot,
    candidate_id: String,
}

pub(crate) fn prepare_ai_grid_edit(
    connection: &mut Connection,
    paths: &AppPaths,
    collection_id: &str,
    selected_icon_ids: Vec<String>,
    layout: AiGridLayout,
    retry_of_request_id: Option<String>,
) -> AppResult<PreparedAiGridRequest> {
    validate_retry(
        connection,
        collection_id,
        "grid_edit",
        retry_of_request_id.as_deref(),
    )?;
    let composed = compose_ai_edit_grid(
        connection,
        ComposeAiEditGridRequest {
            collection_id,
            selected_icon_ids: &selected_icon_ids,
            layout,
        },
    )?;
    let _storage_reservation = ai_handoff::reserve_ai_transfer_storage(
        connection,
        paths,
        planned_grid_artifact_storage_bytes(composed.png_bytes.len())?,
    )?;
    let prepared_source = prepare_source_file_from_bytes(
        &ImportImageFilePayload {
            original_filename: "pmtcon-ai-grid-input.png".to_string(),
            bytes: composed.png_bytes.clone(),
        },
        SourceFileImportOptions {
            allow_gif: false,
            exact_dimensions: Some((
                i64::from(composed.layout.canvas_width),
                i64::from(composed.layout.canvas_height),
            )),
        },
    )?;
    let artifact_snapshot = prepared_source.artifact_snapshot(connection, paths)?;
    let request_id = create_id("ai_request");
    let item_count = i64::try_from(composed.items.len()).unwrap_or(i64::MAX);
    let snapshots = foundation_snapshots("grid_edit_prepare", item_count, &composed.layout)?;
    let payload_signature = hash_text(&[
        AI_GRID_SCHEMA.to_string(),
        composed.png_sha256.clone(),
        composed.manifest_sha256.clone(),
    ]);
    let result = (|| -> AppResult<PreparedAiGridRequest> {
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        ensure_composed_targets_current(&tx, collection_id, &composed)?;
        let collection_name = collection_name(&tx, collection_id)?;
        let stored = commit_prepared_source_file(&tx, paths, &prepared_source)?;
        if stored.sha256 != composed.png_sha256 {
            return Err(AppError::new(
                "ai_grid_input_hash",
                "AI 그리드 입력 시트 해시가 준비한 바이트와 일치하지 않습니다.",
            ));
        }
        insert_grid_request(
            &tx,
            &request_id,
            collection_id,
            &collection_name,
            "grid_edit",
            retry_of_request_id.as_deref(),
            "grid_edit_prepare",
            &snapshots,
            Some(&stored.sha256),
            None,
            &payload_signature,
        )?;
        insert_edit_items(&tx, &request_id, &composed)?;
        insert_artifact(
            &tx,
            &request_id,
            "input_sheet",
            &stored.id,
            &stored.sha256,
            &composed.manifest_json,
        )?;
        transition_request(&tx, &request_id, "draft", "prepared")?;
        tx.commit()?;
        Ok(PreparedAiGridRequest {
            request_id,
            request_scope: "grid_edit".to_string(),
            status: "prepared".to_string(),
            item_count,
            input_sheet_sha256: Some(composed.png_sha256),
            input_manifest_sha256: Some(composed.manifest_sha256),
            retry_of_request_id,
        })
    })();
    if result.is_err() {
        let _ = artifact_snapshot.cleanup_if_unreferenced(connection);
    }
    result
}

pub(crate) fn prepare_ai_generation(
    connection: &mut Connection,
    collection_id: &str,
    request: PrepareAiGenerationRequest,
) -> AppResult<PreparedAiGridRequest> {
    let item_count = request.target_names.len();
    let scope = if item_count == 1 {
        "single_generate"
    } else {
        "grid_generate"
    };
    validate_generation_layout(item_count, &request.layout)?;
    let payload_signature = normalized_signature(&request.payload_input_signature)?;
    validate_retry(
        connection,
        collection_id,
        scope,
        request.retry_of_request_id.as_deref(),
    )?;
    let request_id = create_id("ai_request");
    let operation = if scope == "single_generate" {
        "single_generate_prepare"
    } else {
        "grid_generate_prepare"
    };
    let snapshots = foundation_snapshots(
        operation,
        i64::try_from(item_count).unwrap_or(i64::MAX),
        &request.layout,
    )?;
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let collection_name = collection_name(&tx, collection_id)?;
    insert_grid_request(
        &tx,
        &request_id,
        collection_id,
        &collection_name,
        scope,
        request.retry_of_request_id.as_deref(),
        operation,
        &snapshots,
        None,
        None,
        &payload_signature,
    )?;
    insert_generation_items(
        &tx,
        &request_id,
        scope,
        &request.target_names,
        &request.layout,
    )?;
    transition_request(&tx, &request_id, "draft", "prepared")?;
    tx.commit()?;
    Ok(PreparedAiGridRequest {
        request_id,
        request_scope: scope.to_string(),
        status: "prepared".to_string(),
        item_count: i64::try_from(item_count).unwrap_or(i64::MAX),
        input_sheet_sha256: None,
        input_manifest_sha256: None,
        retry_of_request_id: request.retry_of_request_id,
    })
}

pub(crate) fn prepare_ai_generation_with_references(
    connection: &mut Connection,
    paths: &AppPaths,
    collection_id: &str,
    request: PrepareAiGenerationRequest,
    references: PrepareAiGenerationReferences,
) -> AppResult<PreparedAiGridRequest> {
    if references.selected_icon_ids.is_empty() && references.external_files.is_empty() {
        return prepare_ai_generation(connection, collection_id, request);
    }
    let item_count = request.target_names.len();
    let scope = if item_count == 1 {
        "single_generate"
    } else {
        "grid_generate"
    };
    validate_generation_layout(item_count, &request.layout)?;
    let prompt_signature = normalized_signature(&request.payload_input_signature)?;
    validate_retry(
        connection,
        collection_id,
        scope,
        request.retry_of_request_id.as_deref(),
    )?;
    let composed = compose_ai_reference_sheet(
        connection,
        ComposeAiReferenceSheetRequest {
            collection_id,
            selected_icon_ids: &references.selected_icon_ids,
            external_files: &references.external_files,
            canvas_size: 1_024,
        },
    )?;
    let _storage_reservation = ai_handoff::reserve_ai_transfer_storage(
        connection,
        paths,
        planned_grid_artifact_storage_bytes(composed.png_bytes.len())?,
    )?;
    let prepared_source = prepare_source_file_from_bytes(
        &ImportImageFilePayload {
            original_filename: "pmtcon-ai-generation-references.png".to_string(),
            bytes: composed.png_bytes.clone(),
        },
        SourceFileImportOptions {
            allow_gif: false,
            exact_dimensions: Some((composed.layout.canvas_width, composed.layout.canvas_height)),
        },
    )?;
    let artifact_snapshot = prepared_source.artifact_snapshot(connection, paths)?;
    let request_id = create_id("ai_request");
    let operation = if scope == "single_generate" {
        "single_generate_prepare"
    } else {
        "grid_generate_prepare"
    };
    let snapshots = foundation_snapshots(
        operation,
        i64::try_from(item_count).unwrap_or(i64::MAX),
        &request.layout,
    )?;
    let payload_signature = hash_text(&[
        "pmtcon-ai-generation-reference-v1".to_string(),
        prompt_signature,
        composed.png_sha256.clone(),
        composed.manifest_sha256.clone(),
    ]);
    let result = (|| -> AppResult<PreparedAiGridRequest> {
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        ensure_ai_reference_targets_current(&tx, collection_id, &composed)?;
        let collection_name = collection_name(&tx, collection_id)?;
        let stored = commit_prepared_source_file(&tx, paths, &prepared_source)?;
        if stored.sha256 != composed.png_sha256 {
            return Err(AppError::new(
                "ai_reference_hash",
                "AI 참고 시트 해시가 준비한 바이트와 일치하지 않습니다.",
            ));
        }
        insert_grid_request(
            &tx,
            &request_id,
            collection_id,
            &collection_name,
            scope,
            request.retry_of_request_id.as_deref(),
            operation,
            &snapshots,
            Some(&stored.sha256),
            Some(&stored.sha256),
            &payload_signature,
        )?;
        insert_generation_items(
            &tx,
            &request_id,
            scope,
            &request.target_names,
            &request.layout,
        )?;
        insert_artifact(
            &tx,
            &request_id,
            "input_sheet",
            &stored.id,
            &stored.sha256,
            &composed.manifest_json,
        )?;
        transition_request(&tx, &request_id, "draft", "prepared")?;
        tx.commit()?;
        Ok(PreparedAiGridRequest {
            request_id,
            request_scope: scope.to_string(),
            status: "prepared".to_string(),
            item_count: i64::try_from(item_count).unwrap_or(i64::MAX),
            input_sheet_sha256: Some(composed.png_sha256.clone()),
            input_manifest_sha256: Some(composed.manifest_sha256.clone()),
            retry_of_request_id: request.retry_of_request_id.clone(),
        })
    })();
    if result.is_err() {
        let _ = artifact_snapshot.cleanup_if_unreferenced(connection);
    }
    result
}

pub(crate) fn mark_ai_grid_awaiting_result(
    connection: &Connection,
    request_id: &str,
) -> AppResult<()> {
    transition_request(connection, request_id, "prepared", "awaiting_result")
}

pub(crate) fn record_ai_grid_output_artifact(
    connection: &mut Connection,
    paths: &AppPaths,
    request_id: &str,
    file: ImportImageFilePayload,
    manifest_json: &str,
) -> AppResult<AiGridRequestState> {
    if file.bytes.len() > MAX_GRID_OUTPUT_BYTES {
        return Err(AppError::new(
            "ai_grid_output_too_large",
            "AI 그리드 결과 시트는 최대 16MB까지 가져올 수 있습니다.",
        ));
    }
    let canonical_manifest = canonical_grid_manifest(manifest_json)?;
    let prepared = prepare_source_file_from_bytes(
        &file,
        SourceFileImportOptions {
            allow_gif: false,
            exact_dimensions: None,
        },
    )?;
    let planned = prepared.planned_source_file(paths);
    let pixels = planned
        .width
        .checked_mul(planned.height)
        .unwrap_or(i64::MAX);
    if planned.is_animated
        || !matches!(planned.original_extension.as_str(), "png" | "jpg" | "jpeg")
        || planned.width > i64::from(MAX_GRID_CANVAS_DIMENSION)
        || planned.height > i64::from(MAX_GRID_CANVAS_DIMENSION)
        || pixels > i64::try_from(MAX_GRID_CANVAS_PIXELS).unwrap_or(i64::MAX)
    {
        return Err(AppError::new(
            "ai_grid_output_format",
            "AI 그리드 결과는 최대 2048×2048의 정적 PNG/JPG만 사용할 수 있습니다.",
        ));
    }
    let _storage_reservation = ai_handoff::reserve_ai_transfer_storage(
        connection,
        paths,
        planned_grid_artifact_storage_bytes(file.bytes.len())?,
    )?;
    let artifact_snapshot = prepared.artifact_snapshot(connection, paths)?;
    let result = (|| -> AppResult<AiGridRequestState> {
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current_status = tx
            .query_row(
                "SELECT status FROM ai_requests
                 WHERE id = ?1
                   AND request_scope IN ('grid_edit', 'single_generate', 'grid_generate')",
                [request_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or_else(|| AppError::not_found("AI 그리드 요청을 찾을 수 없습니다."))?;
        match current_status.as_str() {
            "prepared" => {
                transition_request(&tx, request_id, "prepared", "awaiting_result")?;
            }
            "awaiting_result" => {}
            _ => {
                return Err(AppError::new(
                    "ai_grid_status_conflict",
                    "결과 파일은 준비됨 또는 결과 대기 상태의 AI 그리드 요청에만 첨부할 수 있습니다.",
                ));
            }
        }
        let stored = commit_prepared_source_file(&tx, paths, &prepared)?;
        insert_artifact(
            &tx,
            request_id,
            "output_sheet",
            &stored.id,
            &stored.sha256,
            &canonical_manifest,
        )?;
        transition_request(&tx, request_id, "awaiting_result", "layout_review_pending")?;
        let state = get_ai_grid_request_state(&tx, request_id)?;
        tx.commit()?;
        Ok(state)
    })();
    if result.is_err() {
        let _ = artifact_snapshot.cleanup_if_unreferenced(connection);
    }
    result
}

pub(crate) fn commit_ai_grid_candidates(
    connection: &mut Connection,
    paths: &AppPaths,
    request_id: &str,
    decisions: Vec<ReviewedGridDecision>,
) -> AppResult<CommitAiGridCandidatesResult> {
    let output = load_output_artifact(connection, request_id)?;
    let encoded_sheet = read_import_file_bytes(std::path::Path::new(&output.source_path))?;
    if encoded_sheet.len() > MAX_GRID_OUTPUT_BYTES || sha256_hex(&encoded_sheet) != output.sha256 {
        return Err(AppError::new(
            "ai_grid_output_hash",
            "저장된 AI 그리드 결과 시트가 불변 artifact 해시와 일치하지 않습니다.",
        ));
    }
    let items = load_request_items(connection, request_id)?;
    let item_indexes = items.iter().map(|item| item.item_index).collect::<Vec<_>>();
    if output.request_scope == "grid_edit" && decisions.iter().any(|decision| !decision.include) {
        return Err(AppError::new(
            "ai_grid_edit_mapping_incomplete",
            "여러 아이콘 수정 결과는 모든 대상 셀의 매핑을 확정해야 합니다.",
        ));
    }
    let (format, image_format) = match output.source_extension.as_str() {
        "png" => (StaticGridImageFormat::Png, ImageFormat::Png),
        "jpg" | "jpeg" => (StaticGridImageFormat::Jpeg, ImageFormat::Jpeg),
        _ => {
            return Err(AppError::new(
                "ai_grid_output_format",
                "AI 그리드 결과 시트 형식을 확인할 수 없습니다.",
            ))
        }
    };
    let workspace = get_ai_grid_workspace(connection, request_id)?;
    if workspace.request_scope != output.request_scope {
        return Err(ai_grid_output_structure_error());
    }
    let decoded = decode_import_image(&encoded_sheet, image_format)?.to_rgba8();
    let expected_settings = expected_output_sheet_settings(&workspace.layout);
    let analysis = analyze_rgba_grid(
        &decoded,
        &expected_settings,
        i64::from(decoded.width()),
        i64::from(decoded.height()),
    )
    .map_err(|_| ai_grid_output_structure_error())?;
    ensure_output_review_contract(&workspace.layout, &analysis, &decisions)?;
    let split = split_reviewed_grid(SplitReviewedGridRequest {
        encoded_sheet: &encoded_sheet,
        format,
        request_item_indexes: &item_indexes,
        decisions: &decisions,
    })?;
    if split.output_sheet_sha256 != output.sha256 {
        return Err(AppError::new(
            "ai_grid_output_hash",
            "검토한 결과 시트와 저장된 output artifact가 일치하지 않습니다.",
        ));
    }
    if split.cells.is_empty() {
        return Err(AppError::new(
            "ai_grid_empty_review",
            "후보로 저장할 결과 셀을 하나 이상 포함해야 합니다.",
        ));
    }
    if output.request_scope == "grid_edit" {
        ensure_edit_items_current(connection, &output.collection_id, &items)?;
    }
    let mut prepared_cells = prepare_candidate_cells(connection, paths, split.cells)?;
    let result = (|| -> AppResult<CommitAiGridCandidatesResult> {
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        ensure_request_status_and_scope(
            &tx,
            request_id,
            "layout_review_pending",
            &["grid_edit", "single_generate", "grid_generate"],
        )?;
        let transaction_items = load_request_items(&tx, request_id)?;
        ensure_same_item_snapshot(&items, &transaction_items)?;
        if output.request_scope == "grid_edit" {
            ensure_edit_items_current(&tx, &output.collection_id, &transaction_items)?;
        }
        let existing: i64 = tx.query_row(
            "SELECT COUNT(*) FROM ai_candidates WHERE request_id = ?1",
            [request_id],
            |row| row.get(0),
        )?;
        if existing != 0
            || transaction_items
                .iter()
                .any(|item| item.output_candidate_id.is_some() || item.review_status != "pending")
        {
            return Err(AppError::new(
                "ai_grid_candidate_conflict",
                "이 AI 그리드 요청에는 이미 검토 결과가 저장되어 있습니다.",
            ));
        }
        let capability: String = tx.query_row(
            "SELECT capability_snapshot_json FROM ai_requests WHERE id = ?1",
            [request_id],
            |row| row.get(0),
        )?;
        let item_by_index = transaction_items
            .iter()
            .map(|item| (item.item_index, item))
            .collect::<HashMap<_, _>>();
        let mut candidate_ids = Vec::with_capacity(prepared_cells.len());
        for prepared in &prepared_cells {
            let item = item_by_index
                .get(&prepared.cell.target_item_index)
                .ok_or_else(|| {
                    AppError::new(
                        "ai_grid_item_missing",
                        "검토한 결과 셀의 요청 항목을 찾을 수 없습니다.",
                    )
                })?;
            let included_rows = tx.execute(
                "UPDATE ai_request_items
                 SET review_status = 'included',
                     updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE id = ?1 AND request_id = ?2
                   AND review_status = 'pending' AND output_candidate_id IS NULL",
                params![item.id, request_id],
            )?;
            if included_rows != 1 {
                return Err(AppError::new(
                    "ai_grid_item_conflict",
                    "AI 그리드 포함 항목의 검토 상태가 변경되었습니다.",
                ));
            }
            let stored = commit_prepared_source_file(&tx, paths, &prepared.source)?;
            let has_alpha = stored.has_alpha.ok_or_else(|| {
                AppError::new(
                    "ai_grid_candidate_metadata",
                    "AI 그리드 후보의 알파 정보를 확인할 수 없습니다.",
                )
            })?;
            if stored.sha256 != prepared.cell.png_sha256
                || stored.width != prepared.cell.width
                || stored.height != prepared.cell.height
                || stored.is_animated
            {
                return Err(AppError::new(
                    "ai_grid_candidate_metadata",
                    "분할한 후보 셀과 저장한 source metadata가 일치하지 않습니다.",
                ));
            }
            tx.execute(
                "INSERT INTO ai_candidates (
                   id, request_id, request_item_id, candidate_index, raw_source_file_id,
                   raw_source_sha256, output_format, width, height, is_animated, has_alpha,
                   provider_capabilities_snapshot_json, created_at
                 ) VALUES (
                   ?1, ?2, ?3, ?4, ?5, ?6, 'png', ?7, ?8, 0, ?9, ?10,
                   strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 )",
                params![
                    prepared.candidate_id,
                    request_id,
                    item.id,
                    item.item_index,
                    stored.id,
                    stored.sha256,
                    stored.width,
                    stored.height,
                    i64::from(has_alpha),
                    capability,
                ],
            )?;
            let linked_rows = tx.execute(
                "UPDATE ai_request_items
                 SET output_candidate_id = ?1,
                     updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE id = ?2 AND request_id = ?3
                   AND review_status = 'included' AND output_candidate_id IS NULL",
                params![prepared.candidate_id, item.id, request_id],
            )?;
            if linked_rows != 1 {
                return Err(AppError::new(
                    "ai_grid_item_conflict",
                    "AI 그리드 요청 항목에 후보를 연결할 수 없습니다.",
                ));
            }
            if output.request_scope == "grid_edit" {
                let completed_rows = tx.execute(
                    "UPDATE ai_request_items
                     SET review_status = 'candidate_created',
                         updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                     WHERE id = ?1 AND request_id = ?2
                       AND review_status = 'included' AND output_candidate_id = ?3",
                    params![item.id, request_id, prepared.candidate_id],
                )?;
                if completed_rows != 1 {
                    return Err(AppError::new(
                        "ai_grid_item_conflict",
                        "AI 그리드 후보 생성 상태를 확정할 수 없습니다.",
                    ));
                }
            }
            candidate_ids.push(prepared.candidate_id.clone());
        }
        let included = prepared_cells
            .iter()
            .map(|p| p.cell.target_item_index)
            .collect::<HashSet<_>>();
        let mut rejected = Vec::new();
        for item in &transaction_items {
            if !included.contains(&item.item_index) {
                let updated = tx.execute(
                    "UPDATE ai_request_items
                     SET review_status = 'excluded', updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                     WHERE id = ?1 AND request_id = ?2
                       AND review_status = 'pending' AND output_candidate_id IS NULL",
                    params![item.id, request_id],
                )?;
                if updated != 1 {
                    return Err(AppError::new(
                        "ai_grid_item_conflict",
                        "AI 그리드 제외 항목의 검토 상태가 변경되었습니다.",
                    ));
                }
                rejected.push(item.item_index);
            }
        }
        if output.request_scope == "grid_edit" {
            transition_request(&tx, request_id, "layout_review_pending", "completed")?;
        }
        tx.commit()?;
        Ok(CommitAiGridCandidatesResult {
            request_id: request_id.to_string(),
            candidate_ids,
            rejected_item_indexes: rejected,
            review_signature: split.review_signature,
        })
    })();
    if result.is_err() {
        for prepared in &mut prepared_cells {
            let _ = prepared
                .artifact_snapshot
                .cleanup_if_unreferenced(connection);
        }
    }
    result
}
fn expected_output_sheet_settings(layout: &AiGridLayout) -> SheetGridSettings {
    SheetGridSettings {
        mode: "cell_size".to_string(),
        rows: Some(layout.rows),
        columns: Some(layout.columns),
        cell_width: Some(layout.cell_size),
        cell_height: Some(layout.cell_size),
        border_left: layout.border_left,
        border_top: layout.border_top,
        border_right: layout.border_right,
        border_bottom: layout.border_bottom,
        gap_x: layout.gap_x,
        gap_y: layout.gap_y,
        read_order: "row_major".to_string(),
        empty_cell_threshold: Some(0.99),
    }
}

fn ensure_output_review_contract(
    layout: &AiGridLayout,
    analysis: &SheetGridAnalysis,
    decisions: &[ReviewedGridDecision],
) -> AppResult<()> {
    let expected_cell_count = layout
        .rows
        .checked_mul(layout.columns)
        .ok_or_else(ai_grid_output_structure_error)?;
    if analysis.sheet_width != layout.canvas_width
        || analysis.sheet_height != layout.canvas_height
        || analysis.computed_rows != layout.rows
        || analysis.computed_columns != layout.columns
        || analysis.cell_count != expected_cell_count
        || analysis.cells.len() != usize::try_from(expected_cell_count).unwrap_or(usize::MAX)
        || !analysis.out_of_bounds_cells.is_empty()
    {
        return Err(ai_grid_output_structure_error());
    }

    let cells_by_index = analysis
        .cells
        .iter()
        .map(|cell| (cell.index, cell))
        .collect::<HashMap<_, _>>();
    if cells_by_index.len() != analysis.cells.len() {
        return Err(ai_grid_output_structure_error());
    }
    for index in 0..expected_cell_count {
        let row = index / layout.columns;
        let column = index % layout.columns;
        let expected_x =
            checked_grid_coordinate(layout.border_left, column, layout.cell_size, layout.gap_x)?;
        let expected_y =
            checked_grid_coordinate(layout.border_top, row, layout.cell_size, layout.gap_y)?;
        let cell = cells_by_index
            .get(&index)
            .ok_or_else(ai_grid_output_structure_error)?;
        if cell.page != 0
            || cell.row != row
            || cell.col != column
            || cell.x != expected_x
            || cell.y != expected_y
            || cell.w != layout.cell_size
            || cell.h != layout.cell_size
            || cell.out_of_bounds
        {
            return Err(ai_grid_output_structure_error());
        }
    }

    let empty_cells = analysis
        .empty_cell_candidates
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    let mut mapped_results = HashSet::with_capacity(decisions.len());
    for decision in decisions {
        let cell = cells_by_index
            .get(&decision.result_cell_index)
            .ok_or_else(ai_grid_output_structure_error)?;
        if !mapped_results.insert(decision.result_cell_index) {
            return Err(AppError::new(
                "ai_grid_review_mapping",
                "AI 그리드 결과 셀은 대상별로 한 번만 지정해야 합니다.",
            ));
        }
        if decision.include {
            if empty_cells.contains(&decision.result_cell_index) {
                return Err(AppError::new(
                    "ai_grid_output_empty_cell",
                    "포함할 AI 그리드 결과 셀이 비어 있습니다. 빈 셀은 제외하거나 결과를 다시 받아 주세요.",
                ));
            }
            let expected_crop = AiGridRect {
                x: cell.x,
                y: cell.y,
                width: cell.w,
                height: cell.h,
            };
            if decision.crop != Some(expected_crop) {
                return Err(ai_grid_output_structure_error());
            }
        } else if decision.crop.is_some() {
            return Err(AppError::new(
                "ai_grid_review_mapping",
                "제외한 AI 그리드 결과 셀에는 crop 영역을 저장할 수 없습니다.",
            ));
        }
    }
    Ok(())
}

fn ai_grid_output_structure_error() -> AppError {
    AppError::new(
        "ai_grid_output_structure",
        "AI 그리드 결과의 캔버스, 행·열, 셀 위치·크기가 준비된 레이아웃과 일치하지 않습니다.",
    )
}

pub(crate) fn cancel_ai_grid_request(connection: &Connection, request_id: &str) -> AppResult<()> {
    let updated = connection.execute(
        "UPDATE ai_requests
         SET status = 'cancelled', completed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?1
           AND request_scope IN ('grid_edit', 'single_generate', 'grid_generate')
           AND status IN ('draft', 'prepared', 'awaiting_result', 'running', 'layout_review_pending')",
        [request_id],
    )?;
    if updated != 1 {
        return Err(AppError::new(
            "ai_grid_cancel_conflict",
            "취소할 수 있는 상태의 AI 그리드 요청을 찾을 수 없습니다.",
        ));
    }
    Ok(())
}

pub(crate) fn get_ai_grid_request_state(
    connection: &Connection,
    request_id: &str,
) -> AppResult<AiGridRequestState> {
    connection.query_row(
        "SELECT request.id, request.request_scope, request.status, request.retry_of_request_id,
                (SELECT COUNT(*) FROM ai_request_items item WHERE item.request_id = request.id),
                (SELECT COUNT(*) FROM ai_candidates candidate WHERE candidate.request_id = request.id),
                (SELECT artifact.sha256 FROM ai_request_artifacts artifact
                 WHERE artifact.request_id = request.id AND artifact.role = 'input_sheet'),
                (SELECT artifact.sha256 FROM ai_request_artifacts artifact
                 WHERE artifact.request_id = request.id AND artifact.role = 'output_sheet')
         FROM ai_requests request
         WHERE request.id = ?1
           AND request.request_scope IN ('grid_edit', 'single_generate', 'grid_generate')",
        [request_id],
        |row| Ok(AiGridRequestState {
            request_id: row.get(0)?, request_scope: row.get(1)?, status: row.get(2)?,
            retry_of_request_id: row.get(3)?, item_count: row.get(4)?, candidate_count: row.get(5)?,
            input_sheet_sha256: row.get(6)?, output_sheet_sha256: row.get(7)?,
        }),
    ).optional()?.ok_or_else(|| AppError::not_found("AI 그리드 요청을 찾을 수 없습니다."))
}

pub(crate) fn get_ai_grid_workspace(
    connection: &Connection,
    request_id: &str,
) -> AppResult<AiGridWorkspaceDto> {
    let request = connection
        .query_row(
            "SELECT
               origin_collection_id, request_scope, status, retry_of_request_id,
               prompt_options_snapshot_json,
               (SELECT COUNT(*) FROM ai_candidates candidate
                WHERE candidate.request_id = request.id),
               (SELECT COUNT(*) FROM ai_icon_root_creations creation
                JOIN ai_request_items item ON item.id = creation.request_item_id
                WHERE item.request_id = request.id AND creation.creation_kind = 'source_free'),
               created_at, updated_at
             FROM ai_requests request
             WHERE id = ?1
               AND request_scope IN ('grid_edit', 'single_generate', 'grid_generate')",
            [request_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("AI 그리드 요청을 찾을 수 없습니다."))?;

    let mut statement = connection.prepare(
        "SELECT
           item.id, item.item_index, item.origin_icon_id, item.origin_icon_id_snapshot,
           item.target_name_snapshot, item.shape, item.row_index, item.column_index,
           item.input_cell_x, item.input_cell_y, item.cell_width, item.cell_height,
           item.review_status, item.output_candidate_id,
           (SELECT creation.icon_id FROM ai_icon_root_creations creation
            WHERE creation.request_item_id = item.id
              AND creation.creation_kind = 'source_free'
            ORDER BY creation.creation_order DESC LIMIT 1)
         FROM ai_request_items item
         WHERE item.request_id = ?1
         ORDER BY item.item_index ASC",
    )?;
    let items = statement
        .query_map([request_id], |row| {
            Ok(AiGridWorkspaceItemDto {
                id: row.get(0)?,
                item_index: row.get(1)?,
                origin_icon_id: row.get(2)?,
                origin_icon_id_snapshot: row.get(3)?,
                target_name_snapshot: row.get(4)?,
                shape: row.get(5)?,
                row_index: row.get(6)?,
                column_index: row.get(7)?,
                input_rect: AiGridRect {
                    x: row.get(8)?,
                    y: row.get(9)?,
                    width: row.get(10)?,
                    height: row.get(11)?,
                },
                review_status: row.get(12)?,
                output_candidate_id: row.get(13)?,
                created_icon_id: row.get(14)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    if items.is_empty() {
        return Err(AppError::new(
            "ai_grid_workspace_corrupt",
            "AI 그리드 요청의 저장된 항목을 복구할 수 없습니다.",
        ));
    }
    let layout = workspace_layout_from_snapshot(&request.4, &items)?;
    Ok(AiGridWorkspaceDto {
        request_id: request_id.to_string(),
        collection_id: request.0,
        request_scope: request.1,
        status: request.2,
        retry_of_request_id: request.3,
        layout,
        item_count: i64::try_from(items.len()).unwrap_or(i64::MAX),
        candidate_count: request.5,
        created_icon_count: request.6,
        input_artifact: load_ai_grid_artifact(connection, request_id, "input_sheet")?,
        output_artifact: load_ai_grid_artifact(connection, request_id, "output_sheet")?,
        items,
        created_at: request.7,
        updated_at: request.8,
    })
}

pub(crate) fn get_latest_ai_grid_workspace(
    connection: &Connection,
    collection_id: &str,
) -> AppResult<Option<AiGridWorkspaceDto>> {
    collection_name(connection, collection_id)?;
    let request_id = connection
        .query_row(
            "SELECT id FROM ai_requests
             WHERE origin_collection_id = ?1
               AND request_scope IN ('grid_edit', 'single_generate', 'grid_generate')
               AND status IN ('draft', 'prepared', 'awaiting_result', 'running', 'layout_review_pending')
             ORDER BY updated_at DESC, created_at DESC, id DESC
             LIMIT 1",
            [collection_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    request_id
        .map(|request_id| get_ai_grid_workspace(connection, &request_id))
        .transpose()
}

pub(crate) fn analyze_ai_grid_output(
    connection: &Connection,
    request_id: &str,
    settings: SheetGridSettings,
) -> AppResult<SheetGridAnalysis> {
    let output = load_output_artifact(connection, request_id)?;
    let source = ai::load_and_validate_source(
        connection,
        &load_ai_grid_artifact(connection, request_id, "output_sheet")?
            .ok_or_else(|| AppError::not_found("AI 그리드 결과 파일을 찾을 수 없습니다."))?
            .source_file_id,
    )?;
    if source.path != output.source_path || source.sha256 != output.sha256 {
        return Err(AppError::new(
            "ai_grid_output_hash",
            "저장된 AI 그리드 결과와 artifact 정보가 일치하지 않습니다.",
        ));
    }
    let bytes = read_import_file_bytes(Path::new(&source.path))?;
    let format = match source.extension.as_str() {
        "png" => ImageFormat::Png,
        "jpg" | "jpeg" => ImageFormat::Jpeg,
        _ => {
            return Err(AppError::new(
                "ai_grid_output_format",
                "AI 그리드 결과는 정적 PNG/JPG만 분석할 수 있습니다.",
            ));
        }
    };
    let image = decode_import_image(&bytes, format)?.to_rgba8();
    let mut analysis = analyze_rgba_grid(
        &image,
        &settings,
        i64::from(image.width()),
        i64::from(image.height()),
    )?;
    if let Some(warning) = alpha_warning_for_extension(&source.extension) {
        analysis.warnings.push(warning.to_string());
    }
    Ok(analysis)
}

pub(crate) fn reveal_ai_grid_input(
    connection: &Connection,
    paths: &AppPaths,
    request_id: &str,
) -> AppResult<()> {
    let input_path = verified_ai_grid_input_path(connection, paths, request_id)?;
    crate::export::open_export_path(input_path.to_string_lossy().as_ref())
}

pub(crate) fn verified_ai_grid_input_path(
    connection: &Connection,
    paths: &AppPaths,
    request_id: &str,
) -> AppResult<PathBuf> {
    let (request_scope, status) = connection
        .query_row(
            "SELECT request_scope, status FROM ai_requests WHERE id = ?1",
            [request_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("AI 그리드 요청을 찾을 수 없습니다."))?;
    if !matches!(
        request_scope.as_str(),
        "grid_edit" | "single_generate" | "grid_generate"
    ) {
        return Err(AppError::not_found(
            "전달할 AI 입력 이미지를 찾을 수 없습니다.",
        ));
    }
    if !matches!(status.as_str(), "prepared" | "awaiting_result") {
        return Err(AppError::new(
            "ai_grid_input_not_live",
            "현재 단계에서는 입력 시트를 다시 전달할 수 없습니다. 새 AI 그리드 작업을 준비해 주세요.",
        ));
    }

    // Read the stored path without opening it first. This prevents a tampered DB row
    // or a symlink/reparse point from making source validation follow an unmanaged file.
    let stored_path = connection
        .query_row(
            "SELECT source.original_path_in_library
             FROM ai_request_artifacts artifact
             JOIN source_files source ON source.id = artifact.source_file_id
             WHERE artifact.request_id = ?1 AND artifact.role = 'input_sheet'",
            [request_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .ok_or_else(|| {
            AppError::not_found("이 AI 요청에는 전달할 편집 시트 또는 참고 시트가 없습니다.")
        })?;
    let canonical_path =
        crate::native_drag::canonical_managed_drag_file(paths, Path::new(&stored_path))
            .map_err(|_| ai_grid_input_managed_path_error())?;
    let canonical_originals = paths
        .originals_dir
        .canonicalize()
        .map_err(|_| ai_grid_input_managed_path_error())?;
    if !canonical_path.starts_with(&canonical_originals) {
        return Err(ai_grid_input_managed_path_error());
    }

    let input = load_ai_grid_artifact(connection, request_id, "input_sheet")?.ok_or_else(|| {
        AppError::not_found("이 AI 요청에는 전달할 편집 시트 또는 참고 시트가 없습니다.")
    })?;
    let verified_path =
        crate::native_drag::canonical_managed_drag_file(paths, Path::new(&input.file_path))
            .map_err(|_| ai_grid_input_managed_path_error())?;
    if verified_path != canonical_path {
        return Err(ai_grid_input_managed_path_error());
    }
    Ok(PathBuf::from(input.file_path))
}

fn ai_grid_input_managed_path_error() -> AppError {
    AppError::new(
        "ai_grid_input_unmanaged_path",
        "AI 그리드 입력 파일이 PMTCONCON Studio의 원본 관리 경로에 없거나 링크로 바뀌었습니다. 새 작업을 준비해 주세요.",
    )
}

pub(crate) fn commit_ai_generated_icons(
    connection: &mut Connection,
    paths: &AppPaths,
    collection_id: &str,
    request_id: &str,
    finalized_items: Vec<FinalizeGeneratedIconInput>,
) -> AppResult<CommitGeneratedIconsResult> {
    let mut finalized_by_index = HashMap::with_capacity(finalized_items.len());
    for item in finalized_items {
        if !(0..=15).contains(&item.item_index)
            || finalized_by_index.insert(item.item_index, item).is_some()
        {
            return Err(AppError::new(
                "ai_grid_finalize_mapping",
                "확정할 생성 항목 번호가 중복되었거나 범위를 벗어났습니다.",
            ));
        }
    }

    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let (scope, status, request_collection): (String, String, String) = tx
        .query_row(
            "SELECT request_scope, status, origin_collection_id
             FROM ai_requests WHERE id = ?1",
            [request_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("AI 그리드 생성 요청을 찾을 수 없습니다."))?;
    if !matches!(scope.as_str(), "single_generate" | "grid_generate")
        || status != "layout_review_pending"
        || request_collection != collection_id
    {
        return Err(AppError::new(
            "ai_grid_finalize_state",
            "레이아웃 검토가 끝난 현재 모음의 AI 생성 요청만 아이콘으로 확정할 수 있습니다.",
        ));
    }

    let collection: (i64, i64, Option<String>, Option<String>) = tx
        .query_row(
            "SELECT default_cell_width, default_cell_height, cover_icon_id, cover_source_file_id
             FROM collections WHERE id = ?1 AND deleted_at IS NULL",
            [collection_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("생성된 아이콘을 넣을 모음을 찾을 수 없습니다."))?;

    let existing_roots: i64 = tx.query_row(
        "SELECT COUNT(*) FROM ai_icon_root_creations creation
         JOIN ai_request_items item ON item.id = creation.request_item_id
         WHERE item.request_id = ?1 AND creation.creation_kind = 'source_free'",
        [request_id],
        |row| row.get(0),
    )?;
    if existing_roots != 0 {
        return Err(AppError::new(
            "ai_grid_finalize_conflict",
            "이 AI 생성 요청은 이미 아이콘으로 확정되었습니다.",
        ));
    }

    let raw_items = {
        let mut statement = tx.prepare(
            "SELECT item.id, item.item_index, item.target_name_snapshot, item.review_status,
                    item.output_candidate_id, candidate.raw_source_file_id,
                    candidate.candidate_index
             FROM ai_request_items item
             LEFT JOIN ai_candidates candidate ON candidate.id = item.output_candidate_id
             WHERE item.request_id = ?1
             ORDER BY item.item_index ASC",
        )?;
        let rows = statement
            .query_map([request_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, Option<i64>>(6)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows
    };
    if !(1..=MAX_GRID_ITEMS).contains(&raw_items.len()) {
        return Err(AppError::new(
            "ai_grid_finalize_mapping",
            "저장된 AI 생성 항목 수가 1~16개 범위를 벗어났습니다.",
        ));
    }

    let reviewed_included_indexes = raw_items
        .iter()
        .filter(|item| item.3 == "included")
        .map(|item| item.1)
        .collect::<HashSet<_>>();
    let finalized_indexes = finalized_by_index.keys().copied().collect::<HashSet<_>>();
    if finalized_indexes != reviewed_included_indexes {
        return Err(AppError::new(
            "ai_grid_finalize_mapping",
            "최종 생성 목록은 검토에서 포함한 셀과 정확히 일치해야 하며 제외한 빈 셀을 포함할 수 없습니다.",
        ));
    }

    struct CreationPlanItem {
        request_item_id: String,
        item_index: i64,
        candidate_id: String,
        source: ai::VisualSourceRecord,
        thumbnail_path: String,
        display_name: String,
        alt_text: String,
    }
    let mut plan = Vec::new();
    for (
        item_id,
        item_index,
        target_name,
        review_status,
        candidate_id,
        source_id,
        candidate_index,
    ) in raw_items
    {
        match review_status.as_str() {
            "included" => {
                let finalized = finalized_by_index.remove(&item_index).ok_or_else(|| {
                    AppError::new(
                        "ai_grid_finalize_mapping",
                        "포함된 모든 생성 항목의 이름과 대체 텍스트를 확인해 주세요.",
                    )
                })?;
                let candidate_id = candidate_id.ok_or_else(|| {
                    AppError::new(
                        "ai_grid_finalize_mapping",
                        "생성 후보 연결 정보가 없습니다.",
                    )
                })?;
                let source_id = source_id.ok_or_else(|| {
                    AppError::new(
                        "ai_grid_finalize_mapping",
                        "생성 후보 원본 정보가 없습니다.",
                    )
                })?;
                if candidate_index != Some(item_index) {
                    return Err(AppError::new(
                        "ai_grid_finalize_mapping",
                        "생성 후보 순서가 요청 항목 순서와 일치하지 않습니다.",
                    ));
                }
                let source = ai::load_and_validate_source(&tx, &source_id)?;
                if source.extension != "png" || source.is_animated {
                    return Err(AppError::new(
                        "ai_grid_finalize_source",
                        "검토를 통과한 정적 PNG 후보만 새 아이콘으로 확정할 수 있습니다.",
                    ));
                }
                let thumbnail = source_thumbnail_path(paths, &source.id);
                if !thumbnail.is_file() {
                    return Err(AppError::new(
                        "ai_grid_finalize_source",
                        "생성 후보의 관리 썸네일을 찾을 수 없어 아무 항목도 저장하지 않았습니다.",
                    ));
                }
                plan.push(CreationPlanItem {
                    request_item_id: item_id,
                    item_index,
                    candidate_id,
                    source,
                    thumbnail_path: thumbnail.to_string_lossy().to_string(),
                    display_name: normalized_final_display_name(
                        &finalized.display_name,
                        &target_name,
                    )?,
                    alt_text: normalized_final_alt(&finalized.alt_text)?,
                });
            }
            "excluded" => {
                if candidate_id.is_some() || source_id.is_some() {
                    return Err(AppError::new(
                        "ai_grid_finalize_mapping",
                        "제외된 항목에 생성 후보가 연결되어 있습니다.",
                    ));
                }
            }
            _ => {
                return Err(AppError::new(
                    "ai_grid_finalize_state",
                    "모든 생성 항목의 포함 또는 제외 검토를 먼저 완료해 주세요.",
                ));
            }
        }
    }
    if !finalized_by_index.is_empty() || plan.is_empty() {
        return Err(AppError::new(
            "ai_grid_finalize_mapping",
            "확정 목록은 검토에서 포함한 항목과 정확히 일치해야 합니다.",
        ));
    }

    let mut next_order: i64 = tx.query_row(
        "SELECT COALESCE(MAX(order_index) + 1, 0) FROM icons
         WHERE collection_id = ?1 AND deleted_at IS NULL",
        [collection_id],
        |row| row.get(0),
    )?;
    let should_set_cover = collection.2.is_none() && collection.3.is_none();
    let mut created_ids = Vec::with_capacity(plan.len());
    for item in plan {
        let icon_id = create_id("icon");
        tx.execute(
            "INSERT INTO icons (
               id, collection_id, source_file_id, display_name, shape, order_index,
               thumbnail_path, current_preview_path, icon_kind, readiness, created_at, updated_at
             ) VALUES (
               ?1, ?2, ?3, ?4, 'single', ?5, ?6, ?6, 'image', 'working',
               strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
               strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             )",
            params![
                icon_id,
                collection_id,
                item.source.id,
                item.display_name,
                next_order,
                item.thumbnail_path,
            ],
        )?;
        let crop = centered_crop_rect(
            item.source.width,
            item.source.height,
            collection.0,
            collection.1,
        );
        tx.execute(
            "INSERT INTO crop_settings (
               id, icon_id, crop_mode, crop_x, crop_y, crop_w, crop_h, preset_position,
               source_width_at_apply, source_height_at_apply,
               viewport_width_at_apply, viewport_height_at_apply, updated_at
             ) VALUES (
               ?1, ?2, 'free', ?3, ?4, ?5, ?6, 'center', ?7, ?8, ?9, ?10,
               strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             )",
            params![
                create_id("crop"),
                icon_id,
                crop.0,
                crop.1,
                crop.2,
                crop.3,
                item.source.width,
                item.source.height,
                collection.0,
                collection.1,
            ],
        )?;
        tx.execute(
            "INSERT INTO icon_pieces (
               id, icon_id, piece_index, piece_role, alt_text, export_status,
               created_at, updated_at
             ) VALUES (
               ?1, ?2, 0, 'single', ?3, 'not_exported',
               strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
               strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             )",
            params![create_id("piece"), icon_id, item.alt_text],
        )?;
        tx.execute(
            "INSERT INTO ai_icon_root_creations (
               icon_id, source_icon_id, candidate_id, request_item_id, creation_kind,
               normalization_recipe_hash, created_at
             ) VALUES (
               ?1, NULL, ?2, ?3, 'source_free', NULL,
               strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             )",
            params![icon_id, item.candidate_id, item.request_item_id],
        )?;
        let updated = tx.execute(
            "UPDATE ai_request_items
             SET review_status = 'icon_created',
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?1 AND request_id = ?2 AND item_index = ?3
               AND review_status = 'included'",
            params![item.request_item_id, request_id, item.item_index],
        )?;
        if updated != 1 {
            return Err(AppError::new(
                "ai_grid_finalize_conflict",
                "생성 항목 상태가 바뀌어 아무 항목도 저장하지 않았습니다.",
            ));
        }
        created_ids.push(icon_id);
        next_order = next_order.checked_add(1).ok_or_else(|| {
            AppError::new("ai_grid_finalize_order", "아이콘 정렬 번호가 너무 큽니다.")
        })?;
    }

    if should_set_cover {
        let first_icon_id = created_ids.first().ok_or_else(|| {
            AppError::new("ai_grid_finalize_mapping", "생성된 아이콘이 없습니다.")
        })?;
        let first_source_id: String = tx.query_row(
            "SELECT source_file_id FROM icons WHERE id = ?1",
            [first_icon_id],
            |row| row.get(0),
        )?;
        tx.execute(
            "UPDATE collections
             SET cover_icon_id = ?1, cover_source_file_id = ?2,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?3 AND cover_icon_id IS NULL AND cover_source_file_id IS NULL",
            params![first_icon_id, first_source_id, collection_id],
        )?;
    }
    transition_request(&tx, request_id, "layout_review_pending", "completed")?;
    let created_icons = created_ids
        .iter()
        .map(|icon_id| icons::get_icon(&tx, collection_id, icon_id))
        .collect::<AppResult<Vec<_>>>()?;
    tx.commit()?;
    Ok(CommitGeneratedIconsResult {
        request_id: request_id.to_string(),
        created_icons,
    })
}

fn load_ai_grid_artifact(
    connection: &Connection,
    request_id: &str,
    role: &str,
) -> AppResult<Option<AiGridArtifactDto>> {
    let artifact = connection
        .query_row(
            "SELECT source_file_id, sha256, manifest_json, created_at
             FROM ai_request_artifacts
             WHERE request_id = ?1 AND role = ?2",
            params![request_id, role],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )
        .optional()?;
    let Some((source_file_id, artifact_sha256, manifest_json, created_at)) = artifact else {
        return Ok(None);
    };
    let source = ai::load_and_validate_source(connection, &source_file_id)?;
    if source.sha256 != artifact_sha256 {
        return Err(AppError::new(
            "ai_grid_artifact_hash",
            "AI 그리드 artifact와 관리 원본의 해시가 일치하지 않습니다.",
        ));
    }
    Ok(Some(AiGridArtifactDto {
        role: role.to_string(),
        source_file_id,
        original_filename: source.original_filename,
        file_path: source.path,
        extension: source.extension,
        mime_type: source.mime_type,
        width: source.width,
        height: source.height,
        byte_size: source.byte_size,
        sha256: source.sha256,
        has_alpha: source.has_alpha,
        manifest_json,
        created_at,
    }))
}

fn workspace_layout_from_snapshot(
    prompt_options_json: &str,
    items: &[AiGridWorkspaceItemDto],
) -> AppResult<AiGridLayout> {
    let snapshot: Value = serde_json::from_str(prompt_options_json).map_err(|_| {
        AppError::new(
            "ai_grid_workspace_corrupt",
            "AI 그리드 레이아웃 스냅샷을 읽을 수 없습니다.",
        )
    })?;
    let layout = if let Some(value) = snapshot.get("gridLayout") {
        serde_json::from_value::<AiGridLayout>(value.clone()).map_err(|_| {
            AppError::new(
                "ai_grid_workspace_corrupt",
                "AI 그리드 레이아웃 스냅샷 형식이 올바르지 않습니다.",
            )
        })?
    } else {
        fallback_workspace_layout(&snapshot, items)?
    };
    validate_generation_layout(items.len(), &layout)?;
    for item in items {
        let expected_x = checked_grid_coordinate(
            layout.border_left,
            item.column_index,
            layout.cell_size,
            layout.gap_x,
        )?;
        let expected_y = checked_grid_coordinate(
            layout.border_top,
            item.row_index,
            layout.cell_size,
            layout.gap_y,
        )?;
        if item.input_rect
            != (AiGridRect {
                x: expected_x,
                y: expected_y,
                width: layout.cell_size,
                height: layout.cell_size,
            })
        {
            return Err(AppError::new(
                "ai_grid_workspace_corrupt",
                "AI 그리드 항목 좌표와 저장된 레이아웃이 일치하지 않습니다.",
            ));
        }
    }
    Ok(layout)
}

fn fallback_workspace_layout(
    snapshot: &Value,
    items: &[AiGridWorkspaceItemDto],
) -> AppResult<AiGridLayout> {
    let canvas_width = snapshot
        .get("width")
        .and_then(Value::as_i64)
        .ok_or_else(|| {
            AppError::new(
                "ai_grid_workspace_corrupt",
                "AI 그리드 canvas 너비가 없습니다.",
            )
        })?;
    let canvas_height = snapshot
        .get("height")
        .and_then(Value::as_i64)
        .ok_or_else(|| {
            AppError::new(
                "ai_grid_workspace_corrupt",
                "AI 그리드 canvas 높이가 없습니다.",
            )
        })?;
    let rows = items.iter().map(|item| item.row_index).max().unwrap_or(0) + 1;
    let columns = items
        .iter()
        .map(|item| item.column_index)
        .max()
        .unwrap_or(0)
        + 1;
    let first = items
        .first()
        .ok_or_else(|| AppError::new("ai_grid_workspace_corrupt", "AI 그리드 항목이 없습니다."))?;
    let cell_size = first.input_rect.width;
    let origin = items
        .iter()
        .find(|item| item.row_index == 0 && item.column_index == 0)
        .ok_or_else(|| {
            AppError::new("ai_grid_workspace_corrupt", "AI 그리드 시작 셀이 없습니다.")
        })?;
    let gap_x = if columns > 1 {
        items
            .iter()
            .find(|item| item.row_index == 0 && item.column_index == 1)
            .map(|item| item.input_rect.x - origin.input_rect.x - cell_size)
            .unwrap_or(0)
    } else {
        0
    };
    let gap_y = if rows > 1 {
        items
            .iter()
            .find(|item| item.row_index == 1 && item.column_index == 0)
            .map(|item| item.input_rect.y - origin.input_rect.y - cell_size)
            .unwrap_or(0)
    } else {
        0
    };
    let border_right =
        canvas_width - origin.input_rect.x - columns * cell_size - (columns - 1) * gap_x;
    let border_bottom = canvas_height - origin.input_rect.y - rows * cell_size - (rows - 1) * gap_y;
    Ok(AiGridLayout {
        canvas_width,
        canvas_height,
        rows,
        columns,
        cell_size,
        gap_x,
        gap_y,
        border_left: origin.input_rect.x,
        border_top: origin.input_rect.y,
        border_right,
        border_bottom,
    })
}

fn planned_grid_artifact_storage_bytes(payload_bytes: usize) -> AppResult<u64> {
    let payload_bytes = u64::try_from(payload_bytes).map_err(|_| {
        AppError::new(
            "ai_handoff_storage_size",
            "AI 그리드 임시 파일 크기를 계산할 수 없습니다.",
        )
    })?;
    payload_bytes
        .checked_add(GRID_THUMBNAIL_RESERVATION_BYTES)
        .ok_or_else(|| {
            AppError::new(
                "ai_handoff_storage_size",
                "AI 그리드 임시 파일 크기를 계산할 수 없습니다.",
            )
        })
}
fn normalized_final_display_name(value: &str, fallback: &str) -> AppResult<String> {
    let trimmed = value.trim();
    let chosen = if trimmed.is_empty() {
        fallback.trim()
    } else {
        trimmed
    };
    if chosen.is_empty() || chosen.len() > 255 || chosen.chars().any(char::is_control) {
        return Err(AppError::new(
            "ai_grid_final_name",
            "새 아이콘 이름은 제어 문자 없이 1~255바이트여야 합니다.",
        ));
    }
    Ok(chosen.to_string())
}

fn normalized_final_alt(value: &str) -> AppResult<String> {
    let trimmed = value.trim();
    if trimmed.len() > 255 || trimmed.chars().any(char::is_control) {
        return Err(AppError::new(
            "ai_grid_final_alt",
            "새 아이콘 대체 텍스트는 제어 문자 없이 255바이트 이하여야 합니다.",
        ));
    }
    Ok(trimmed.to_string())
}

fn centered_crop_rect(
    source_width: i64,
    source_height: i64,
    target_width: i64,
    target_height: i64,
) -> (f64, f64, f64, f64) {
    let source_width = source_width.max(1) as f64;
    let source_height = source_height.max(1) as f64;
    let target_aspect = target_width.max(1) as f64 / target_height.max(1) as f64;
    let source_aspect = source_width / source_height;
    let (width, height) = if source_aspect > target_aspect {
        (source_height * target_aspect, source_height)
    } else {
        (source_width, source_width / target_aspect)
    };
    (
        ((source_width - width) / 2.0).max(0.0),
        ((source_height - height) / 2.0).max(0.0),
        width.min(source_width),
        height.min(source_height),
    )
}

struct FoundationSnapshots {
    capability: String,
    data_tier: String,
    retention: String,
    consent: String,
    policy_refs: String,
    prompt_options: String,
}

fn foundation_snapshots(
    operation: &str,
    output_count: i64,
    layout: &AiGridLayout,
) -> AppResult<FoundationSnapshots> {
    Ok(FoundationSnapshots {
        capability: ai_snapshots::canonicalize(
            "capability",
            &json!({
                "schema": "pmtcon-ai-capability-v1", "provider": "unassigned",
                "serviceSurface": "other_manual", "source": "provider-free-grid-foundation",
                "supports": ["static-grid-review"], "limitations": ["no-provider-dispatch"]
            })
            .to_string(),
        )?,
        data_tier: ai_snapshots::canonicalize(
            "data_tier",
            r#"{"schema":"pmtcon-ai-data-tier-v1","source":"not-dispatched","tier":"none"}"#,
        )?,
        retention: ai_snapshots::canonicalize(
            "retention",
            r#"{"schema":"pmtcon-ai-retention-v1","source":"local-library","retention":"user-managed"}"#,
        )?,
        consent: ai_snapshots::canonicalize(
            "consent",
            r#"{"schema":"pmtcon-ai-consent-v1","source":"not-dispatched","confirmed":false,"humanActionConfirmed":false}"#,
        )?,
        policy_refs: ai_snapshots::canonicalize("policy_refs", "[]")?,
        prompt_options: ai_snapshots::canonicalize(
            "prompt_options",
            &json!({
                "schema": "pmtcon-ai-prompt-options-v1", "operation": operation,
                "provider": "unassigned", "width": layout.canvas_width,
                "height": layout.canvas_height, "outputCount": output_count,
                "gridLayout": layout
            })
            .to_string(),
        )?,
    })
}

#[allow(clippy::too_many_arguments)]
fn insert_grid_request(
    tx: &Transaction<'_>,
    request_id: &str,
    collection_id: &str,
    collection_name: &str,
    scope: &str,
    retry_of: Option<&str>,
    operation: &str,
    snapshots: &FoundationSnapshots,
    input_sha256: Option<&str>,
    reference_sha256: Option<&str>,
    payload_signature: &str,
) -> AppResult<()> {
    let inserted = tx.execute(
        "INSERT INTO ai_requests (
           id, request_scope, retry_of_request_id, origin_collection_id, origin_icon_id,
           origin_collection_name_snapshot, origin_icon_name_snapshot,
           provider_mode, service_surface, provider, adapter_id, adapter_contract_version,
           account_context, model, operation, provenance_trust, credential_mode_snapshot,
           capability_snapshot_json, data_tier_snapshot_json, retention_snapshot_json,
           consent_snapshot_json, policy_refs_json, prompt_options_snapshot_json,
           input_package_sha256, reference_package_sha256,
           original_lineage_id, original_lineage_generation,
           original_source_sha256, effective_source_sha256, payload_input_signature,
           request_recipe_signature, activation_revision, status, created_at, updated_at
         ) VALUES (
           ?1, ?2, ?3, ?4, NULL, ?5, NULL,
           'manual_web', 'other_manual', 'unassigned', 'pmtcon-ai-grid-foundation', '1',
           'unknown', NULL, ?6, 'manual_unverified', 'none', ?7, ?8, ?9, ?10, ?11, ?12,
           ?13, ?14, NULL, NULL, NULL, NULL, ?15, NULL, NULL, 'draft',
           strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         )",
        params![
            request_id,
            scope,
            retry_of,
            collection_id,
            collection_name,
            operation,
            snapshots.capability,
            snapshots.data_tier,
            snapshots.retention,
            snapshots.consent,
            snapshots.policy_refs,
            snapshots.prompt_options,
            input_sha256,
            reference_sha256,
            payload_signature,
        ],
    )?;
    if inserted != 1 {
        return Err(AppError::new(
            "ai_grid_request_insert",
            "AI 그리드 요청을 저장할 수 없습니다.",
        ));
    }
    Ok(())
}

fn insert_edit_items(
    tx: &Transaction<'_>,
    request_id: &str,
    composed: &ComposedAiGrid,
) -> AppResult<()> {
    for item in &composed.items {
        let row = i64::from(item.item_index) / i64::from(composed.layout.columns);
        let column = i64::from(item.item_index) % i64::from(composed.layout.columns);
        tx.execute(
            "INSERT INTO ai_request_items (
               id, request_id, request_scope, item_index, origin_icon_id,
               origin_icon_id_snapshot, target_name_snapshot, shape, row_index, column_index,
               input_cell_x, input_cell_y, cell_width, cell_height, original_lineage_id,
               original_lineage_generation, original_source_sha256, effective_source_sha256,
               activation_revision, native_recipe_signature, input_render_recipe_hash,
               input_render_sha256, output_candidate_id, review_status, created_at, updated_at
             ) VALUES (
               ?1, ?2, 'grid_edit', ?3, ?4, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
               ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, NULL, 'pending',
               strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             )",
            params![
                create_id("ai_request_item"),
                request_id,
                i64::from(item.item_index),
                item.origin_icon_id,
                item.target_name_snapshot,
                item.shape,
                row,
                column,
                item.input_rect.x,
                item.input_rect.y,
                item.input_rect.width,
                item.input_rect.height,
                item.original_lineage_id,
                item.original_lineage_generation,
                item.original_source_sha256,
                item.effective_source_sha256,
                item.activation_revision,
                item.native_recipe_signature,
                item.input_render_recipe_hash,
                item.input_render_sha256,
            ],
        )?;
    }
    Ok(())
}

fn insert_generation_items(
    tx: &Transaction<'_>,
    request_id: &str,
    scope: &str,
    target_names: &[String],
    layout: &AiGridLayout,
) -> AppResult<()> {
    let columns = i64::from(layout.columns);
    for (position, target_name) in target_names.iter().enumerate() {
        let item_index = i64::try_from(position)
            .map_err(|_| AppError::new("ai_grid_layout", "AI 그리드 항목 수가 너무 큽니다."))?;
        let row = item_index / columns;
        let column = item_index % columns;
        let x = checked_grid_coordinate(
            i64::from(layout.border_left),
            column,
            i64::from(layout.cell_size),
            i64::from(layout.gap_x),
        )?;
        let y = checked_grid_coordinate(
            i64::from(layout.border_top),
            row,
            i64::from(layout.cell_size),
            i64::from(layout.gap_y),
        )?;
        tx.execute(
            "INSERT INTO ai_request_items (
               id, request_id, request_scope, item_index, origin_icon_id,
               origin_icon_id_snapshot, target_name_snapshot, shape, row_index, column_index,
               input_cell_x, input_cell_y, cell_width, cell_height, original_lineage_id,
               original_lineage_generation, original_source_sha256, effective_source_sha256,
               activation_revision, native_recipe_signature, input_render_recipe_hash,
               input_render_sha256, output_candidate_id, review_status, created_at, updated_at
             ) VALUES (
               ?1, ?2, ?3, ?4, NULL, NULL, ?5, 'single', ?6, ?7, ?8, ?9, ?10, ?10,
               NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'pending',
               strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             )",
            params![
                create_id("ai_request_item"),
                request_id,
                scope,
                item_index,
                normalized_target_name(target_name, position)?,
                row,
                column,
                x,
                y,
                i64::from(layout.cell_size),
            ],
        )?;
    }
    Ok(())
}

fn insert_artifact(
    tx: &Transaction<'_>,
    request_id: &str,
    role: &str,
    source_file_id: &str,
    sha256: &str,
    manifest_json: &str,
) -> AppResult<()> {
    tx.execute(
        "INSERT INTO ai_request_artifacts (
           request_id, role, source_file_id, sha256, manifest_json, created_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
        params![request_id, role, source_file_id, sha256, manifest_json],
    )?;
    Ok(())
}
fn transition_request(
    connection: &Connection,
    request_id: &str,
    from: &str,
    to: &str,
) -> AppResult<()> {
    let updated = connection.execute(
        "UPDATE ai_requests
         SET status = ?1,
             started_at = CASE WHEN ?1 = 'awaiting_result'
               THEN strftime('%Y-%m-%dT%H:%M:%fZ', 'now') ELSE started_at END,
             completed_at = CASE WHEN ?1 IN ('completed', 'failed', 'cancelled', 'expired')
               THEN strftime('%Y-%m-%dT%H:%M:%fZ', 'now') ELSE completed_at END,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?2
           AND request_scope IN ('grid_edit', 'single_generate', 'grid_generate')
           AND status = ?3",
        params![to, request_id, from],
    )?;
    if updated != 1 {
        return Err(AppError::new(
            "ai_grid_status_conflict",
            "AI 그리드 요청 상태가 변경되었습니다. 현재 상태를 다시 확인해 주세요.",
        ));
    }
    Ok(())
}

fn ensure_request_status_and_scope(
    connection: &Connection,
    request_id: &str,
    expected_status: &str,
    scopes: &[&str],
) -> AppResult<String> {
    let (scope, status) = connection
        .query_row(
            "SELECT request_scope, status FROM ai_requests WHERE id = ?1",
            [request_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("AI 그리드 요청을 찾을 수 없습니다."))?;
    if status != expected_status || !scopes.contains(&scope.as_str()) {
        return Err(AppError::new(
            "ai_grid_status_conflict",
            "AI 그리드 요청 상태 또는 범위가 현재 작업과 일치하지 않습니다.",
        ));
    }
    Ok(scope)
}

fn ensure_composed_targets_current(
    connection: &Connection,
    collection_id: &str,
    composed: &ComposedAiGrid,
) -> AppResult<()> {
    for item in &composed.items {
        let current =
            ai::resolve_effective_visual_source(connection, collection_id, &item.origin_icon_id)?;
        let recipe = ai_activation::current_recipe_signature(
            connection,
            collection_id,
            &item.origin_icon_id,
            &current.render_source,
            current.activation_revision,
        )?;
        if current.original_lineage_id != item.original_lineage_id
            || current.original_lineage_generation != item.original_lineage_generation
            || current.original_source.sha256 != item.original_source_sha256
            || current.render_source.sha256 != item.effective_source_sha256
            || current.activation_revision != item.activation_revision
            || recipe != item.native_recipe_signature
        {
            return Err(grid_stale_error());
        }
    }
    Ok(())
}

fn ensure_edit_items_current(
    connection: &Connection,
    collection_id: &str,
    items: &[GridRequestItem],
) -> AppResult<()> {
    for item in items {
        let icon_id = item
            .origin_icon_id
            .as_deref()
            .filter(|id| Some(*id) == item.origin_icon_id_snapshot.as_deref())
            .ok_or_else(grid_stale_error)?;
        let current = ai::resolve_effective_visual_source(connection, collection_id, icon_id)
            .map_err(|_| grid_stale_error())?;
        let recipe = ai_activation::current_recipe_signature(
            connection,
            collection_id,
            icon_id,
            &current.render_source,
            current.activation_revision,
        )?;
        if item.original_lineage_id.as_deref() != Some(current.original_lineage_id.as_str())
            || item.original_lineage_generation != Some(current.original_lineage_generation)
            || item.original_source_sha256.as_deref()
                != Some(current.original_source.sha256.as_str())
            || item.effective_source_sha256.as_deref()
                != Some(current.render_source.sha256.as_str())
            || item.activation_revision != Some(current.activation_revision)
            || item.native_recipe_signature.as_deref() != Some(recipe.as_str())
        {
            return Err(grid_stale_error());
        }
    }
    Ok(())
}

fn grid_stale_error() -> AppError {
    AppError::new(
        "ai_grid_target_stale",
        "AI 그리드 결과를 검토하는 동안 대상 아이콘 하나 이상이 변경되었습니다. 새 요청으로 다시 준비해 주세요.",
    )
}

fn load_request_items(
    connection: &Connection,
    request_id: &str,
) -> AppResult<Vec<GridRequestItem>> {
    let mut statement = connection.prepare(
        "SELECT id, item_index, origin_icon_id, origin_icon_id_snapshot,
                original_lineage_id, original_lineage_generation, original_source_sha256,
                effective_source_sha256, activation_revision, native_recipe_signature,
                review_status, output_candidate_id
         FROM ai_request_items WHERE request_id = ?1 ORDER BY item_index ASC",
    )?;
    let items = statement
        .query_map([request_id], |row| {
            Ok(GridRequestItem {
                id: row.get(0)?,
                item_index: row.get(1)?,
                origin_icon_id: row.get(2)?,
                origin_icon_id_snapshot: row.get(3)?,
                original_lineage_id: row.get(4)?,
                original_lineage_generation: row.get(5)?,
                original_source_sha256: row.get(6)?,
                effective_source_sha256: row.get(7)?,
                activation_revision: row.get(8)?,
                native_recipe_signature: row.get(9)?,
                review_status: row.get(10)?,
                output_candidate_id: row.get(11)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    if items.is_empty() {
        return Err(AppError::new(
            "ai_grid_items_missing",
            "AI 그리드 요청 항목을 찾을 수 없습니다.",
        ));
    }
    Ok(items)
}

fn load_output_artifact(connection: &Connection, request_id: &str) -> AppResult<OutputArtifact> {
    connection
        .query_row(
            "SELECT request.request_scope, request.origin_collection_id,
                source.original_path_in_library, source.original_extension, artifact.sha256
         FROM ai_requests request
         JOIN ai_request_artifacts artifact
           ON artifact.request_id = request.id AND artifact.role = 'output_sheet'
         JOIN source_files source ON source.id = artifact.source_file_id
         WHERE request.id = ?1 AND request.status = 'layout_review_pending'",
            [request_id],
            |row| {
                Ok(OutputArtifact {
                    request_scope: row.get(0)?,
                    collection_id: row.get(1)?,
                    source_path: row.get(2)?,
                    source_extension: row.get(3)?,
                    sha256: row.get(4)?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| {
            AppError::new(
                "ai_grid_output_missing",
                "레이아웃을 검토할 AI 그리드 결과 artifact를 찾을 수 없습니다.",
            )
        })
}

fn prepare_candidate_cells(
    connection: &Connection,
    paths: &AppPaths,
    cells: Vec<SplitGridCell>,
) -> AppResult<Vec<PreparedCandidateCell>> {
    let total = cells.iter().try_fold(0_usize, |total, cell| {
        total.checked_add(cell.png_bytes.len()).ok_or_else(|| {
            AppError::new(
                "ai_grid_candidate_too_large",
                "AI 그리드 후보 셀의 전체 크기가 지원 범위를 벗어났습니다.",
            )
        })
    })?;
    if total > MAX_GRID_OUTPUT_BYTES.saturating_mul(4) {
        return Err(AppError::new(
            "ai_grid_candidate_too_large",
            "AI 그리드 후보 셀은 합계 64MB까지 저장할 수 있습니다.",
        ));
    }
    let mut prepared = Vec::with_capacity(cells.len());
    for cell in cells {
        if cell.png_bytes.len() > MAX_GRID_OUTPUT_BYTES {
            return Err(AppError::new(
                "ai_grid_candidate_too_large",
                "AI 그리드 후보 셀 하나는 최대 16MB까지 저장할 수 있습니다.",
            ));
        }
        let source = prepare_source_file_from_bytes(
            &ImportImageFilePayload {
                original_filename: format!(
                    "pmtcon-ai-grid-cell-{:02}.png",
                    cell.target_item_index + 1
                ),
                bytes: cell.png_bytes.clone(),
            },
            SourceFileImportOptions {
                allow_gif: false,
                exact_dimensions: Some((cell.width, cell.height)),
            },
        )?;
        let artifact_snapshot = source.artifact_snapshot(connection, paths)?;
        prepared.push(PreparedCandidateCell {
            cell,
            source,
            artifact_snapshot,
            candidate_id: create_id("ai_candidate"),
        });
    }
    Ok(prepared)
}
fn ensure_same_item_snapshot(
    before: &[GridRequestItem],
    after: &[GridRequestItem],
) -> AppResult<()> {
    if before.len() != after.len()
        || before.iter().zip(after).any(|(left, right)| {
            left.id != right.id
                || left.item_index != right.item_index
                || left.origin_icon_id_snapshot != right.origin_icon_id_snapshot
                || left.original_lineage_id != right.original_lineage_id
                || left.original_lineage_generation != right.original_lineage_generation
                || left.original_source_sha256 != right.original_source_sha256
                || left.effective_source_sha256 != right.effective_source_sha256
                || left.activation_revision != right.activation_revision
                || left.native_recipe_signature != right.native_recipe_signature
                || left.review_status != right.review_status
                || left.output_candidate_id != right.output_candidate_id
        })
    {
        return Err(AppError::new(
            "ai_grid_item_conflict",
            "AI 그리드 요청 항목이 검토 중 변경되었습니다.",
        ));
    }
    Ok(())
}

fn validate_retry(
    connection: &Connection,
    collection_id: &str,
    scope: &str,
    retry_of: Option<&str>,
) -> AppResult<()> {
    let Some(retry_id) = retry_of else {
        return Ok(());
    };
    let valid = connection
        .query_row(
            "SELECT 1 FROM ai_requests
         WHERE id = ?1 AND origin_collection_id = ?2 AND request_scope = ?3
           AND status IN ('failed', 'cancelled', 'expired')",
            params![retry_id, collection_id, scope],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    if !valid {
        return Err(AppError::new(
            "ai_grid_retry_invalid",
            "실패·취소·만료된 같은 범위의 요청만 새 요청으로 다시 시도할 수 있습니다.",
        ));
    }
    Ok(())
}

fn validate_generation_layout(item_count: usize, layout: &AiGridLayout) -> AppResult<()> {
    if !(1..=MAX_GRID_ITEMS).contains(&item_count) {
        return Err(AppError::new(
            "ai_grid_item_count",
            "AI 생성 항목 수는 1~16개여야 합니다.",
        ));
    }
    if !(1_i64..=4).contains(&layout.rows)
        || !(1_i64..=4).contains(&layout.columns)
        || layout.cell_size <= 0
        || layout.canvas_width <= 0
        || layout.canvas_height <= 0
        || layout.gap_x < 0
        || layout.gap_y < 0
        || layout.border_left < 0
        || layout.border_top < 0
        || layout.border_right < 0
        || layout.border_bottom < 0
        || layout.canvas_width > i64::from(MAX_GRID_CANVAS_DIMENSION)
        || layout.canvas_height > i64::from(MAX_GRID_CANVAS_DIMENSION)
    {
        return Err(AppError::new(
            "ai_grid_layout",
            "AI 그리드는 최대 4×4, 2048×2048의 정사각 셀 배치만 지원합니다.",
        ));
    }
    let rows = usize::try_from(layout.rows)
        .map_err(|_| AppError::new("ai_grid_layout", "AI 그리드 행 수가 올바르지 않습니다."))?;
    let columns = usize::try_from(layout.columns)
        .map_err(|_| AppError::new("ai_grid_layout", "AI 그리드 열 수가 올바르지 않습니다."))?;
    let capacity = rows.checked_mul(columns).ok_or_else(|| {
        AppError::new(
            "ai_grid_layout",
            "AI 그리드 셀 수가 지원 범위를 벗어났습니다.",
        )
    })?;
    let expected_rows = item_count.div_ceil(columns);
    if capacity < item_count || rows != expected_rows {
        return Err(AppError::new(
            "ai_grid_layout",
            "AI 그리드 행·열 수가 생성 항목 수와 맞지 않습니다.",
        ));
    }
    let expected_width = checked_axis_extent(
        layout.border_left,
        layout.border_right,
        layout.columns,
        layout.cell_size,
        layout.gap_x,
    )?;
    let expected_height = checked_axis_extent(
        layout.border_top,
        layout.border_bottom,
        layout.rows,
        layout.cell_size,
        layout.gap_y,
    )?;
    let pixels = layout
        .canvas_width
        .checked_mul(layout.canvas_height)
        .ok_or_else(|| AppError::new("ai_grid_layout", "AI 그리드 픽셀 수가 너무 큽니다."))?;
    if expected_width != layout.canvas_width
        || expected_height != layout.canvas_height
        || pixels > i64::try_from(MAX_GRID_CANVAS_PIXELS).unwrap_or(i64::MAX)
    {
        return Err(AppError::new(
            "ai_grid_layout",
            "AI 그리드 셀·간격·테두리가 canvas를 정확히 채우지 않습니다.",
        ));
    }
    Ok(())
}

fn checked_axis_extent(
    leading: i64,
    trailing: i64,
    count: i64,
    cell_size: i64,
    gap: i64,
) -> AppResult<i64> {
    let cells = count.checked_mul(cell_size);
    let gaps = count
        .checked_sub(1)
        .and_then(|value| value.checked_mul(gap));
    leading
        .checked_add(trailing)
        .and_then(|value| cells.and_then(|cells| value.checked_add(cells)))
        .and_then(|value| gaps.and_then(|gaps| value.checked_add(gaps)))
        .ok_or_else(|| {
            AppError::new(
                "ai_grid_layout",
                "AI 그리드 축 길이가 지원 범위를 벗어났습니다.",
            )
        })
}
fn checked_grid_coordinate(border: i64, ordinal: i64, cell_size: i64, gap: i64) -> AppResult<i64> {
    ordinal
        .checked_mul(cell_size.checked_add(gap).ok_or_else(|| {
            AppError::new(
                "ai_grid_layout",
                "AI 그리드 좌표가 지원 범위를 벗어났습니다.",
            )
        })?)
        .and_then(|offset| border.checked_add(offset))
        .ok_or_else(|| {
            AppError::new(
                "ai_grid_layout",
                "AI 그리드 좌표가 지원 범위를 벗어났습니다.",
            )
        })
}

fn canonical_grid_manifest(raw: &str) -> AppResult<String> {
    if raw.len() > MAX_GRID_MANIFEST_BYTES {
        return Err(AppError::new(
            "ai_grid_manifest",
            "AI 그리드 manifest는 64KiB를 넘을 수 없습니다.",
        ));
    }
    let value: Value = serde_json::from_str(raw).map_err(|_| {
        AppError::new(
            "ai_grid_manifest",
            "AI 그리드 manifest JSON이 올바르지 않습니다.",
        )
    })?;
    if value.get("schema").and_then(Value::as_str) != Some(AI_GRID_SCHEMA) {
        return Err(AppError::new(
            "ai_grid_manifest",
            "AI 그리드 manifest schema가 pmtcon-ai-grid-v1이 아닙니다.",
        ));
    }
    let canonical = ai_snapshots::canonical_value(&value);
    if canonical.len() > MAX_GRID_MANIFEST_BYTES {
        return Err(AppError::new(
            "ai_grid_manifest",
            "정규화된 AI 그리드 manifest는 64KiB를 넘을 수 없습니다.",
        ));
    }
    Ok(canonical)
}

fn normalized_signature(value: &str) -> AppResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.len() > 4096 || trimmed.chars().any(char::is_control) {
        return Err(AppError::new(
            "ai_grid_signature",
            "AI 생성 요청 서명은 비어 있지 않은 4096자 이하의 값이어야 합니다.",
        ));
    }
    Ok(hash_text(&[
        AI_GRID_SCHEMA.to_string(),
        "source-free".to_string(),
        trimmed.to_string(),
    ]))
}

fn normalized_target_name(value: &str, position: usize) -> AppResult<String> {
    let trimmed = value.trim();
    if trimmed.chars().any(char::is_control) || trimmed.len() > 255 {
        return Err(AppError::new(
            "ai_grid_target_name",
            "AI 생성 항목 이름은 제어 문자 없이 255자 이하여야 합니다.",
        ));
    }
    if trimmed.is_empty() {
        Ok(format!("새 이모티콘 {}", position + 1))
    } else {
        Ok(trimmed.to_string())
    }
}

fn collection_name(connection: &Connection, collection_id: &str) -> AppResult<String> {
    connection
        .query_row(
            "SELECT name FROM collections WHERE id = ?1 AND deleted_at IS NULL",
            [collection_id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("AI 그리드를 만들 모음을 찾을 수 없습니다."))
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::time::{SystemTime, UNIX_EPOCH};

    use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};
    use rusqlite::{params, Connection};

    use crate::db::migrations;
    use crate::db::repositories::collections::create_collection;
    use crate::db::repositories::imports::import_image_files;
    use crate::models::ImportImageFilePayload;
    use crate::paths::AppPaths;
    use crate::sheet::composer::{default_ai_grid_layout, AiGridLayout, AiGridRect};
    use crate::sheet::grid::SheetGridSettings;
    use crate::sheet::importer::{create_icons_from_png_cells, CellImportInput};
    use crate::sheet::splitter::ReviewedGridDecision;

    use super::{
        analyze_ai_grid_output, cancel_ai_grid_request, commit_ai_generated_icons,
        commit_ai_grid_candidates, get_ai_grid_request_state, get_ai_grid_workspace,
        get_latest_ai_grid_workspace, mark_ai_grid_awaiting_result, prepare_ai_generation,
        prepare_ai_generation_with_references, prepare_ai_grid_edit,
        record_ai_grid_output_artifact, verified_ai_grid_input_path, FinalizeGeneratedIconInput,
        PrepareAiGenerationReferences, PrepareAiGenerationRequest,
    };

    struct Fixture {
        connection: Connection,
        paths: AppPaths,
        collection_id: String,
        icon_ids: Vec<String>,
    }

    impl Fixture {
        fn new(icon_count: usize) -> Self {
            let mut connection = Connection::open_in_memory().unwrap();
            connection
                .pragma_update(None, "foreign_keys", "ON")
                .unwrap();
            migrations::run(&mut connection).unwrap();
            let paths = temp_paths();
            let collection =
                create_collection(&mut connection, Some("AI grid repository test".to_string()))
                    .unwrap();
            let files = (0..icon_count)
                .map(|index| ImportImageFilePayload {
                    original_filename: format!("icon-{index}.png"),
                    bytes: solid_png(
                        48,
                        48,
                        [
                            40_u8.saturating_add((index * 30) as u8),
                            90_u8.saturating_add((index * 20) as u8),
                            180_u8.saturating_sub((index * 20) as u8),
                            255,
                        ],
                    ),
                })
                .collect::<Vec<_>>();
            let imported =
                import_image_files(&mut connection, &paths, &collection.id, files).unwrap();
            let icon_ids = imported
                .imported_icons
                .into_iter()
                .map(|icon| icon.id)
                .collect();
            Self {
                connection,
                paths,
                collection_id: collection.id,
                icon_ids,
            }
        }

        fn cleanup(self) {
            std::fs::remove_dir_all(self.paths.root).unwrap();
        }
    }

    fn temp_paths() -> AppPaths {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        AppPaths::prepare(std::env::temp_dir().join(format!("pmtcon-ai-grid-repo-{suffix}")))
            .unwrap()
    }

    fn solid_png(width: u32, height: u32, color: [u8; 4]) -> Vec<u8> {
        let image = ImageBuffer::from_pixel(width, height, Rgba(color));
        png_from_rgba(image)
    }

    fn two_cell_png() -> Vec<u8> {
        let image = ImageBuffer::from_fn(64, 32, |x, _| {
            if x < 32 {
                Rgba([230, 40, 70, 255])
            } else {
                Rgba([30, 180, 120, 255])
            }
        });
        png_from_rgba(image)
    }

    fn small_two_item_edit_layout() -> AiGridLayout {
        AiGridLayout {
            canvas_width: 64,
            canvas_height: 64,
            rows: 1,
            columns: 2,
            cell_size: 32,
            gap_x: 0,
            gap_y: 0,
            border_left: 0,
            border_top: 16,
            border_right: 0,
            border_bottom: 16,
        }
    }

    fn small_edit_result_png(empty_second: bool) -> Vec<u8> {
        let image = ImageBuffer::from_fn(64, 64, |x, y| {
            if (16..48).contains(&y) && x < 32 {
                Rgba([230, 40, 70, 255])
            } else if (16..48).contains(&y) && !empty_second {
                Rgba([30, 180, 120, 255])
            } else {
                Rgba([0, 0, 0, 0])
            }
        });
        png_from_rgba(image)
    }

    fn two_cell_png_with_empty_second() -> Vec<u8> {
        let image = ImageBuffer::from_fn(64, 32, |x, _| {
            if x < 32 {
                Rgba([230, 40, 70, 255])
            } else {
                Rgba([0, 0, 0, 0])
            }
        });
        png_from_rgba(image)
    }

    fn png_from_rgba(image: ImageBuffer<Rgba<u8>, Vec<u8>>) -> Vec<u8> {
        let mut cursor = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image)
            .write_to(&mut cursor, ImageFormat::Png)
            .unwrap();
        cursor.into_inner()
    }

    fn input_artifact(connection: &Connection, request_id: &str) -> (Vec<u8>, String) {
        let (path, manifest): (String, String) = connection
            .query_row(
                "SELECT source.original_path_in_library, artifact.manifest_json
             FROM ai_request_artifacts artifact
             JOIN source_files source ON source.id = artifact.source_file_id
             WHERE artifact.request_id = ?1 AND artifact.role = 'input_sheet'",
                [request_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        (std::fs::read(path).unwrap(), manifest)
    }

    fn item_decisions(connection: &Connection, request_id: &str) -> Vec<ReviewedGridDecision> {
        let mut statement = connection
            .prepare(
                "SELECT item_index, input_cell_x, input_cell_y, cell_width, cell_height
             FROM ai_request_items WHERE request_id = ?1 ORDER BY item_index ASC",
            )
            .unwrap();
        statement
            .query_map([request_id], |row| {
                Ok(ReviewedGridDecision {
                    result_cell_index: row.get(0)?,
                    target_item_index: row.get(0)?,
                    include: true,
                    crop: Some(AiGridRect {
                        x: row.get(1)?,
                        y: row.get(2)?,
                        width: row.get(3)?,
                        height: row.get(4)?,
                    }),
                })
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    }

    fn prepare_two_source_free_candidates(fixture: &mut Fixture) -> String {
        let request = prepare_ai_generation(
            &mut fixture.connection,
            &fixture.collection_id,
            PrepareAiGenerationRequest {
                target_names: vec!["왼쪽 생성".into(), "오른쪽 생성".into()],
                layout: AiGridLayout {
                    canvas_width: 64,
                    canvas_height: 32,
                    rows: 1,
                    columns: 2,
                    cell_size: 32,
                    gap_x: 0,
                    gap_y: 0,
                    border_left: 0,
                    border_top: 0,
                    border_right: 0,
                    border_bottom: 0,
                },
                payload_input_signature: "grid-2-source-free".into(),
                retry_of_request_id: None,
            },
        )
        .unwrap();
        record_ai_grid_output_artifact(
            &mut fixture.connection,
            &fixture.paths,
            &request.request_id,
            ImportImageFilePayload {
                original_filename: "generated-grid.png".into(),
                bytes: two_cell_png(),
            },
            r#"{"schema":"pmtcon-ai-grid-v1","kind":"grid-2-test"}"#,
        )
        .unwrap();
        commit_ai_grid_candidates(
            &mut fixture.connection,
            &fixture.paths,
            &request.request_id,
            vec![
                ReviewedGridDecision {
                    result_cell_index: 0,
                    target_item_index: 0,
                    include: true,
                    crop: Some(AiGridRect {
                        x: 0,
                        y: 0,
                        width: 32,
                        height: 32,
                    }),
                },
                ReviewedGridDecision {
                    result_cell_index: 1,
                    target_item_index: 1,
                    include: true,
                    crop: Some(AiGridRect {
                        x: 32,
                        y: 0,
                        width: 32,
                        height: 32,
                    }),
                },
            ],
        )
        .unwrap();
        request.request_id
    }

    fn materialize_source_free_icon_for_clone(
        fixture: &mut Fixture,
        request_id: &str,
        candidate_id: &str,
    ) -> String {
        let (item_id, source_file_id, source_path): (String, String, String) = fixture
            .connection
            .query_row(
                "SELECT item.id, candidate.raw_source_file_id, source.original_path_in_library
                 FROM ai_candidates candidate
                 JOIN ai_request_items item ON item.id = candidate.request_item_id
                 JOIN source_files source ON source.id = candidate.raw_source_file_id
                 WHERE candidate.id = ?1 AND candidate.request_id = ?2",
                params![candidate_id, request_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        let bytes = std::fs::read(source_path).unwrap();
        let icon = create_icons_from_png_cells(
            &mut fixture.connection,
            &fixture.paths,
            &fixture.collection_id,
            vec![CellImportInput {
                original_filename: "source-free-root.png".into(),
                bytes,
                display_name: "원본 없는 생성".into(),
                alt_text: String::new(),
                cell_width: Some(32),
                cell_height: Some(32),
            }],
        )
        .unwrap()
        .remove(0);
        let imported_source_file_id: String = fixture
            .connection
            .query_row(
                "SELECT source_file_id FROM icons WHERE id = ?1",
                [&icon.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(imported_source_file_id, source_file_id);

        let tx = fixture.connection.transaction().unwrap();
        tx.execute(
            "INSERT INTO ai_icon_root_creations (
               icon_id, source_icon_id, candidate_id, request_item_id, creation_kind,
               normalization_recipe_hash, created_at
             ) VALUES (
               ?1, NULL, ?2, ?3, 'source_free', NULL,
               strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             )",
            params![icon.id, candidate_id, item_id],
        )
        .unwrap();
        tx.execute(
            "UPDATE ai_request_items
             SET review_status = 'icon_created',
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?1 AND request_id = ?2
               AND output_candidate_id = ?3 AND review_status = 'included'",
            params![item_id, request_id, candidate_id],
        )
        .unwrap();
        tx.execute(
            "UPDATE ai_requests
             SET status = 'completed',
                 completed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?1 AND status = 'layout_review_pending'",
            [request_id],
        )
        .unwrap();
        tx.commit().unwrap();
        icon.id
    }
    #[test]
    fn edit_prepare_is_deterministic_persistent_and_preserves_current_sources() {
        let mut fixture = Fixture::new(2);
        let before_sources = fixture
            .icon_ids
            .iter()
            .map(|icon_id| {
                fixture
                    .connection
                    .query_row(
                        "SELECT source_file_id FROM icons WHERE id = ?1",
                        [icon_id],
                        |row| row.get::<_, String>(0),
                    )
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let layout = default_ai_grid_layout(2, 1024).unwrap();
        let first = prepare_ai_grid_edit(
            &mut fixture.connection,
            &fixture.paths,
            &fixture.collection_id,
            fixture.icon_ids.iter().rev().cloned().collect(),
            layout.clone(),
            None,
        )
        .unwrap();
        assert_eq!(first.status, "prepared");
        assert_eq!(first.item_count, 2);
        let ordered_ids = {
            let mut statement = fixture
                .connection
                .prepare(
                    "SELECT origin_icon_id_snapshot FROM ai_request_items
                 WHERE request_id = ?1 ORDER BY item_index",
                )
                .unwrap();
            statement
                .query_map([first.request_id.as_str()], |row| row.get::<_, String>(0))
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
        };
        assert_eq!(ordered_ids, fixture.icon_ids);
        let second = prepare_ai_grid_edit(
            &mut fixture.connection,
            &fixture.paths,
            &fixture.collection_id,
            fixture.icon_ids.clone(),
            layout,
            None,
        )
        .unwrap();
        assert_eq!(first.input_sheet_sha256, second.input_sheet_sha256);
        assert_eq!(first.input_manifest_sha256, second.input_manifest_sha256);
        let input_path: String = fixture
            .connection
            .query_row(
                "SELECT source.original_path_in_library
                 FROM ai_request_artifacts artifact
                 JOIN source_files source ON source.id = artifact.source_file_id
                 WHERE artifact.request_id = ?1 AND artifact.role = 'input_sheet'",
                [&first.request_id],
                |row| row.get(0),
            )
            .unwrap();
        crate::db::repositories::library::cleanup_library(
            &fixture.connection,
            &fixture.paths,
            true,
        )
        .unwrap();
        assert!(std::path::Path::new(&input_path).is_file());
        let after_sources = fixture
            .icon_ids
            .iter()
            .map(|icon_id| {
                fixture
                    .connection
                    .query_row(
                        "SELECT source_file_id FROM icons WHERE id = ?1",
                        [icon_id],
                        |row| row.get::<_, String>(0),
                    )
                    .unwrap()
            })
            .collect::<Vec<_>>();
        assert_eq!(before_sources, after_sources);
        fixture.cleanup();
    }

    #[test]
    fn generation_references_persist_one_managed_sheet_without_mutating_sources() {
        let mut fixture = Fixture::new(1);
        let original_source: (String, Option<String>) = fixture
            .connection
            .query_row(
                "SELECT source_file_id, current_preview_path FROM icons WHERE id = ?1",
                [&fixture.icon_ids[0]],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        let prepared = prepare_ai_generation_with_references(
            &mut fixture.connection,
            &fixture.paths,
            &fixture.collection_id,
            PrepareAiGenerationRequest {
                target_names: vec!["기쁨".into(), "놀람".into()],
                layout: default_ai_grid_layout(2, 1024).unwrap(),
                payload_input_signature: "reference-generation-v1".into(),
                retry_of_request_id: None,
            },
            PrepareAiGenerationReferences {
                selected_icon_ids: vec![fixture.icon_ids[0].clone()],
                external_files: vec![ImportImageFilePayload {
                    original_filename: "style-reference.png".into(),
                    bytes: solid_png(24, 48, [210, 40, 120, 255]),
                }],
            },
        )
        .unwrap();

        assert_eq!(prepared.request_scope, "grid_generate");
        assert!(prepared.input_sheet_sha256.is_some());
        let workspace = get_ai_grid_workspace(&fixture.connection, &prepared.request_id).unwrap();
        let input = workspace.input_artifact.as_ref().unwrap();
        assert!(std::path::Path::new(&input.file_path).is_file());
        assert!(input.manifest_json.contains("generation_reference"));
        assert!(input.manifest_json.contains("library_icon"));
        assert!(input.manifest_json.contains("external_file"));
        let request_hashes: (Option<String>, Option<String>) = fixture
            .connection
            .query_row(
                "SELECT input_package_sha256, reference_package_sha256
                 FROM ai_requests WHERE id = ?1",
                [&prepared.request_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(request_hashes.0, Some(input.sha256.clone()));
        assert_eq!(request_hashes.1, Some(input.sha256.clone()));
        let verified =
            verified_ai_grid_input_path(&fixture.connection, &fixture.paths, &prepared.request_id)
                .unwrap();
        assert!(verified.is_file());
        let current_source: (String, Option<String>) = fixture
            .connection
            .query_row(
                "SELECT source_file_id, current_preview_path FROM icons WHERE id = ?1",
                [&fixture.icon_ids[0]],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(current_source, original_source);
        fixture.cleanup();
    }

    #[test]
    fn source_free_prepare_creates_no_placeholder_and_retry_is_a_new_request() {
        let mut fixture = Fixture::new(0);
        let layout = default_ai_grid_layout(3, 1024).unwrap();
        let original = prepare_ai_generation(
            &mut fixture.connection,
            &fixture.collection_id,
            PrepareAiGenerationRequest {
                target_names: vec!["기쁨".into(), "화남".into(), "놀람".into()],
                layout: layout.clone(),
                payload_input_signature: "three-emotions-v1".into(),
                retry_of_request_id: None,
            },
        )
        .unwrap();
        let icon_count: i64 = fixture
            .connection
            .query_row(
                "SELECT COUNT(*) FROM icons WHERE collection_id = ?1",
                [&fixture.collection_id],
                |row| row.get(0),
            )
            .unwrap();
        let origin_count: i64 = fixture
            .connection
            .query_row(
                "SELECT COUNT(*) FROM ai_request_items
             WHERE request_id = ?1 AND (
               origin_icon_id IS NOT NULL OR origin_icon_id_snapshot IS NOT NULL
               OR original_lineage_id IS NOT NULL OR original_source_sha256 IS NOT NULL
             )",
                [&original.request_id],
                |row| row.get(0),
            )
            .unwrap();
        let input_artifacts: i64 = fixture.connection.query_row(
            "SELECT COUNT(*) FROM ai_request_artifacts WHERE request_id = ?1 AND role = 'input_sheet'",
            [&original.request_id], |row| row.get(0),
        ).unwrap();
        assert_eq!((icon_count, origin_count, input_artifacts), (0, 0, 0));
        cancel_ai_grid_request(&fixture.connection, &original.request_id).unwrap();
        let retry = prepare_ai_generation(
            &mut fixture.connection,
            &fixture.collection_id,
            PrepareAiGenerationRequest {
                target_names: vec!["기쁨".into(), "화남".into(), "놀람".into()],
                layout,
                payload_input_signature: "three-emotions-v1".into(),
                retry_of_request_id: Some(original.request_id.clone()),
            },
        )
        .unwrap();
        assert_ne!(retry.request_id, original.request_id);
        assert_eq!(retry.retry_of_request_id, Some(original.request_id));
        fixture.cleanup();
    }
    #[test]
    fn edit_commit_rejects_output_canvas_mismatch_before_creating_candidates() {
        let mut fixture = Fixture::new(2);
        let prepared = prepare_ai_grid_edit(
            &mut fixture.connection,
            &fixture.paths,
            &fixture.collection_id,
            fixture.icon_ids.clone(),
            small_two_item_edit_layout(),
            None,
        )
        .unwrap();
        record_ai_grid_output_artifact(
            &mut fixture.connection,
            &fixture.paths,
            &prepared.request_id,
            ImportImageFilePayload {
                original_filename: "wrong-canvas.png".into(),
                bytes: two_cell_png(),
            },
            r#"{"schema":"pmtcon-ai-grid-v1","kind":"wrong-canvas"}"#,
        )
        .unwrap();

        let decisions = item_decisions(&fixture.connection, &prepared.request_id);
        let error = commit_ai_grid_candidates(
            &mut fixture.connection,
            &fixture.paths,
            &prepared.request_id,
            decisions,
        )
        .unwrap_err();
        assert_eq!(error.code, "ai_grid_output_structure");
        let state = get_ai_grid_request_state(&fixture.connection, &prepared.request_id).unwrap();
        assert_eq!(
            (state.status.as_str(), state.candidate_count),
            ("layout_review_pending", 0)
        );
        fixture.cleanup();
    }

    #[test]
    fn edit_commit_rejects_empty_required_output_cell_without_partial_rows() {
        let mut fixture = Fixture::new(2);
        let prepared = prepare_ai_grid_edit(
            &mut fixture.connection,
            &fixture.paths,
            &fixture.collection_id,
            fixture.icon_ids.clone(),
            small_two_item_edit_layout(),
            None,
        )
        .unwrap();
        record_ai_grid_output_artifact(
            &mut fixture.connection,
            &fixture.paths,
            &prepared.request_id,
            ImportImageFilePayload {
                original_filename: "empty-required-cell.png".into(),
                bytes: small_edit_result_png(true),
            },
            r#"{"schema":"pmtcon-ai-grid-v1","kind":"empty-cell"}"#,
        )
        .unwrap();

        let decisions = item_decisions(&fixture.connection, &prepared.request_id);
        let error = commit_ai_grid_candidates(
            &mut fixture.connection,
            &fixture.paths,
            &prepared.request_id,
            decisions,
        )
        .unwrap_err();
        assert_eq!(error.code, "ai_grid_output_empty_cell");
        let item_state: (i64, i64) = fixture
            .connection
            .query_row(
                "SELECT COUNT(*), SUM(review_status = 'pending')
                 FROM ai_request_items WHERE request_id = ?1",
                [&prepared.request_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(item_state, (2, 2));
        assert_eq!(
            get_ai_grid_request_state(&fixture.connection, &prepared.request_id)
                .unwrap()
                .candidate_count,
            0
        );
        fixture.cleanup();
    }

    #[test]
    fn edit_result_candidates_commit_all_or_none_and_remain_inactive() {
        let mut fixture = Fixture::new(2);
        let before_sources = fixture
            .icon_ids
            .iter()
            .map(|icon_id| {
                fixture
                    .connection
                    .query_row(
                        "SELECT source_file_id FROM icons WHERE id = ?1",
                        [icon_id],
                        |row| row.get::<_, String>(0),
                    )
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let prepared = prepare_ai_grid_edit(
            &mut fixture.connection,
            &fixture.paths,
            &fixture.collection_id,
            fixture.icon_ids.clone(),
            default_ai_grid_layout(2, 1024).unwrap(),
            None,
        )
        .unwrap();
        let (bytes, manifest) = input_artifact(&fixture.connection, &prepared.request_id);
        mark_ai_grid_awaiting_result(&fixture.connection, &prepared.request_id).unwrap();
        let pending = record_ai_grid_output_artifact(
            &mut fixture.connection,
            &fixture.paths,
            &prepared.request_id,
            ImportImageFilePayload {
                original_filename: "result.png".into(),
                bytes,
            },
            &manifest,
        )
        .unwrap();
        assert_eq!(pending.status, "layout_review_pending");
        let decisions = item_decisions(&fixture.connection, &prepared.request_id);
        let committed = commit_ai_grid_candidates(
            &mut fixture.connection,
            &fixture.paths,
            &prepared.request_id,
            decisions,
        )
        .unwrap();
        assert_eq!(committed.candidate_ids.len(), 2);
        assert!(committed.rejected_item_indexes.is_empty());
        let state = get_ai_grid_request_state(&fixture.connection, &prepared.request_id).unwrap();
        assert_eq!(
            (state.status.as_str(), state.candidate_count),
            ("completed", 2)
        );
        let request_count_before: i64 = fixture
            .connection
            .query_row("SELECT COUNT(*) FROM ai_requests", [], |row| row.get(0))
            .unwrap();
        let retry_error = prepare_ai_grid_edit(
            &mut fixture.connection,
            &fixture.paths,
            &fixture.collection_id,
            fixture.icon_ids.clone(),
            default_ai_grid_layout(2, 1024).unwrap(),
            Some(prepared.request_id.clone()),
        )
        .unwrap_err();
        assert_eq!(retry_error.code, "ai_grid_retry_invalid");
        let request_count_after: i64 = fixture
            .connection
            .query_row("SELECT COUNT(*) FROM ai_requests", [], |row| row.get(0))
            .unwrap();
        assert_eq!(request_count_after, request_count_before);
        let active_versions: i64 = fixture
            .connection
            .query_row(
                "SELECT COUNT(*) FROM icon_ai_state WHERE active_version_id IS NOT NULL",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let after_sources = fixture
            .icon_ids
            .iter()
            .map(|icon_id| {
                fixture
                    .connection
                    .query_row(
                        "SELECT source_file_id FROM icons WHERE id = ?1",
                        [icon_id],
                        |row| row.get::<_, String>(0),
                    )
                    .unwrap()
            })
            .collect::<Vec<_>>();
        assert_eq!(active_versions, 0);
        assert_eq!(before_sources, after_sources);
        for icon_id in &fixture.icon_ids {
            let review = crate::db::repositories::ai::get_ai_review_state(
                &fixture.connection,
                &fixture.collection_id,
                icon_id,
            )
            .unwrap();
            assert_eq!(review.candidates.len(), 1);
            assert!(!review.candidates[0].is_stale);
            let preview = crate::db::repositories::ai::preview_ai_candidate_normalization(
                &fixture.connection,
                &fixture.paths,
                &fixture.collection_id,
                crate::models::PreviewAiCandidateNormalizationPayload {
                    icon_id: icon_id.clone(),
                    candidate_id: review.candidates[0].id.clone(),
                    expected_revision: review.visual_source.activation_revision,
                    normalization: crate::models::AiNormalizationOptionsPayload::default(),
                },
            )
            .unwrap();
            assert!(preview.current_icon_compatibility.allowed);
        }
        fixture.cleanup();
    }

    #[test]
    fn verified_grid_input_path_rejects_tampered_managed_artifact() {
        let mut fixture = Fixture::new(2);
        let prepared = prepare_ai_grid_edit(
            &mut fixture.connection,
            &fixture.paths,
            &fixture.collection_id,
            fixture.icon_ids.clone(),
            default_ai_grid_layout(2, 1024).unwrap(),
            None,
        )
        .unwrap();
        let input_path =
            verified_ai_grid_input_path(&fixture.connection, &fixture.paths, &prepared.request_id)
                .unwrap();
        assert!(input_path.is_file());

        std::fs::write(&input_path, b"tampered-grid-input").unwrap();
        let error =
            verified_ai_grid_input_path(&fixture.connection, &fixture.paths, &prepared.request_id)
                .unwrap_err();
        assert_eq!(error.code, "ai_source_repair_required");
        fixture.cleanup();
    }

    #[test]
    fn verified_grid_input_path_only_allows_live_edit_delivery_stage() {
        let mut fixture = Fixture::new(2);
        let prepared = prepare_ai_grid_edit(
            &mut fixture.connection,
            &fixture.paths,
            &fixture.collection_id,
            fixture.icon_ids.clone(),
            default_ai_grid_layout(2, 1024).unwrap(),
            None,
        )
        .unwrap();
        assert!(verified_ai_grid_input_path(
            &fixture.connection,
            &fixture.paths,
            &prepared.request_id
        )
        .unwrap()
        .is_file());

        cancel_ai_grid_request(&fixture.connection, &prepared.request_id).unwrap();
        let error =
            verified_ai_grid_input_path(&fixture.connection, &fixture.paths, &prepared.request_id)
                .unwrap_err();
        assert_eq!(error.code, "ai_grid_input_not_live");
        fixture.cleanup();
    }
    #[test]
    fn verified_grid_input_path_rejects_valid_file_outside_app_managed_root() {
        let mut fixture = Fixture::new(2);
        let prepared = prepare_ai_grid_edit(
            &mut fixture.connection,
            &fixture.paths,
            &fixture.collection_id,
            fixture.icon_ids.clone(),
            default_ai_grid_layout(2, 1024).unwrap(),
            None,
        )
        .unwrap();
        let (source_file_id, managed_path): (String, String) = fixture
            .connection
            .query_row(
                "SELECT artifact.source_file_id, source.original_path_in_library
                 FROM ai_request_artifacts artifact
                 JOIN source_files source ON source.id = artifact.source_file_id
                 WHERE artifact.request_id = ?1 AND artifact.role = 'input_sheet'",
                [&prepared.request_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        let outside_path = fixture.paths.root.with_extension("outside-grid-input.png");
        std::fs::copy(&managed_path, &outside_path).unwrap();
        fixture
            .connection
            .execute(
                "UPDATE source_files SET original_path_in_library = ?1 WHERE id = ?2",
                params![outside_path.to_string_lossy(), source_file_id],
            )
            .unwrap();

        let error =
            verified_ai_grid_input_path(&fixture.connection, &fixture.paths, &prepared.request_id)
                .unwrap_err();
        assert_eq!(error.code, "ai_grid_input_unmanaged_path");

        std::fs::remove_file(outside_path).unwrap();
        fixture.cleanup();
    }

    #[test]
    fn verified_grid_input_path_never_follows_file_symlinks_or_reparse_points() {
        let mut fixture = Fixture::new(2);
        let prepared = prepare_ai_grid_edit(
            &mut fixture.connection,
            &fixture.paths,
            &fixture.collection_id,
            fixture.icon_ids.clone(),
            default_ai_grid_layout(2, 1024).unwrap(),
            None,
        )
        .unwrap();
        let (source_file_id, managed_path): (String, String) = fixture
            .connection
            .query_row(
                "SELECT artifact.source_file_id, source.original_path_in_library
                 FROM ai_request_artifacts artifact
                 JOIN source_files source ON source.id = artifact.source_file_id
                 WHERE artifact.request_id = ?1 AND artifact.role = 'input_sheet'",
                [&prepared.request_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        let outside_path = fixture.paths.root.with_extension("grid-link-target.png");
        std::fs::copy(&managed_path, &outside_path).unwrap();
        let link_path = fixture.paths.originals_dir.join("linked-grid-input.png");
        #[cfg(windows)]
        let link_result = std::os::windows::fs::symlink_file(&outside_path, &link_path);
        #[cfg(unix)]
        let link_result = std::os::unix::fs::symlink(&outside_path, &link_path);
        #[cfg(not(any(windows, unix)))]
        let link_result: std::io::Result<()> = Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "file symlinks are not supported on this target",
        ));
        if let Err(error) = link_result {
            eprintln!("symlink/reparse assertion skipped: {error}");
            std::fs::remove_file(outside_path).unwrap();
            fixture.cleanup();
            return;
        }
        fixture
            .connection
            .execute(
                "UPDATE source_files SET original_path_in_library = ?1 WHERE id = ?2",
                params![link_path.to_string_lossy(), source_file_id],
            )
            .unwrap();

        let error =
            verified_ai_grid_input_path(&fixture.connection, &fixture.paths, &prepared.request_id)
                .unwrap_err();
        assert_eq!(error.code, "ai_grid_input_unmanaged_path");

        std::fs::remove_file(link_path).unwrap();
        std::fs::remove_file(outside_path).unwrap();
        fixture.cleanup();
    }

    #[test]
    fn stale_target_aborts_candidate_and_source_batch_without_partial_rows() {
        let mut fixture = Fixture::new(2);
        let prepared = prepare_ai_grid_edit(
            &mut fixture.connection,
            &fixture.paths,
            &fixture.collection_id,
            fixture.icon_ids.clone(),
            default_ai_grid_layout(2, 1024).unwrap(),
            None,
        )
        .unwrap();
        let (bytes, manifest) = input_artifact(&fixture.connection, &prepared.request_id);
        mark_ai_grid_awaiting_result(&fixture.connection, &prepared.request_id).unwrap();
        record_ai_grid_output_artifact(
            &mut fixture.connection,
            &fixture.paths,
            &prepared.request_id,
            ImportImageFilePayload {
                original_filename: "result.png".into(),
                bytes,
            },
            &manifest,
        )
        .unwrap();
        fixture.connection.execute(
            "UPDATE crop_settings SET crop_x = crop_x + 1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE icon_id = ?1",
            [&fixture.icon_ids[1]],
        ).unwrap();
        let source_count_before: i64 = fixture
            .connection
            .query_row("SELECT COUNT(*) FROM source_files", [], |row| row.get(0))
            .unwrap();
        let decisions = item_decisions(&fixture.connection, &prepared.request_id);
        let error = commit_ai_grid_candidates(
            &mut fixture.connection,
            &fixture.paths,
            &prepared.request_id,
            decisions,
        )
        .unwrap_err();
        assert_eq!(error.code, "ai_grid_target_stale");
        let candidate_count: i64 = fixture
            .connection
            .query_row(
                "SELECT COUNT(*) FROM ai_candidates WHERE request_id = ?1",
                [&prepared.request_id],
                |row| row.get(0),
            )
            .unwrap();
        let source_count_after: i64 = fixture
            .connection
            .query_row("SELECT COUNT(*) FROM source_files", [], |row| row.get(0))
            .unwrap();
        assert_eq!(
            (candidate_count, source_count_after),
            (0, source_count_before)
        );
        assert_eq!(
            get_ai_grid_request_state(&fixture.connection, &prepared.request_id)
                .unwrap()
                .status,
            "layout_review_pending"
        );
        fixture.cleanup();
    }

    #[test]
    fn deleting_one_grid_target_cancels_the_whole_pending_request() {
        let mut fixture = Fixture::new(2);
        let prepared = prepare_ai_grid_edit(
            &mut fixture.connection,
            &fixture.paths,
            &fixture.collection_id,
            fixture.icon_ids.clone(),
            default_ai_grid_layout(2, 1024).unwrap(),
            None,
        )
        .unwrap();
        crate::db::repositories::icons::delete_icons(
            &mut fixture.connection,
            &fixture.collection_id,
            vec![fixture.icon_ids[0].clone()],
        )
        .unwrap();
        let state = get_ai_grid_request_state(&fixture.connection, &prepared.request_id).unwrap();
        assert_eq!(state.status, "cancelled");
        let candidates: i64 = fixture
            .connection
            .query_row(
                "SELECT COUNT(*) FROM ai_candidates WHERE request_id = ?1",
                [&prepared.request_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(candidates, 0);
        fixture.cleanup();
    }

    #[test]
    fn source_free_commit_rejects_output_structure_mismatch_before_review_rows_change() {
        let mut fixture = Fixture::new(0);
        let layout = AiGridLayout {
            canvas_width: 64,
            canvas_height: 32,
            rows: 1,
            columns: 2,
            cell_size: 32,
            gap_x: 0,
            gap_y: 0,
            border_left: 0,
            border_top: 0,
            border_right: 0,
            border_bottom: 0,
        };
        let prepared = prepare_ai_generation(
            &mut fixture.connection,
            &fixture.collection_id,
            PrepareAiGenerationRequest {
                target_names: vec!["first".into(), "second".into()],
                layout,
                payload_input_signature: "source-free-wrong-structure".into(),
                retry_of_request_id: None,
            },
        )
        .unwrap();
        record_ai_grid_output_artifact(
            &mut fixture.connection,
            &fixture.paths,
            &prepared.request_id,
            ImportImageFilePayload {
                original_filename: "wrong-source-free-canvas.png".into(),
                bytes: solid_png(32, 64, [80, 160, 220, 255]),
            },
            r#"{"schema":"pmtcon-ai-grid-v1","kind":"wrong-source-free"}"#,
        )
        .unwrap();
        let decisions = vec![
            ReviewedGridDecision {
                result_cell_index: 0,
                target_item_index: 0,
                include: true,
                crop: Some(AiGridRect {
                    x: 0,
                    y: 0,
                    width: 32,
                    height: 32,
                }),
            },
            ReviewedGridDecision {
                result_cell_index: 1,
                target_item_index: 1,
                include: true,
                crop: Some(AiGridRect {
                    x: 32,
                    y: 0,
                    width: 32,
                    height: 32,
                }),
            },
        ];

        let error = commit_ai_grid_candidates(
            &mut fixture.connection,
            &fixture.paths,
            &prepared.request_id,
            decisions,
        )
        .unwrap_err();
        assert_eq!(error.code, "ai_grid_output_structure");
        let state = get_ai_grid_request_state(&fixture.connection, &prepared.request_id).unwrap();
        assert_eq!(
            (state.status.as_str(), state.candidate_count),
            ("layout_review_pending", 0)
        );
        fixture.cleanup();
    }

    #[test]
    fn source_free_empty_exclusion_and_finalized_inputs_must_match_atomically() {
        let mut fixture = Fixture::new(0);
        let layout = AiGridLayout {
            canvas_width: 64,
            canvas_height: 32,
            rows: 1,
            columns: 2,
            cell_size: 32,
            gap_x: 0,
            gap_y: 0,
            border_left: 0,
            border_top: 0,
            border_right: 0,
            border_bottom: 0,
        };
        let prepared = prepare_ai_generation(
            &mut fixture.connection,
            &fixture.collection_id,
            PrepareAiGenerationRequest {
                target_names: vec!["kept".into(), "empty".into()],
                layout,
                payload_input_signature: "source-free-empty-exclusion".into(),
                retry_of_request_id: None,
            },
        )
        .unwrap();
        record_ai_grid_output_artifact(
            &mut fixture.connection,
            &fixture.paths,
            &prepared.request_id,
            ImportImageFilePayload {
                original_filename: "source-free-empty-cell.png".into(),
                bytes: two_cell_png_with_empty_second(),
            },
            r#"{"schema":"pmtcon-ai-grid-v1","kind":"empty-exclusion"}"#,
        )
        .unwrap();
        let review = commit_ai_grid_candidates(
            &mut fixture.connection,
            &fixture.paths,
            &prepared.request_id,
            vec![
                ReviewedGridDecision {
                    result_cell_index: 0,
                    target_item_index: 0,
                    include: true,
                    crop: Some(AiGridRect {
                        x: 0,
                        y: 0,
                        width: 32,
                        height: 32,
                    }),
                },
                ReviewedGridDecision {
                    result_cell_index: 1,
                    target_item_index: 1,
                    include: false,
                    crop: None,
                },
            ],
        )
        .unwrap();
        assert_eq!(review.candidate_ids.len(), 1);
        assert_eq!(review.rejected_item_indexes, vec![1]);

        let mismatch = commit_ai_generated_icons(
            &mut fixture.connection,
            &fixture.paths,
            &fixture.collection_id,
            &prepared.request_id,
            vec![
                FinalizeGeneratedIconInput {
                    item_index: 0,
                    display_name: "kept".into(),
                    alt_text: String::new(),
                },
                FinalizeGeneratedIconInput {
                    item_index: 1,
                    display_name: "excluded".into(),
                    alt_text: String::new(),
                },
            ],
        )
        .unwrap_err();
        assert_eq!(mismatch.code, "ai_grid_finalize_mapping");
        let icon_count: i64 = fixture
            .connection
            .query_row(
                "SELECT COUNT(*) FROM icons WHERE collection_id = ?1",
                [&fixture.collection_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(icon_count, 0);

        let created = commit_ai_generated_icons(
            &mut fixture.connection,
            &fixture.paths,
            &fixture.collection_id,
            &prepared.request_id,
            vec![FinalizeGeneratedIconInput {
                item_index: 0,
                display_name: "kept".into(),
                alt_text: String::new(),
            }],
        )
        .unwrap();
        assert_eq!(created.created_icons.len(), 1);
        assert_eq!(
            get_ai_grid_request_state(&fixture.connection, &prepared.request_id)
                .unwrap()
                .status,
            "completed"
        );
        fixture.cleanup();
    }

    #[test]
    fn source_free_cells_persist_without_creating_icons() {
        let mut fixture = Fixture::new(0);
        let layout = AiGridLayout {
            canvas_width: 64,
            canvas_height: 32,
            rows: 1,
            columns: 2,
            cell_size: 32,
            gap_x: 0,
            gap_y: 0,
            border_left: 0,
            border_top: 0,
            border_right: 0,
            border_bottom: 0,
        };
        let prepared = prepare_ai_generation(
            &mut fixture.connection,
            &fixture.collection_id,
            PrepareAiGenerationRequest {
                target_names: vec!["왼쪽".into(), "오른쪽".into()],
                layout,
                payload_input_signature: "two-icons".into(),
                retry_of_request_id: None,
            },
        )
        .unwrap();
        mark_ai_grid_awaiting_result(&fixture.connection, &prepared.request_id).unwrap();
        record_ai_grid_output_artifact(
            &mut fixture.connection,
            &fixture.paths,
            &prepared.request_id,
            ImportImageFilePayload {
                original_filename: "generated.png".into(),
                bytes: two_cell_png(),
            },
            r#"{"schema":"pmtcon-ai-grid-v1","kind":"mock-output"}"#,
        )
        .unwrap();
        let decisions = vec![
            ReviewedGridDecision {
                result_cell_index: 0,
                target_item_index: 0,
                include: true,
                crop: Some(AiGridRect {
                    x: 0,
                    y: 0,
                    width: 32,
                    height: 32,
                }),
            },
            ReviewedGridDecision {
                result_cell_index: 1,
                target_item_index: 1,
                include: true,
                crop: Some(AiGridRect {
                    x: 32,
                    y: 0,
                    width: 32,
                    height: 32,
                }),
            },
        ];
        let result = commit_ai_grid_candidates(
            &mut fixture.connection,
            &fixture.paths,
            &prepared.request_id,
            decisions,
        )
        .unwrap();
        assert_eq!(result.candidate_ids.len(), 2);
        let state = get_ai_grid_request_state(&fixture.connection, &prepared.request_id).unwrap();
        assert_eq!(
            (state.status.as_str(), state.candidate_count),
            ("layout_review_pending", 2)
        );
        let item_state: (i64, i64) = fixture
            .connection
            .query_row(
                "SELECT COUNT(*), SUM(review_status = 'included')
             FROM ai_request_items WHERE request_id = ?1 AND output_candidate_id IS NOT NULL",
                [&prepared.request_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        let icon_count: i64 = fixture
            .connection
            .query_row(
                "SELECT COUNT(*) FROM icons WHERE collection_id = ?1",
                [&fixture.collection_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(item_state, (2, 2));
        assert_eq!(icon_count, 0);
        fixture.cleanup();
    }
    #[test]
    fn source_free_single_result_uses_shared_review_path_without_placeholder() {
        let mut fixture = Fixture::new(0);
        let layout = AiGridLayout {
            canvas_width: 32,
            canvas_height: 32,
            rows: 1,
            columns: 1,
            cell_size: 32,
            gap_x: 0,
            gap_y: 0,
            border_left: 0,
            border_top: 0,
            border_right: 0,
            border_bottom: 0,
        };
        let prepared = prepare_ai_generation(
            &mut fixture.connection,
            &fixture.collection_id,
            PrepareAiGenerationRequest {
                target_names: vec!["단일 생성".into()],
                layout,
                payload_input_signature: "single-icon".into(),
                retry_of_request_id: None,
            },
        )
        .unwrap();
        assert_eq!(prepared.request_scope, "single_generate");
        mark_ai_grid_awaiting_result(&fixture.connection, &prepared.request_id).unwrap();
        record_ai_grid_output_artifact(
            &mut fixture.connection,
            &fixture.paths,
            &prepared.request_id,
            ImportImageFilePayload {
                original_filename: "generated-single.png".into(),
                bytes: solid_png(32, 32, [80, 160, 220, 255]),
            },
            r#"{"schema":"pmtcon-ai-grid-v1","kind":"mock-single-output"}"#,
        )
        .unwrap();
        let result = commit_ai_grid_candidates(
            &mut fixture.connection,
            &fixture.paths,
            &prepared.request_id,
            vec![ReviewedGridDecision {
                result_cell_index: 0,
                target_item_index: 0,
                include: true,
                crop: Some(AiGridRect {
                    x: 0,
                    y: 0,
                    width: 32,
                    height: 32,
                }),
            }],
        )
        .unwrap();
        assert_eq!(result.candidate_ids.len(), 1);
        let state = get_ai_grid_request_state(&fixture.connection, &prepared.request_id).unwrap();
        assert_eq!(
            (
                state.status.as_str(),
                state.item_count,
                state.candidate_count
            ),
            ("layout_review_pending", 1, 1)
        );
        let (linked_items, origin_items): (i64, i64) = fixture
            .connection
            .query_row(
                "SELECT SUM(output_candidate_id IS NOT NULL), SUM(origin_icon_id IS NOT NULL)
                 FROM ai_request_items WHERE request_id = ?1 AND review_status = 'included'",
                [&prepared.request_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        let icon_count: i64 = fixture
            .connection
            .query_row(
                "SELECT COUNT(*) FROM icons WHERE collection_id = ?1",
                [&fixture.collection_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!((linked_items, origin_items, icon_count), (1, 0, 0));
        fixture.cleanup();
    }
    #[test]
    fn source_free_candidate_history_survives_icon_and_collection_clone() {
        let mut fixture = Fixture::new(0);
        let prepared = prepare_ai_generation(
            &mut fixture.connection,
            &fixture.collection_id,
            PrepareAiGenerationRequest {
                target_names: vec!["복제할 생성 아이콘".into()],
                layout: AiGridLayout {
                    canvas_width: 32,
                    canvas_height: 32,
                    rows: 1,
                    columns: 1,
                    cell_size: 32,
                    gap_x: 0,
                    gap_y: 0,
                    border_left: 0,
                    border_top: 0,
                    border_right: 0,
                    border_bottom: 0,
                },
                payload_input_signature: "clone-source-free".into(),
                retry_of_request_id: None,
            },
        )
        .unwrap();
        mark_ai_grid_awaiting_result(&fixture.connection, &prepared.request_id).unwrap();
        record_ai_grid_output_artifact(
            &mut fixture.connection,
            &fixture.paths,
            &prepared.request_id,
            ImportImageFilePayload {
                original_filename: "clone-source-free.png".into(),
                bytes: solid_png(32, 32, [130, 70, 220, 255]),
            },
            r#"{"schema":"pmtcon-ai-grid-v1","kind":"clone-fixture"}"#,
        )
        .unwrap();
        let committed = commit_ai_grid_candidates(
            &mut fixture.connection,
            &fixture.paths,
            &prepared.request_id,
            vec![ReviewedGridDecision {
                result_cell_index: 0,
                target_item_index: 0,
                include: true,
                crop: Some(AiGridRect {
                    x: 0,
                    y: 0,
                    width: 32,
                    height: 32,
                }),
            }],
        )
        .unwrap();
        let candidate_id = committed.candidate_ids[0].clone();
        let source_icon_id = materialize_source_free_icon_for_clone(
            &mut fixture,
            &prepared.request_id,
            &candidate_id,
        );
        let source_root: (String, String, Option<String>) = fixture
            .connection
            .query_row(
                "SELECT creation_kind, candidate_id, source_icon_id
                 FROM ai_icon_root_creations WHERE icon_id = ?1",
                [&source_icon_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(
            source_root,
            ("source_free".into(), candidate_id.clone(), None)
        );

        let icon_clone = crate::db::repositories::icons::duplicate_icon(
            &mut fixture.connection,
            &fixture.paths,
            &fixture.collection_id,
            &source_icon_id,
        )
        .unwrap();
        let clone_root: (String, String, Option<String>) = fixture
            .connection
            .query_row(
                "SELECT creation_kind, candidate_id, source_icon_id
                 FROM ai_icon_root_creations WHERE icon_id = ?1",
                [&icon_clone.id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(
            clone_root,
            (
                "clone".into(),
                candidate_id.clone(),
                Some(source_icon_id.clone())
            )
        );
        let icon_clone_review = crate::db::repositories::ai::get_ai_review_state(
            &fixture.connection,
            &fixture.collection_id,
            &icon_clone.id,
        )
        .unwrap();
        assert_eq!(icon_clone_review.candidates.len(), 1);
        assert_eq!(icon_clone_review.candidates[0].id, candidate_id);
        assert!(!icon_clone_review.candidates[0].is_stale);

        let collection_clone = crate::db::repositories::collections::duplicate_collection(
            &mut fixture.connection,
            &fixture.paths,
            &fixture.collection_id,
        )
        .unwrap();
        let collection_icons =
            crate::db::repositories::icons::list_icons(&fixture.connection, &collection_clone.id)
                .unwrap();
        assert_eq!(collection_icons.len(), 2);
        for cloned_icon in collection_icons {
            let (kind, owned_candidate, cloned_from): (String, String, Option<String>) = fixture
                .connection
                .query_row(
                    "SELECT creation_kind, candidate_id, source_icon_id
                     FROM ai_icon_root_creations WHERE icon_id = ?1",
                    [&cloned_icon.id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .unwrap();
            assert_eq!(kind, "clone");
            assert_eq!(owned_candidate, candidate_id);
            assert!(cloned_from.is_some());
            let review = crate::db::repositories::ai::get_ai_review_state(
                &fixture.connection,
                &collection_clone.id,
                &cloned_icon.id,
            )
            .unwrap();
            assert_eq!(review.candidates.len(), 1);
            assert_eq!(review.candidates[0].id, candidate_id);
            assert!(!review.candidates[0].is_stale);
        }
        let violations: i64 = fixture
            .connection
            .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(violations, 0);
        fixture.cleanup();
    }
    #[test]
    fn source_free_review_state_survives_database_reopen() {
        let paths = temp_paths();
        let root = paths.root.clone();
        let database_path = root.join("grid-restart.sqlite3");
        let (request_id, collection_id, candidate_id) = {
            let mut connection = Connection::open(&database_path).unwrap();
            connection
                .pragma_update(None, "foreign_keys", "ON")
                .unwrap();
            migrations::run(&mut connection).unwrap();
            let collection =
                create_collection(&mut connection, Some("AI grid restart test".to_string()))
                    .unwrap();
            let prepared = prepare_ai_generation(
                &mut connection,
                &collection.id,
                PrepareAiGenerationRequest {
                    target_names: vec!["재시작 유지".into()],
                    layout: AiGridLayout {
                        canvas_width: 24,
                        canvas_height: 24,
                        rows: 1,
                        columns: 1,
                        cell_size: 24,
                        gap_x: 0,
                        gap_y: 0,
                        border_left: 0,
                        border_top: 0,
                        border_right: 0,
                        border_bottom: 0,
                    },
                    payload_input_signature: "restart-single".into(),
                    retry_of_request_id: None,
                },
            )
            .unwrap();
            mark_ai_grid_awaiting_result(&connection, &prepared.request_id).unwrap();
            record_ai_grid_output_artifact(
                &mut connection,
                &paths,
                &prepared.request_id,
                ImportImageFilePayload {
                    original_filename: "restart-result.png".into(),
                    bytes: solid_png(24, 24, [200, 100, 40, 255]),
                },
                r#"{"schema":"pmtcon-ai-grid-v1","kind":"restart-fixture"}"#,
            )
            .unwrap();
            let committed = commit_ai_grid_candidates(
                &mut connection,
                &paths,
                &prepared.request_id,
                vec![ReviewedGridDecision {
                    result_cell_index: 0,
                    target_item_index: 0,
                    include: true,
                    crop: Some(AiGridRect {
                        x: 0,
                        y: 0,
                        width: 24,
                        height: 24,
                    }),
                }],
            )
            .unwrap();
            (
                prepared.request_id,
                collection.id,
                committed.candidate_ids[0].clone(),
            )
        };

        {
            let mut reopened = Connection::open(&database_path).unwrap();
            reopened.pragma_update(None, "foreign_keys", "ON").unwrap();
            migrations::run(&mut reopened).unwrap();
            let state = get_ai_grid_request_state(&reopened, &request_id).unwrap();
            assert_eq!(
                (
                    state.status.as_str(),
                    state.item_count,
                    state.candidate_count
                ),
                ("layout_review_pending", 1, 1)
            );
            let workspace = get_ai_grid_workspace(&reopened, &request_id).unwrap();
            assert_eq!(workspace.layout.rows, 1);
            assert_eq!(workspace.layout.columns, 1);
            assert!(workspace.output_artifact.is_some());
            assert_eq!(
                get_latest_ai_grid_workspace(&reopened, &collection_id)
                    .unwrap()
                    .unwrap()
                    .request_id,
                request_id
            );
            let (stored_candidate, review_status, origin_icon_id, source_path): (
                String,
                String,
                Option<String>,
                String,
            ) = reopened
                .query_row(
                    "SELECT item.output_candidate_id, item.review_status, item.origin_icon_id,
                            source.original_path_in_library
                     FROM ai_request_items item
                     JOIN ai_candidates candidate ON candidate.id = item.output_candidate_id
                     JOIN source_files source ON source.id = candidate.raw_source_file_id
                     WHERE item.request_id = ?1 AND item.item_index = 0",
                    [&request_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .unwrap();
            let icon_count: i64 = reopened
                .query_row(
                    "SELECT COUNT(*) FROM icons WHERE collection_id = ?1",
                    [&collection_id],
                    |row| row.get(0),
                )
                .unwrap();
            let foreign_key_violations: i64 = reopened
                .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(stored_candidate, candidate_id);
            assert_eq!(review_status, "included");
            assert!(origin_icon_id.is_none());
            assert_eq!(icon_count, 0);
            assert_eq!(foreign_key_violations, 0);
            assert!(std::path::Path::new(&source_path).is_file());
        }

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn direct_output_attach_restores_workspace_and_analyzes_managed_artifact() {
        let mut fixture = Fixture::new(0);
        let request = prepare_ai_generation(
            &mut fixture.connection,
            &fixture.collection_id,
            PrepareAiGenerationRequest {
                target_names: vec!["왼쪽".into(), "오른쪽".into()],
                layout: AiGridLayout {
                    canvas_width: 64,
                    canvas_height: 32,
                    rows: 1,
                    columns: 2,
                    cell_size: 32,
                    gap_x: 0,
                    gap_y: 0,
                    border_left: 0,
                    border_top: 0,
                    border_right: 0,
                    border_bottom: 0,
                },
                payload_input_signature: "direct-attach".into(),
                retry_of_request_id: None,
            },
        )
        .unwrap();

        let invalid = record_ai_grid_output_artifact(
            &mut fixture.connection,
            &fixture.paths,
            &request.request_id,
            ImportImageFilePayload {
                original_filename: "broken.png".into(),
                bytes: b"not-an-image".to_vec(),
            },
            r#"{"schema":"pmtcon-ai-grid-v1","kind":"invalid"}"#,
        )
        .unwrap_err();
        assert!(!invalid.code.is_empty());
        let prepared_workspace =
            get_ai_grid_workspace(&fixture.connection, &request.request_id).unwrap();
        assert_eq!(prepared_workspace.status, "prepared");
        assert!(prepared_workspace.output_artifact.is_none());

        record_ai_grid_output_artifact(
            &mut fixture.connection,
            &fixture.paths,
            &request.request_id,
            ImportImageFilePayload {
                original_filename: "direct.png".into(),
                bytes: two_cell_png(),
            },
            r#"{"schema":"pmtcon-ai-grid-v1","kind":"direct"}"#,
        )
        .unwrap();
        let workspace = get_ai_grid_workspace(&fixture.connection, &request.request_id).unwrap();
        assert_eq!(workspace.status, "layout_review_pending");
        assert_eq!(workspace.layout.columns, 2);
        assert!(workspace.input_artifact.is_none());
        assert!(workspace
            .output_artifact
            .as_ref()
            .is_some_and(|artifact| std::path::Path::new(&artifact.file_path).is_file()));
        let latest = get_latest_ai_grid_workspace(&fixture.connection, &fixture.collection_id)
            .unwrap()
            .unwrap();
        assert_eq!(latest.request_id, request.request_id);

        let analysis = analyze_ai_grid_output(
            &fixture.connection,
            &request.request_id,
            SheetGridSettings {
                mode: "rows_columns".into(),
                rows: Some(1),
                columns: Some(2),
                cell_width: Some(32),
                cell_height: Some(32),
                border_left: 0,
                border_top: 0,
                border_right: 0,
                border_bottom: 0,
                gap_x: 0,
                gap_y: 0,
                read_order: "row_major".into(),
                empty_cell_threshold: None,
            },
        )
        .unwrap();
        assert_eq!((analysis.computed_rows, analysis.computed_columns), (1, 2));
        assert_eq!(analysis.cells.len(), 2);
        fixture.cleanup();
    }

    #[test]
    fn source_free_finalize_creates_atomic_roots_without_mutating_existing_icon() {
        let mut fixture = Fixture::new(1);
        fixture
            .connection
            .execute(
                "UPDATE collections SET cover_icon_id = NULL, cover_source_file_id = NULL
                 WHERE id = ?1",
                [&fixture.collection_id],
            )
            .unwrap();
        let original_before: (String, Option<String>, Option<String>, i64) = fixture
            .connection
            .query_row(
                "SELECT icon.source_file_id, icon.current_preview_path,
                        state.active_version_id, state.revision
                 FROM icons icon JOIN icon_ai_state state ON state.icon_id = icon.id
                 WHERE icon.id = ?1",
                [&fixture.icon_ids[0]],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        let request_id = prepare_two_source_free_candidates(&mut fixture);
        let result = commit_ai_generated_icons(
            &mut fixture.connection,
            &fixture.paths,
            &fixture.collection_id,
            &request_id,
            vec![
                FinalizeGeneratedIconInput {
                    item_index: 0,
                    display_name: "새 왼쪽".into(),
                    alt_text: "왼".into(),
                },
                FinalizeGeneratedIconInput {
                    item_index: 1,
                    display_name: "새 오른쪽".into(),
                    alt_text: "오".into(),
                },
            ],
        )
        .unwrap();
        assert_eq!(result.created_icons.len(), 2);
        assert_eq!(result.created_icons[0].readiness, "working");
        assert_eq!(result.created_icons[0].pieces[0].alt_text, "왼");
        assert_eq!(result.created_icons[1].pieces[0].alt_text, "오");
        assert_eq!(
            result
                .created_icons
                .iter()
                .map(|icon| icon.order_index)
                .collect::<Vec<_>>(),
            vec![1, 2]
        );

        let original_after: (String, Option<String>, Option<String>, i64) = fixture
            .connection
            .query_row(
                "SELECT icon.source_file_id, icon.current_preview_path,
                        state.active_version_id, state.revision
                 FROM icons icon JOIN icon_ai_state state ON state.icon_id = icon.id
                 WHERE icon.id = ?1",
                [&fixture.icon_ids[0]],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(original_after, original_before);
        let workspace = get_ai_grid_workspace(&fixture.connection, &request_id).unwrap();
        assert_eq!(workspace.status, "completed");
        assert_eq!(workspace.created_icon_count, 2);
        assert!(workspace
            .items
            .iter()
            .all(|item| item.review_status == "icon_created" && item.created_icon_id.is_some()));
        let provenance_count: i64 = fixture
            .connection
            .query_row(
                "SELECT COUNT(*) FROM ai_icon_root_creations creation
                 JOIN ai_request_items item ON item.id = creation.request_item_id
                 WHERE item.request_id = ?1
                   AND creation.creation_kind = 'source_free'
                   AND creation.source_icon_id IS NULL
                   AND creation.normalization_recipe_hash IS NULL",
                [&request_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(provenance_count, 2);
        let cover: (Option<String>, Option<String>) = fixture
            .connection
            .query_row(
                "SELECT cover_icon_id, cover_source_file_id FROM collections WHERE id = ?1",
                [&fixture.collection_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            cover.0.as_deref(),
            Some(result.created_icons[0].id.as_str())
        );
        assert_eq!(
            cover.1.as_deref(),
            Some(result.created_icons[0].source_file_id.as_str())
        );
        fixture.cleanup();
    }

    #[test]
    fn missing_candidate_file_rolls_back_entire_source_free_finalize_batch() {
        let mut fixture = Fixture::new(0);
        let request_id = prepare_two_source_free_candidates(&mut fixture);
        let missing_path: String = fixture
            .connection
            .query_row(
                "SELECT source.original_path_in_library
                 FROM ai_request_items item
                 JOIN ai_candidates candidate ON candidate.id = item.output_candidate_id
                 JOIN source_files source ON source.id = candidate.raw_source_file_id
                 WHERE item.request_id = ?1 AND item.item_index = 1",
                [&request_id],
                |row| row.get(0),
            )
            .unwrap();
        std::fs::remove_file(&missing_path).unwrap();
        let error = commit_ai_generated_icons(
            &mut fixture.connection,
            &fixture.paths,
            &fixture.collection_id,
            &request_id,
            vec![
                FinalizeGeneratedIconInput {
                    item_index: 0,
                    display_name: "왼쪽".into(),
                    alt_text: String::new(),
                },
                FinalizeGeneratedIconInput {
                    item_index: 1,
                    display_name: "오른쪽".into(),
                    alt_text: String::new(),
                },
            ],
        )
        .unwrap_err();
        assert_eq!(error.code, "ai_source_repair_required");
        let counts: (i64, i64, i64) = fixture
            .connection
            .query_row(
                "SELECT
                   (SELECT COUNT(*) FROM icons WHERE collection_id = ?1),
                   (SELECT COUNT(*) FROM ai_icon_root_creations creation
                    JOIN ai_request_items item ON item.id = creation.request_item_id
                    WHERE item.request_id = ?2),
                   (SELECT COUNT(*) FROM ai_request_items
                    WHERE request_id = ?2 AND review_status = 'included')",
                params![fixture.collection_id, request_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(counts, (0, 0, 2));
        assert_eq!(
            get_ai_grid_request_state(&fixture.connection, &request_id)
                .unwrap()
                .status,
            "layout_review_pending"
        );
        fixture.cleanup();
    }
}
