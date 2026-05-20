use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::error::{AppError, AppResult};
use crate::models::OptimizationCandidateDto;

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

pub fn find_active_variant(
    connection: &Connection,
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

    Ok(variant.filter(|record| Path::new(&record.path).is_file()))
}

pub fn set_active_variant(
    connection: &Connection,
    candidate_id: &str,
) -> AppResult<ProcessedAssetVariantRecord> {
    let variant = get_variant(connection, candidate_id)?;
    let piece_id = variant.piece_id.as_deref().ok_or_else(|| {
        AppError::new(
            "optimization",
            "조각 단위가 없는 후보는 export 활성 후보로 적용할 수 없습니다.",
        )
    })?;
    let profile_id = variant.profile_id.as_deref().ok_or_else(|| {
        AppError::new(
            "optimization",
            "프로필 정보가 없는 후보는 export 활성 후보로 적용할 수 없습니다.",
        )
    })?;

    if !Path::new(&variant.path).is_file() {
        return Err(AppError::not_found(
            "최적화 후보 파일을 찾을 수 없습니다. 후보를 다시 생성하세요.",
        ));
    }

    connection.execute(
        "UPDATE processed_asset_variants
         SET is_active_for_export = 0
         WHERE icon_id = ?1
           AND profile_id = ?2
           AND piece_id = ?3",
        params![variant.icon_id, profile_id, piece_id],
    )?;
    connection.execute(
        "UPDATE processed_asset_variants
         SET is_active_for_export = 1
         WHERE id = ?1",
        params![candidate_id],
    )?;

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

    if !Path::new(&variant.path).is_file() {
        return Err(AppError::not_found(
            "적용할 후보 파일을 찾을 수 없습니다. 후보를 다시 생성하세요.",
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
        crop_hash: row.get("crop_hash")?,
        profile_hash: row.get("profile_hash")?,
        settings_hash: row.get("settings_hash")?,
        is_active_for_export: is_active_for_export != 0,
    })
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
