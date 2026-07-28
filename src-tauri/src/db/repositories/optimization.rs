use std::fs::{self, File};
use std::io::Read;
use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension, Row};
use sha2::{Digest, Sha256};

use crate::db::repositories::ai_activation;
use crate::error::{AppError, AppResult};
use crate::models::OptimizationCandidateDto;
use crate::paths::AppPaths;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ProcessedAssetVariantRecord {
    pub id: String,
    pub icon_id: String,
    pub piece_id: Option<String>,
    pub profile_id: Option<String>,
    pub source_file_id: Option<String>,
    pub kind: String,
    pub preset: Option<String>,
    pub path: String,
    pub format: String,
    pub width: i64,
    pub height: i64,
    pub byte_size: i64,
    pub frame_count: Option<i64>,
    pub duration_ms: Option<i64>,
    pub loop_mode: Option<String>,
    pub settings_json: String,
    pub source_hash: String,
    pub crop_hash: String,
    pub output_sha256: Option<String>,
    pub profile_hash: String,
    pub settings_hash: String,
    pub is_active_for_export: bool,
}

#[derive(Debug, Clone)]
pub struct NewProcessedAssetVariant {
    pub id: String,
    pub icon_id: String,
    pub piece_id: Option<String>,
    pub profile_id: Option<String>,
    pub source_file_id: Option<String>,
    pub kind: String,
    pub preset: Option<String>,
    pub path: String,
    pub format: String,
    pub width: i64,
    pub height: i64,
    pub byte_size: i64,
    pub frame_count: Option<i64>,
    pub duration_ms: Option<i64>,
    pub loop_mode: Option<String>,
    pub settings_json: String,
    pub source_hash: String,
    pub crop_hash: String,
    pub profile_hash: String,
    pub settings_hash: String,
}

pub fn insert_variant(
    connection: &Connection,
    variant: &NewProcessedAssetVariant,
) -> AppResult<ProcessedAssetVariantRecord> {
    let source_file_id = variant.source_file_id.as_deref().ok_or_else(|| {
        AppError::new(
            "optimization_provenance",
            "새 최적화 후보에는 현재 effective source ID가 필요합니다.",
        )
    })?;
    let source_matches: i64 = connection.query_row(
        "SELECT COUNT(*)
         FROM effective_visual_sources
         WHERE icon_id = ?1
           AND effective_source_file_id = ?2
           AND effective_source_sha256 = ?3",
        params![variant.icon_id, source_file_id, variant.source_hash],
        |row| row.get(0),
    )?;
    if source_matches != 1 {
        return Err(AppError::new(
            "optimization_provenance",
            "최적화 후보의 source ID/hash가 현재 AI 렌더 소스와 일치하지 않습니다.",
        ));
    }

    let output_path = Path::new(&variant.path);
    let metadata = fs::metadata(output_path).map_err(|_| {
        AppError::not_found("최적화 후보 파일을 찾을 수 없습니다. 후보를 다시 생성하세요.")
    })?;
    if i64::try_from(metadata.len()).unwrap_or(i64::MAX) != variant.byte_size {
        return Err(AppError::new(
            "optimization_artifact",
            "최적화 후보 파일 크기가 생성 기록과 일치하지 않습니다.",
        ));
    }
    let analysis = crate::optimization::analyzer::analyze_file(output_path, &variant.format)?;
    if analysis.width != variant.width || analysis.height != variant.height {
        return Err(AppError::new(
            "optimization_artifact",
            "최적화 후보 이미지 크기가 생성 기록과 일치하지 않습니다.",
        ));
    }
    let output_sha256 = sha256_file(output_path)?;

    connection.execute(
        "INSERT INTO processed_asset_variants (
           id,
           icon_id,
           piece_id,
           profile_id,
           source_file_id,
           kind,
           preset,
           path,
           format,
           width,
           height,
           byte_size,
           frame_count,
           duration_ms,
           loop_mode,
           output_sha256,
           settings_json,
           source_hash,
           crop_hash,
           profile_hash,
           settings_hash,
           created_at,
           is_active_for_export
         )
         VALUES (
           ?1,
           ?2,
           ?3,
           ?4,
           ?5,
           ?6,
           ?7,
           ?8,
           ?9,
           ?10,
           ?11,
           ?12,
           ?13,
           ?14,
           ?15,
           ?21,
           ?16,
           ?17,
           ?18,
           ?19,
           ?20,
           strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
           0
         )",
        params![
            variant.id,
            variant.icon_id,
            variant.piece_id,
            variant.profile_id,
            variant.source_file_id,
            variant.kind,
            variant.preset,
            variant.path,
            variant.format,
            variant.width,
            variant.height,
            variant.byte_size,
            variant.frame_count,
            variant.duration_ms,
            variant.loop_mode,
            variant.settings_json,
            variant.source_hash,
            variant.crop_hash,
            variant.profile_hash,
            variant.settings_hash,
            output_sha256,
        ],
    )?;

    get_variant(connection, &variant.id)
}

pub fn get_variant(
    connection: &Connection,
    candidate_id: &str,
) -> AppResult<ProcessedAssetVariantRecord> {
    connection
        .query_row(
            "SELECT
               id,
               icon_id,
               piece_id,
               profile_id,
               source_file_id,
               kind,
               preset,
               path,
               format,
               width,
               height,
               byte_size,
               frame_count,
               duration_ms,
               loop_mode,
               settings_json,
               source_hash,
               output_sha256,
               crop_hash,
               profile_hash,
               settings_hash,
               is_active_for_export
             FROM processed_asset_variants
             WHERE id = ?1",
            params![candidate_id],
            variant_from_row,
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("최적화 후보를 찾을 수 없습니다."))
}

pub fn list_candidates(
    connection: &Connection,
    icon_id: &str,
    profile_id: &str,
    piece_id: Option<&str>,
) -> AppResult<Vec<ProcessedAssetVariantRecord>> {
    let mut statement = connection.prepare(
        "SELECT
           id,
           icon_id,
           piece_id,
           profile_id,
           source_file_id,
           kind,
           preset,
           path,
           format,
           width,
           height,
           byte_size,
           frame_count,
           duration_ms,
           loop_mode,
           settings_json,
           source_hash,
           output_sha256,
           crop_hash,
           profile_hash,
           settings_hash,
           is_active_for_export
         FROM processed_asset_variants
         WHERE icon_id = ?1
           AND profile_id = ?2
           AND (?3 IS NULL OR piece_id = ?3)
           AND kind IN ('baseline_export', 'optimized_gif', 'optimized_png', 'optimized_jpg')
         ORDER BY created_at DESC",
    )?;

    let variants = statement
        .query_map(params![icon_id, profile_id, piece_id], variant_from_row)?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(variants)
}

pub(crate) fn list_active_variants_for_icon(
    connection: &Connection,
    icon_id: &str,
) -> AppResult<Vec<ProcessedAssetVariantRecord>> {
    let mut statement = connection.prepare(
        "SELECT
           id,
           icon_id,
           piece_id,
           profile_id,
           source_file_id,
           kind,
           preset,
           path,
           format,
           width,
           height,
           byte_size,
           frame_count,
           duration_ms,
           loop_mode,
           settings_json,
           source_hash,
           output_sha256,
           crop_hash,
           profile_hash,
           settings_hash,
           is_active_for_export
         FROM processed_asset_variants
         WHERE icon_id = ?1
           AND is_active_for_export = 1
         ORDER BY created_at DESC, id DESC",
    )?;

    let variants = statement
        .query_map(params![icon_id], variant_from_row)?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(variants)
}
pub fn find_active_variant(
    connection: &Connection,
    paths: &AppPaths,
    icon_id: &str,
    profile_id: &str,
    piece_id: &str,
    source_hash: &str,
    crop_hash: &str,
    profile_hash: &str,
    format: &str,
) -> AppResult<Option<ProcessedAssetVariantRecord>> {
    let variant = connection
        .query_row(
            "SELECT
               id,
               icon_id,
               piece_id,
               profile_id,
               source_file_id,
               kind,
               preset,
               path,
               format,
               width,
               height,
               byte_size,
               frame_count,
               duration_ms,
               loop_mode,
               settings_json,
               source_hash,
               output_sha256,
               crop_hash,
               profile_hash,
               settings_hash,
               is_active_for_export
             FROM processed_asset_variants
             WHERE icon_id = ?1
               AND profile_id = ?2
               AND piece_id = ?3
               AND source_hash = ?4
               AND crop_hash = ?5
               AND source_file_id = (
                 SELECT effective_source_file_id FROM effective_visual_sources WHERE icon_id = ?1
               )
               AND source_hash = (SELECT effective_source_sha256 FROM effective_visual_sources WHERE icon_id = ?1)
               AND profile_hash = ?6
               AND format = ?7
               AND is_active_for_export = 1
             ORDER BY created_at DESC
             LIMIT 1",
            params![
                icon_id,
                profile_id,
                piece_id,
                source_hash,
                crop_hash,
                profile_hash,
                format
            ],
            variant_from_row,
        )
        .optional()?;

    let Some(variant) = variant else {
        return Ok(None);
    };
    if validate_variant_artifact(&variant)? {
        return Ok(Some(variant));
    }
    invalidate_variant(connection, paths, &variant)?;
    Ok(None)
}

pub fn set_active_variant(
    connection: &Connection,
    candidate_id: &str,
) -> AppResult<ProcessedAssetVariantRecord> {
    let variant = get_variant(connection, candidate_id)?;
    let piece_id = variant.piece_id.clone().ok_or_else(|| {
        AppError::new(
            "optimization",
            "조각 단위가 없는 후보는 export 활성 후보로 적용할 수 없습니다.",
        )
    })?;
    let profile_id = variant.profile_id.clone().ok_or_else(|| {
        AppError::new(
            "optimization",
            "프로필 정보가 없는 후보는 export 활성 후보로 적용할 수 없습니다.",
        )
    })?;

    if !variant_provenance_matches_current(connection, &variant)? {
        return Err(AppError::new(
            "optimization_provenance",
            "현재 AI 렌더 소스와 다른 후보는 활성화할 수 없습니다. 후보를 다시 생성하세요.",
        ));
    }
    if !validate_variant_artifact(&variant)? {
        return Err(AppError::new(
            "optimization_artifact",
            "최적화 후보 파일의 크기·형식·digest가 기록과 일치하지 않습니다.",
        ));
    }

    let transaction = connection.unchecked_transaction()?;
    transaction.execute(
        "UPDATE processed_asset_variants
         SET is_active_for_export = 0
         WHERE icon_id = ?1
           AND profile_id = ?2
           AND piece_id = ?3",
        params![variant.icon_id, profile_id, piece_id],
    )?;
    transaction.execute(
        "UPDATE processed_asset_variants
         SET is_active_for_export = 1
         WHERE id = ?1",
        params![candidate_id],
    )?;
    transaction.commit()?;

    get_variant(connection, candidate_id)
}

pub fn promote_variant_to_preview(
    connection: &Connection,
    candidate_id: &str,
) -> AppResult<ProcessedAssetVariantRecord> {
    let variant = get_variant(connection, candidate_id)?;
    let piece_id = variant.piece_id.as_deref().ok_or_else(|| {
        AppError::new(
            "optimization",
            "조각 정보가 없는 후보는 미리보기로 적용할 수 없습니다.",
        )
    })?;

    if !variant_provenance_matches_current(connection, &variant)? {
        return Err(AppError::new(
            "optimization_provenance",
            "현재 AI 렌더 소스와 다른 후보는 미리보기로 적용할 수 없습니다.",
        ));
    }
    if !validate_variant_artifact(&variant)? {
        return Err(AppError::new(
            "optimization_artifact",
            "적용할 후보 파일의 크기·형식·digest가 기록과 일치하지 않습니다.",
        ));
    }

    let piece_count: i64 = connection.query_row(
        "SELECT COUNT(*)
         FROM icon_pieces
         WHERE icon_id = ?1",
        params![variant.icon_id.as_str()],
        |row| row.get(0),
    )?;

    connection.execute(
        "UPDATE icon_pieces
         SET generated_preview_path = ?1,
             export_status = 'ready',
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?2
           AND icon_id = ?3",
        params![variant.path.as_str(), piece_id, variant.icon_id.as_str()],
    )?;

    if piece_count == 1 {
        connection.execute(
            "UPDATE icons
             SET current_preview_path = ?1,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?2
               AND deleted_at IS NULL",
            params![variant.path.as_str(), variant.icon_id.as_str()],
        )?;
    } else {
        connection.execute(
            "UPDATE icons
             SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?1
               AND deleted_at IS NULL",
            params![variant.icon_id.as_str()],
        )?;
    }

    get_variant(connection, candidate_id)
}

pub fn clear_active_variant(
    connection: &Connection,
    icon_id: &str,
    profile_id: &str,
    piece_id: Option<&str>,
) -> AppResult<i64> {
    let changed = connection.execute(
        "UPDATE processed_asset_variants
         SET is_active_for_export = 0
         WHERE icon_id = ?1
           AND profile_id = ?2
           AND (?3 IS NULL OR piece_id = ?3)
           AND is_active_for_export = 1",
        params![icon_id, profile_id, piece_id],
    )?;

    Ok(i64::try_from(changed).unwrap_or(i64::MAX))
}

pub fn to_candidate_dto(
    variant: &ProcessedAssetVariantRecord,
    target_max_bytes: i64,
    original_frame_count: Option<i64>,
    original_duration_ms: Option<i64>,
) -> OptimizationCandidateDto {
    let preset = variant
        .preset
        .clone()
        .unwrap_or_else(|| "custom".to_string());
    let quality_impact = quality_impact_for_preset(&preset).to_string();

    OptimizationCandidateDto {
        id: variant.id.clone(),
        icon_id: variant.icon_id.clone(),
        profile_id: variant.profile_id.clone().unwrap_or_default(),
        piece_id: variant.piece_id.clone().unwrap_or_default(),
        preset: preset.clone(),
        path: variant.path.clone(),
        preview_url: variant.path.clone(),
        format: variant.format.clone(),
        measured_byte_size: variant.byte_size,
        target_max_bytes,
        passes: variant.byte_size <= target_max_bytes,
        width: variant.width,
        height: variant.height,
        frame_count: variant.frame_count,
        original_frame_count,
        duration_ms: variant.duration_ms,
        original_duration_ms,
        loop_mode: variant.loop_mode.clone(),
        color_limit: settings_number(&variant.settings_json, "colorLimit"),
        fps_limit: settings_number(&variant.settings_json, "fpsLimit"),
        quality: settings_number(&variant.settings_json, "quality"),
        quality_impact,
        settings_json: variant.settings_json.clone(),
        summary: candidate_summary(variant, target_max_bytes),
        is_active_for_export: variant.is_active_for_export,
    }
}

fn variant_from_row(row: &Row<'_>) -> rusqlite::Result<ProcessedAssetVariantRecord> {
    let is_active_for_export: i64 = row.get("is_active_for_export")?;
    Ok(ProcessedAssetVariantRecord {
        id: row.get("id")?,
        icon_id: row.get("icon_id")?,
        piece_id: row.get("piece_id")?,
        profile_id: row.get("profile_id")?,
        source_file_id: row.get("source_file_id")?,
        kind: row.get("kind")?,
        preset: row.get("preset")?,
        path: row.get("path")?,
        format: row.get("format")?,
        width: row.get("width")?,
        height: row.get("height")?,
        byte_size: row.get("byte_size")?,
        frame_count: row.get("frame_count")?,
        duration_ms: row.get("duration_ms")?,
        loop_mode: row.get("loop_mode")?,
        settings_json: row.get("settings_json")?,
        source_hash: row.get("source_hash")?,
        output_sha256: row.get("output_sha256")?,
        crop_hash: row.get("crop_hash")?,
        profile_hash: row.get("profile_hash")?,
        settings_hash: row.get("settings_hash")?,
        is_active_for_export: is_active_for_export != 0,
    })
}

pub fn reconcile_legacy_variants(connection: &Connection, paths: &AppPaths) -> AppResult<()> {
    let ids = {
        let mut statement = connection
            .prepare("SELECT id FROM processed_asset_variants ORDER BY created_at ASC, id ASC")?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        rows
    };

    for id in ids {
        let variant = get_variant(connection, &id)?;
        let source_is_valid = match variant.source_file_id.as_deref() {
            Some(source_file_id) => {
                connection.query_row(
                    "SELECT EXISTS(
                   SELECT 1 FROM source_files WHERE id = ?1 AND sha256 = ?2
                 )",
                    params![source_file_id, variant.source_hash],
                    |row| row.get::<_, i64>(0),
                )? == 1
            }
            None => false,
        };
        let current_is_valid = !variant.is_active_for_export
            || variant_provenance_matches_current(connection, &variant)?;
        let artifact_sha = if source_is_valid && current_is_valid {
            inspect_variant_artifact(&variant)?
        } else {
            None
        };
        let artifact_is_valid = artifact_sha.as_deref().is_some_and(|actual| {
            variant
                .output_sha256
                .as_deref()
                .is_none_or(|stored| stored == actual)
        });

        if artifact_is_valid {
            if variant.output_sha256.is_none() {
                connection.execute(
                    "UPDATE processed_asset_variants SET output_sha256 = ?1 WHERE id = ?2",
                    params![artifact_sha, variant.id],
                )?;
            }
        } else {
            invalidate_variant(connection, paths, &variant)?;
        }
    }

    Ok(())
}

fn variant_provenance_matches_current(
    connection: &Connection,
    variant: &ProcessedAssetVariantRecord,
) -> AppResult<bool> {
    let Some(source_file_id) = variant.source_file_id.as_deref() else {
        return Ok(false);
    };
    let count: i64 = connection.query_row(
        "SELECT COUNT(*)
         FROM effective_visual_sources
         WHERE icon_id = ?1
           AND effective_source_file_id = ?2
           AND effective_source_sha256 = ?3",
        params![variant.icon_id, source_file_id, variant.source_hash],
        |row| row.get(0),
    )?;
    Ok(count == 1)
}

fn validate_variant_artifact(variant: &ProcessedAssetVariantRecord) -> AppResult<bool> {
    let Some(expected_sha256) = variant.output_sha256.as_deref() else {
        return Ok(false);
    };
    Ok(inspect_variant_artifact(variant)?
        .as_deref()
        .is_some_and(|actual| actual == expected_sha256))
}

fn inspect_variant_artifact(variant: &ProcessedAssetVariantRecord) -> AppResult<Option<String>> {
    let path = Path::new(&variant.path);
    let Ok(metadata) = fs::metadata(path) else {
        return Ok(None);
    };
    if i64::try_from(metadata.len()).unwrap_or(i64::MAX) != variant.byte_size {
        return Ok(None);
    }
    let Ok(analysis) = crate::optimization::analyzer::analyze_file(path, &variant.format) else {
        return Ok(None);
    };
    if analysis.width != variant.width || analysis.height != variant.height {
        return Ok(None);
    }
    Ok(Some(sha256_file(path)?))
}

pub fn reconcile_missing_effective_previews(
    connection: &Connection,
    paths: &AppPaths,
) -> AppResult<()> {
    let icons = {
        let mut statement = connection.prepare(
            "SELECT i.id, i.collection_id, i.current_preview_path
             FROM icons i
             JOIN collections c ON c.id = i.collection_id
             JOIN icon_ai_state state ON state.icon_id = i.id
             WHERE i.deleted_at IS NULL
               AND c.deleted_at IS NULL
               AND state.active_version_id IS NOT NULL
             ORDER BY i.created_at ASC, i.id ASC",
        )?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows
    };

    for (icon_id, collection_id, current_preview_path) in icons {
        let current_is_missing = current_preview_path
            .as_deref()
            .is_none_or(|path| !Path::new(path).is_file());
        let piece_paths = {
            let mut statement = connection.prepare(
                "SELECT generated_preview_path
                 FROM icon_pieces
                 WHERE icon_id = ?1
                 ORDER BY piece_index ASC",
            )?;
            let rows = statement
                .query_map([icon_id.as_str()], |row| row.get::<_, Option<String>>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            rows
        };
        let piece_is_missing = piece_paths.is_empty()
            || piece_paths.iter().any(|path| {
                path.as_deref()
                    .is_none_or(|path| !Path::new(path).is_file())
            });
        if current_is_missing || piece_is_missing {
            ai_activation::repair_effective_preview(
                connection,
                paths,
                &collection_id,
                &icon_id,
                None,
            )?;
        }
    }
    Ok(())
}

fn invalidate_variant(
    connection: &Connection,
    paths: &AppPaths,
    variant: &ProcessedAssetVariantRecord,
) -> AppResult<()> {
    let preview_is_referenced: i64 = connection.query_row(
        "SELECT EXISTS(
           SELECT 1 FROM icons WHERE id = ?1 AND current_preview_path = ?2
           UNION ALL
           SELECT 1 FROM icon_pieces WHERE icon_id = ?1 AND generated_preview_path = ?2
         )",
        params![variant.icon_id, variant.path],
        |row| row.get(0),
    )?;
    if preview_is_referenced == 1 {
        let collection_id = connection
            .query_row(
                "SELECT collection_id
                 FROM icons
                 WHERE id = ?1
                   AND deleted_at IS NULL",
                [variant.icon_id.as_str()],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        if let Some(collection_id) = collection_id {
            ai_activation::repair_effective_preview(
                connection,
                paths,
                &collection_id,
                &variant.icon_id,
                Some(&variant.id),
            )?;
            return Ok(());
        }
    }

    connection.execute(
        "UPDATE processed_asset_variants
         SET is_active_for_export = 0
         WHERE id = ?1",
        [variant.id.as_str()],
    )?;
    Ok(())
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
    Ok(format!("{:x}", hasher.finalize()))
}

fn settings_number(settings_json: &str, key: &str) -> Option<i64> {
    serde_json::from_str::<serde_json::Value>(settings_json)
        .ok()
        .and_then(|value| value.get(key).and_then(serde_json::Value::as_i64))
}

fn quality_impact_for_preset(preset: &str) -> &'static str {
    match preset {
        "quality" => "낮음",
        "balanced" => "보통",
        "smallest" => "큼",
        _ => "보통",
    }
}

fn candidate_summary(variant: &ProcessedAssetVariantRecord, target_max_bytes: i64) -> String {
    let preset = variant.preset.as_deref().unwrap_or("custom");
    let readiness = if variant.byte_size <= target_max_bytes {
        "제한 이하"
    } else {
        "제한 초과"
    };
    format!(
        "{preset} 후보: {} / {} ({readiness})",
        format_bytes(variant.byte_size),
        format_bytes(target_max_bytes)
    )
}

fn format_bytes(bytes: i64) -> String {
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    if bytes < 1024 * 1024 {
        return format!("{:.1} KB", bytes as f64 / 1024.0);
    }
    format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
}
