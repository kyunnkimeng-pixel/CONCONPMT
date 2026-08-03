use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use image::ImageFormat;
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde_json::json;

use crate::db::repositories::ai::{
    load_and_validate_source, resolve_effective_visual_source, source_dto, EffectiveVisualSource,
    VisualSourceRecord,
};
use crate::db::repositories::ai_activation;
use crate::db::repositories::ai_snapshots;
use crate::db::repositories::source_files::{
    commit_prepared_source_file, prepare_source_file_from_bytes, PreparedSourceArtifactSnapshot,
    PreparedSourceFile, SourceFileImportOptions, StoredSourceFile,
};
use crate::error::{AppError, AppResult};
use crate::imaging::ai_normalization::{
    normalize_static_image_to_png, AiNormalizationAlignment, AiNormalizationGeometry,
    AiNormalizationKind, AiNormalizationMode, AiNormalizationOptions, AiNormalizationResizeFilter,
};
use crate::imaging::import_limits::{decode_import_image, read_import_file_bytes};
use crate::models::{
    AiNormalizationCompatibilityDto, AiNormalizationGeometryDto, AiNormalizationOptionsPayload,
    AiNormalizationWarningDto, ImportImageFilePayload,
};
use crate::optimization::cache::hash_text;
use crate::paths::AppPaths;

#[derive(Debug)]
pub(crate) struct PreparedCandidateNormalization {
    pub candidate_id: String,
    pub payload_input_signature: String,
    pub raw_source: VisualSourceRecord,
    pub effective_source: VisualSourceRecord,
    pub normalized_preview_path: PathBuf,
    pub normalization_recipe_json: String,
    pub normalization_recipe_hash: String,
    pub preview_signature: String,
    pub native_recipe_signature: String,
    pub geometry: AiNormalizationGeometry,
    pub normalized_has_alpha: bool,
    pub current_icon_compatibility: AiNormalizationCompatibilityDto,
    pub new_icon_compatibility: AiNormalizationCompatibilityDto,
    pub warnings: Vec<AiNormalizationWarningDto>,
    pub existing_version_id: Option<String>,
    pub is_current_recipe: bool,
    prepared_source: Option<PreparedSourceFile>,
}

#[derive(Debug)]
struct CandidateRecord {
    id: String,
    payload_input_signature: String,
    source: VisualSourceRecord,
    is_direct_origin: bool,
    has_non_base_version: bool,
}

impl PreparedCandidateNormalization {
    pub(crate) fn source_artifact_snapshot(
        &self,
        connection: &Connection,
        paths: &AppPaths,
    ) -> AppResult<Option<PreparedSourceArtifactSnapshot>> {
        self.prepared_source
            .as_ref()
            .map(|prepared| prepared.artifact_snapshot(connection, paths))
            .transpose()
    }

    pub(crate) fn geometry_dto(&self) -> AiNormalizationGeometryDto {
        AiNormalizationGeometryDto {
            kind: self.geometry.kind.as_str().to_string(),
            resized_width: i64::from(self.geometry.resized_width),
            resized_height: i64::from(self.geometry.resized_height),
            crop_x: i64::from(self.geometry.crop_x),
            crop_y: i64::from(self.geometry.crop_y),
            paste_x: i64::from(self.geometry.paste_x),
            paste_y: i64::from(self.geometry.paste_y),
        }
    }

    pub(crate) fn commit_effective_source(
        &self,
        transaction: &Transaction<'_>,
        paths: &AppPaths,
    ) -> AppResult<VisualSourceRecord> {
        let Some(prepared) = self.prepared_source.as_ref() else {
            return Ok(self.raw_source.clone());
        };
        let stored = commit_prepared_source_file(transaction, paths, prepared)?;
        if stored.id != self.effective_source.id
            || stored.sha256 != self.effective_source.sha256
            || stored.width != self.effective_source.width
            || stored.height != self.effective_source.height
        {
            return Err(AppError::new(
                "ai_normalization_source_conflict",
                "AI 후보를 적용하는 동안 정규화 소스가 변경되었습니다. 미리보기를 다시 만들어 주세요.",
            ));
        }
        Ok(visual_source_from_stored(stored))
    }
}

pub(crate) fn prepare_candidate_normalization(
    connection: &Connection,
    paths: &AppPaths,
    collection_id: &str,
    icon_id: &str,
    candidate_id: &str,
    expected_revision: i64,
    payload: &AiNormalizationOptionsPayload,
    working_dir: &Path,
) -> AppResult<PreparedCandidateNormalization> {
    let current = resolve_effective_visual_source(connection, collection_id, icon_id)?;
    ensure_expected_revision(current.activation_revision, expected_revision)?;
    let candidate = load_candidate(connection, collection_id, icon_id, candidate_id)?;
    if candidate.source.is_animated {
        return Err(AppError::new(
            "ai_normalization_animation",
            "현재 AI 후보 규격화는 정적 JPG/PNG만 지원합니다.",
        ));
    }

    let native_recipe_signature = ai_activation::current_recipe_signature(
        connection,
        collection_id,
        icon_id,
        &current.render_source,
        current.activation_revision,
    )?;
    let options = parse_options(payload)?;
    let source_bytes = read_import_file_bytes(Path::new(&candidate.source.path))?;
    let source_format =
        image_format_for_extension(&candidate.source.extension).ok_or_else(|| {
            AppError::new(
                "ai_normalization_format",
                "AI 후보는 JPG 또는 PNG 정적 이미지여야 합니다.",
            )
        })?;
    let source_image = decode_import_image(&source_bytes, source_format)?;
    let target_width = u32::try_from(current.render_source.width).map_err(|_| {
        AppError::new(
            "ai_normalization_dimensions",
            "현재 편집 캔버스의 너비를 확인할 수 없습니다.",
        )
    })?;
    let target_height = u32::try_from(current.render_source.height).map_err(|_| {
        AppError::new(
            "ai_normalization_dimensions",
            "현재 편집 캔버스의 높이를 확인할 수 없습니다.",
        )
    })?;
    let normalized =
        normalize_static_image_to_png(&source_image, target_width, target_height, options)?;
    fs::create_dir_all(working_dir)?;

    let (effective_source, normalized_preview_path, prepared_source, normalized_has_alpha) =
        if normalized.geometry.kind == AiNormalizationKind::Identity {
            (
                candidate.source.clone(),
                PathBuf::from(&candidate.source.path),
                None,
                candidate.source.has_alpha,
            )
        } else {
            let normalized_path = working_dir.join("normalized-candidate.png");
            write_new_file(&normalized_path, &normalized.bytes)?;
            let normalized_file = ImportImageFilePayload {
                original_filename: format!("{}-normalized.png", candidate.id),
                bytes: normalized.bytes,
            };
            let prepared = prepare_source_file_from_bytes(
                &normalized_file,
                SourceFileImportOptions {
                    allow_gif: false,
                    exact_dimensions: Some((
                        current.render_source.width,
                        current.render_source.height,
                    )),
                },
            )?;
            let mut planned = prepared.planned_source_file(paths);
            if let Some(existing_id) = connection
                .query_row(
                    "SELECT id FROM source_files WHERE sha256 = ?1",
                    [planned.sha256.as_str()],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
            {
                planned.id = existing_id;
            }
            let has_alpha = planned.has_alpha.unwrap_or(false);
            let mut effective = visual_source_from_stored(planned);
            effective.path = normalized_path.to_string_lossy().to_string();
            (effective, normalized_path, Some(prepared), has_alpha)
        };

    let normalization_recipe_json = canonical_recipe(
        &candidate.source,
        &effective_source,
        payload,
        &normalized.geometry,
    );
    let normalization_recipe_hash = hash_text(&[normalization_recipe_json.clone()]);
    let preview_signature = preview_signature(
        icon_id,
        &candidate,
        &current,
        &native_recipe_signature,
        &normalization_recipe_hash,
    );
    let current_icon_compatibility =
        current_icon_compatibility(connection, collection_id, icon_id, &candidate, &current)?;
    let new_icon_compatibility = new_icon_compatibility(&candidate, &current);
    let existing_version_id = existing_version_id(
        connection,
        icon_id,
        &candidate.id,
        &current,
        &normalization_recipe_hash,
    )?;
    let is_current_recipe = existing_version_id.is_some()
        && existing_version_id.as_deref() == current.active_version_id.as_deref();
    let warnings = normalization_warnings(
        &candidate.source,
        &current.render_source,
        payload,
        &normalized.geometry,
    );

    Ok(PreparedCandidateNormalization {
        candidate_id: candidate.id,
        payload_input_signature: candidate.payload_input_signature,
        raw_source: candidate.source,
        effective_source,
        normalized_preview_path,
        normalization_recipe_json,
        normalization_recipe_hash,
        preview_signature,
        native_recipe_signature,
        geometry: normalized.geometry,
        normalized_has_alpha,
        current_icon_compatibility,
        new_icon_compatibility,
        warnings,
        existing_version_id,
        is_current_recipe,
        prepared_source,
    })
}

pub(crate) fn ensure_preview_signature(expected: Option<&str>, actual: &str) -> AppResult<()> {
    if let Some(expected) = expected {
        if expected != actual {
            return Err(AppError::new(
                "ai_normalization_preview_stale",
                "후보 또는 크기 맞춤 설정이 바뀌었습니다. 규격화 미리보기를 다시 만들어 주세요.",
            ));
        }
    }
    Ok(())
}

pub(crate) fn ensure_current_icon_compatible(
    compatibility: &AiNormalizationCompatibilityDto,
) -> AppResult<()> {
    if compatibility.allowed {
        return Ok(());
    }
    Err(AppError::new(
        compatibility
            .reason_code
            .as_deref()
            .unwrap_or("ai_candidate_stale"),
        compatibility
            .reason
            .as_deref()
            .unwrap_or("이 AI 후보는 현재 아이콘에 적용할 수 없습니다."),
    ))
}

pub(crate) fn ensure_new_icon_compatible(
    compatibility: &AiNormalizationCompatibilityDto,
) -> AppResult<()> {
    if compatibility.allowed {
        return Ok(());
    }
    Err(AppError::new(
        compatibility
            .reason_code
            .as_deref()
            .unwrap_or("ai_new_icon_candidate_incompatible"),
        compatibility
            .reason
            .as_deref()
            .unwrap_or("이 AI 후보는 새 아이콘으로 추가할 수 없습니다."),
    ))
}

fn load_candidate(
    connection: &Connection,
    collection_id: &str,
    icon_id: &str,
    candidate_id: &str,
) -> AppResult<CandidateRecord> {
    let row = connection
        .query_row(
            "SELECT candidate.raw_source_file_id,
                    request.payload_input_signature,
                    COALESCE(
                      (
                        request_item.id IS NOT NULL
                        AND request_item.origin_icon_id = ?2
                        AND request.origin_collection_id = ?3
                      ) OR (
                        request_item.id IS NULL
                        AND request.origin_icon_id = ?2
                        AND request.origin_collection_id = ?3
                      ),
                      0
                    ) AS is_direct_origin,
                    EXISTS (
                      SELECT 1
                      FROM icon_ai_versions candidate_version
                      WHERE candidate_version.icon_id = ?2
                        AND candidate_version.candidate_id = candidate.id
                        AND candidate_version.input_stage <> 'base_source'
                    ) AS has_non_base_version
             FROM ai_candidates candidate
             JOIN ai_requests request ON request.id = candidate.request_id
             LEFT JOIN ai_request_items request_item
               ON request_item.id = candidate.request_item_id
             WHERE candidate.id = ?1
               AND (
                 (
                   request_item.id IS NOT NULL
                   AND request_item.origin_icon_id = ?2
                   AND request.origin_collection_id = ?3
                 )
                 OR (
                   request_item.id IS NULL
                   AND request.origin_icon_id = ?2
                   AND request.origin_collection_id = ?3
                 )
                 OR EXISTS (
                   SELECT 1
                   FROM icon_ai_versions owned_version
                   WHERE owned_version.icon_id = ?2
                     AND owned_version.candidate_id = candidate.id
                 )
                 OR EXISTS (
                   SELECT 1
                   FROM ai_icon_root_creations owned_root
                   WHERE owned_root.icon_id = ?2
                     AND owned_root.candidate_id = candidate.id
                 )
               )",
            params![candidate_id, icon_id, collection_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)? != 0,
                    row.get::<_, i64>(3)? != 0,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("규격화할 AI 후보를 찾을 수 없습니다."))?;
    Ok(CandidateRecord {
        id: candidate_id.to_string(),
        payload_input_signature: row.1,
        source: load_and_validate_source(connection, &row.0)?,
        is_direct_origin: row.2,
        has_non_base_version: row.3,
    })
}

fn current_icon_compatibility(
    connection: &Connection,
    collection_id: &str,
    icon_id: &str,
    candidate: &CandidateRecord,
    current: &EffectiveVisualSource,
) -> AppResult<AiNormalizationCompatibilityDto> {
    if current.render_source.is_animated {
        return Ok(blocked_compatibility(
            "ai_normalization_animation_target",
            "현재 단계에서는 정적 AI 후보를 GIF 편집 소스에 적용할 수 없습니다.",
        ));
    }
    if candidate.has_non_base_version {
        return Ok(blocked_compatibility(
            "ai_candidate_input_stage",
            "현재는 원본 소스(base_source)에서 만든 AI 후보만 현재 아이콘에 적용할 수 있습니다.",
        ));
    }
    let is_materialized = candidate_is_materialized(connection, icon_id, &candidate.id, current)?;
    if !candidate.is_direct_origin && !is_materialized {
        return Ok(blocked_compatibility(
            "ai_candidate_lineage",
            "이 후보는 복제된 다른 계보에 속하므로 현재 아이콘에는 적용할 수 없습니다.",
        ));
    }
    if !is_materialized {
        if let Some(reason) = ai_activation::candidate_stale_reason(
            connection,
            collection_id,
            icon_id,
            &candidate.id,
            current,
        )? {
            return Ok(blocked_compatibility("ai_candidate_stale", &reason));
        }
    }
    Ok(allowed_compatibility())
}

fn new_icon_compatibility(
    candidate: &CandidateRecord,
    current: &EffectiveVisualSource,
) -> AiNormalizationCompatibilityDto {
    if current.render_source.is_animated {
        return blocked_compatibility(
            "ai_normalization_animation_target",
            "현재 단계에서는 정적 AI 후보로 GIF 편집값을 복제할 수 없습니다.",
        );
    }
    if candidate.has_non_base_version {
        blocked_compatibility(
            "ai_new_icon_input_stage",
            "현재는 원본 소스(base_source)에서 만든 AI 후보만 새 아이콘으로 추가할 수 있습니다.",
        )
    } else {
        allowed_compatibility()
    }
}

fn allowed_compatibility() -> AiNormalizationCompatibilityDto {
    AiNormalizationCompatibilityDto {
        allowed: true,
        reason_code: None,
        reason: None,
    }
}

fn candidate_is_materialized(
    connection: &Connection,
    icon_id: &str,
    candidate_id: &str,
    current: &EffectiveVisualSource,
) -> AppResult<bool> {
    Ok(connection
        .query_row(
            "SELECT 1
             FROM icon_ai_versions
             WHERE icon_id = ?1
               AND candidate_id = ?2
               AND base_original_lineage_id = ?3
               AND base_original_lineage_generation = ?4
               AND input_stage = 'base_source'
             LIMIT 1",
            params![
                icon_id,
                candidate_id,
                current.original_lineage_id,
                current.original_lineage_generation
            ],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

fn blocked_compatibility(code: &str, reason: &str) -> AiNormalizationCompatibilityDto {
    AiNormalizationCompatibilityDto {
        allowed: false,
        reason_code: Some(code.to_string()),
        reason: Some(reason.to_string()),
    }
}

fn parse_options(payload: &AiNormalizationOptionsPayload) -> AppResult<AiNormalizationOptions> {
    let mode = match payload.mode.as_str() {
        "contain_pad" => AiNormalizationMode::ContainPad,
        "cover_crop" => AiNormalizationMode::CoverCrop,
        _ => {
            return Err(AppError::new(
                "ai_normalization_mode",
                "지원하지 않는 AI 후보 크기 맞춤 방식입니다.",
            ))
        }
    };
    let alignment = match payload.alignment.as_str() {
        "top_left" => AiNormalizationAlignment::TopLeft,
        "top" => AiNormalizationAlignment::Top,
        "top_right" => AiNormalizationAlignment::TopRight,
        "left" => AiNormalizationAlignment::Left,
        "center" => AiNormalizationAlignment::Center,
        "right" => AiNormalizationAlignment::Right,
        "bottom_left" => AiNormalizationAlignment::BottomLeft,
        "bottom" => AiNormalizationAlignment::Bottom,
        "bottom_right" => AiNormalizationAlignment::BottomRight,
        _ => {
            return Err(AppError::new(
                "ai_normalization_alignment",
                "지원하지 않는 AI 후보 정렬 위치입니다.",
            ))
        }
    };
    let resize_filter = match payload.resize_filter.as_str() {
        "lanczos3" => AiNormalizationResizeFilter::Lanczos3,
        "nearest" => AiNormalizationResizeFilter::Nearest,
        _ => {
            return Err(AppError::new(
                "ai_normalization_filter",
                "지원하지 않는 AI 후보 크기 조절 필터입니다.",
            ))
        }
    };
    Ok(AiNormalizationOptions {
        mode,
        alignment,
        resize_filter,
        pad_rgba: payload.pad_rgba,
    })
}

fn canonical_recipe(
    raw_source: &VisualSourceRecord,
    effective_source: &VisualSourceRecord,
    options: &AiNormalizationOptionsPayload,
    geometry: &AiNormalizationGeometry,
) -> String {
    let value = if geometry.kind == AiNormalizationKind::Identity {
        json!({
            "schema": "pmtcon-ai-normalization-v1",
            "kind": "identity",
            "rawSourceFileId": raw_source.id,
            "rawSourceSha256": raw_source.sha256,
            "providerNativeWidth": raw_source.width,
            "providerNativeHeight": raw_source.height,
            "targetCanvasWidth": geometry.target_width,
            "targetCanvasHeight": geometry.target_height,
            "output": {
                "reuseRaw": true,
                "format": raw_source.extension,
                "sourceFileId": raw_source.id,
                "sha256": raw_source.sha256
            }
        })
    } else {
        json!({
            "schema": "pmtcon-ai-normalization-v1",
            "kind": geometry.kind.as_str(),
            "rawSourceFileId": raw_source.id,
            "rawSourceSha256": raw_source.sha256,
            "providerNativeWidth": raw_source.width,
            "providerNativeHeight": raw_source.height,
            "targetCanvasWidth": geometry.target_width,
            "targetCanvasHeight": geometry.target_height,
            "mode": options.mode,
            "alignment": options.alignment,
            "resizeFilter": options.resize_filter,
            "padRgba": options.pad_rgba,
            "geometry": {
                "resizedWidth": geometry.resized_width,
                "resizedHeight": geometry.resized_height,
                "cropX": geometry.crop_x,
                "cropY": geometry.crop_y,
                "pasteX": geometry.paste_x,
                "pasteY": geometry.paste_y
            },
            "output": {
                "reuseRaw": false,
                "format": "png",
                "sha256": effective_source.sha256
            }
        })
    };
    ai_snapshots::canonical_value(&value)
}

fn preview_signature(
    icon_id: &str,
    candidate: &CandidateRecord,
    current: &EffectiveVisualSource,
    native_recipe_signature: &str,
    normalization_recipe_hash: &str,
) -> String {
    hash_text(&[
        "pmtcon-ai-normalization-preview-v1".to_string(),
        icon_id.to_string(),
        candidate.id.clone(),
        candidate.source.id.clone(),
        candidate.source.sha256.clone(),
        current.render_source.width.to_string(),
        current.render_source.height.to_string(),
        current.original_lineage_id.clone(),
        current.original_lineage_generation.to_string(),
        current.activation_revision.to_string(),
        native_recipe_signature.to_string(),
        normalization_recipe_hash.to_string(),
    ])
}

fn existing_version_id(
    connection: &Connection,
    icon_id: &str,
    candidate_id: &str,
    current: &EffectiveVisualSource,
    recipe_hash: &str,
) -> AppResult<Option<String>> {
    connection
        .query_row(
            "SELECT id
             FROM icon_ai_versions
             WHERE icon_id = ?1
               AND candidate_id = ?2
               AND base_original_lineage_id = ?3
               AND base_original_lineage_generation = ?4
               AND normalization_recipe_hash = ?5
             ORDER BY created_at DESC, id DESC
             LIMIT 1",
            params![
                icon_id,
                candidate_id,
                current.original_lineage_id,
                current.original_lineage_generation,
                recipe_hash
            ],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(AppError::from)
}

fn normalization_warnings(
    raw: &VisualSourceRecord,
    target: &VisualSourceRecord,
    payload: &AiNormalizationOptionsPayload,
    geometry: &AiNormalizationGeometry,
) -> Vec<AiNormalizationWarningDto> {
    let mut warnings = Vec::new();
    if geometry.kind == AiNormalizationKind::ContainPad
        && (geometry.resized_width != geometry.target_width
            || geometry.resized_height != geometry.target_height)
    {
        warnings.push(AiNormalizationWarningDto {
            code: "contain_padding".to_string(),
            severity: "info".to_string(),
            message: if payload.pad_rgba[3] == 0 {
                "비율을 유지하기 위해 투명 여백이 생깁니다.".to_string()
            } else {
                "비율을 유지하기 위해 선택한 색의 여백이 생깁니다.".to_string()
            },
        });
    }
    if geometry.kind == AiNormalizationKind::CoverCrop
        && (geometry.resized_width != geometry.target_width
            || geometry.resized_height != geometry.target_height)
    {
        warnings.push(AiNormalizationWarningDto {
            code: "cover_crop".to_string(),
            severity: "warning".to_string(),
            message: "캔버스를 빈틈 없이 채우기 위해 가장자리 일부가 잘립니다.".to_string(),
        });
    }
    if geometry.resized_width > geometry.source_width
        || geometry.resized_height > geometry.source_height
    {
        warnings.push(AiNormalizationWarningDto {
            code: "source_upscaled".to_string(),
            severity: "warning".to_string(),
            message: "AI 원본을 확대하므로 결과가 흐리거나 픽셀이 도드라질 수 있습니다."
                .to_string(),
        });
    }
    if !raw.has_alpha {
        warnings.push(AiNormalizationWarningDto {
            code: "opaque_background_preserved".to_string(),
            severity: "warning".to_string(),
            message:
                "AI 원본의 불투명 배경은 자동으로 제거되지 않습니다. 투명 여백과 배경 제거는 서로 다른 기능입니다."
                    .to_string(),
        });
    }
    if target.is_animated {
        warnings.push(AiNormalizationWarningDto {
            code: "animation_not_supported".to_string(),
            severity: "warning".to_string(),
            message: "현재 정적 AI 후보는 애니메이션 편집 소스에 적용할 수 없습니다.".to_string(),
        });
    }
    warnings
}

fn visual_source_from_stored(stored: StoredSourceFile) -> VisualSourceRecord {
    VisualSourceRecord {
        id: stored.id,
        original_filename: Path::new(&stored.original_path_in_library)
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("normalized.png")
            .to_string(),
        path: stored.original_path_in_library,
        extension: stored.original_extension,
        mime_type: stored.mime_type,
        width: stored.width,
        height: stored.height,
        byte_size: stored.byte_size,
        sha256: stored.sha256,
        has_alpha: stored.has_alpha.unwrap_or(false),
        is_animated: stored.is_animated,
        frame_count: stored.frame_count,
        original_loop_mode: stored.original_loop_mode,
        original_loop_count: stored.original_loop_count,
    }
}

fn write_new_file(path: &Path, bytes: &[u8]) -> AppResult<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn image_format_for_extension(extension: &str) -> Option<ImageFormat> {
    match extension {
        "jpg" | "jpeg" => Some(ImageFormat::Jpeg),
        "png" => Some(ImageFormat::Png),
        _ => None,
    }
}

fn ensure_expected_revision(actual: i64, expected: i64) -> AppResult<()> {
    if actual != expected {
        return Err(AppError::new(
            "ai_revision_conflict",
            "AI 적용 상태가 변경되었습니다. 후보를 다시 확인해 주세요.",
        ));
    }
    Ok(())
}

pub(crate) fn raw_source_dto(
    normalization: &PreparedCandidateNormalization,
) -> crate::models::SourceFileDto {
    source_dto(&normalization.raw_source)
}
