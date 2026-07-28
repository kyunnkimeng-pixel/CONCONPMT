use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};

use crate::db::repositories::ai as ai_repository;
use crate::db::repositories::effects as effect_repository;
use crate::db::repositories::icons as icon_repository;
use crate::db::repositories::motion as motion_repository;
use crate::error::{AppError, AppResult};
use crate::ids::create_id;
use crate::imaging::effects::{validate_effect_recipe, EffectRecipe, EffectStep};
use crate::imaging::geometry::piece_roles;
use crate::imaging::import_limits::{validate_crop_rect, validate_import_dimensions};
use crate::imaging::motion::MotionRecipe;
use crate::imaging::preview::{
    generate_icon_preview, generate_icon_preview_in_directory, CropRect, GeneratePreviewRequest,
    GeneratedPreview,
};
use crate::imaging::text_overlay::{text_overlay_from_fields, TextOverlayRenderSpec};
use crate::imaging::transform::{source_viewport_geometry, ImageTransform};
use crate::models::{
    ApplyIconCropPayload, CropSettingsDto, EffectPreviewDto, IconDto, IconEditorStateDto,
    PreviewIconEffectsPayload, SourceFileDto, TextOverlayDto, UpdateIconEffectsPayload,
    UpdateIconTextOverlayPayload,
};
use crate::optimization::cache::hash_text;
use crate::paths::AppPaths;

const EFFECT_PREVIEW_DIRECTORY: &str = "effect-previews";
const EFFECT_PREVIEW_IN_PROGRESS_MARKER: &str = ".in-progress";
const EFFECT_PREVIEW_COMPLETE_MARKER: &str = ".complete";
const MAX_COMPLETED_EFFECT_PREVIEWS_PER_ICON: usize = 8;

pub fn get_icon_editor_state(
    connection: &Connection,
    collection_id: &str,
    icon_id: &str,
) -> AppResult<IconEditorStateDto> {
    let icon = icon_repository::get_icon(connection, collection_id, icon_id)?;
    let visual_source =
        ai_repository::resolve_effective_visual_source(connection, collection_id, icon_id)?;
    let source = ai_repository::source_dto(&visual_source.render_source);
    let crop = crop_settings_for_icon(connection, icon_id)?;
    let text_overlay = text_overlay_for_icon(connection, collection_id, icon_id)?;
    let effects = effect_repository::effect_recipe_for_icon(connection, collection_id, icon_id)?;
    let motion = motion_repository::motion_recipe_for_icon(connection, collection_id, icon_id)?;

    Ok(IconEditorStateDto {
        icon,
        source,
        visual_source: ai_repository::effective_source_dto(&visual_source),
        crop,
        text_overlay,
        effect_recipe: effects.recipe,
        effect_revision: effects.revision,
        motion_recipe: motion.recipe,
        motion_revision: motion.revision,
    })
}

pub fn apply_icon_crop(
    connection: &mut Connection,
    paths: &AppPaths,
    collection_id: &str,
    payload: ApplyIconCropPayload,
) -> AppResult<IconDto> {
    validate_apply_payload(&payload)?;
    ai_repository::resolve_effective_visual_source(connection, collection_id, &payload.icon_id)?;
    let apply_record = apply_record_for_icon(connection, collection_id, &payload.icon_id)?;
    let transform = payload_transform(&payload)?;
    let source_geometry = source_viewport_geometry(
        &payload.shape,
        payload.cell_width,
        payload.cell_height,
        transform,
    )?;
    let source_path = PathBuf::from(&apply_record.original_path_in_library);
    let text_overlay =
        text_overlay_render_spec_for_icon(connection, collection_id, &payload.icon_id)?;
    let effects =
        effect_repository::effect_recipe_for_icon(connection, collection_id, &payload.icon_id)?;
    let motion =
        motion_repository::motion_recipe_for_icon(connection, collection_id, &payload.icon_id)?;

    let preview = generate_icon_preview(
        paths,
        GeneratePreviewRequest {
            collection_id,
            icon_id: &payload.icon_id,
            source_path: &source_path,
            source_extension: &apply_record.original_extension,
            shape: &payload.shape,
            crop: CropRect {
                x: payload.crop_x,
                y: payload.crop_y,
                width: payload.crop_w,
                height: payload.crop_h,
            },
            cell_width: payload.cell_width,
            cell_height: payload.cell_height,
            transform,
            gif_loop_mode: &payload.gif_loop_mode,
            gif_loop_count: payload.gif_loop_count,
            source_gif_loop_mode: Some(&apply_record.original_loop_mode),
            source_gif_loop_count: apply_record.original_loop_count,
            text_overlay,
            effects: effects.recipe,
            motion: motion.recipe,
        },
    )?;
    validate_generated_piece_outputs(&preview, apply_record.max_bytes)?;

    let transaction = connection.transaction()?;
    ensure_icon_still_editable(&transaction, collection_id, &payload.icon_id)?;
    update_icon_record(
        &transaction,
        collection_id,
        &payload,
        &apply_record,
        transform,
        preview.current_preview_path.to_string_lossy().as_ref(),
    )?;
    upsert_crop_settings(
        &transaction,
        &payload,
        apply_record.source_width,
        apply_record.source_height,
        source_geometry.viewport.width,
        source_geometry.viewport.height,
    )?;
    reconcile_icon_pieces(&transaction, collection_id, &payload, &preview.piece_paths)?;
    transaction.commit()?;

    icon_repository::get_icon(connection, collection_id, &payload.icon_id)
}

pub fn update_icon_text_overlay(
    connection: &mut Connection,
    paths: &AppPaths,
    collection_id: &str,
    payload: UpdateIconTextOverlayPayload,
) -> AppResult<IconEditorStateDto> {
    validate_text_overlay_payload(&payload)?;
    ai_repository::resolve_effective_visual_source(connection, collection_id, &payload.icon_id)?;
    let preview_record = text_overlay_preview_record(connection, collection_id, &payload.icon_id)?;
    let source_path = PathBuf::from(&preview_record.original_path_in_library);
    let text_overlay = text_overlay_render_spec_from_payload(&payload)?;
    let effects =
        effect_repository::effect_recipe_for_icon(connection, collection_id, &payload.icon_id)?;
    let motion =
        motion_repository::motion_recipe_for_icon(connection, collection_id, &payload.icon_id)?;
    let transform = ImageTransform::new(
        preview_record.transform_quarter_turns,
        preview_record.transform_flip_horizontal,
        preview_record.transform_flip_vertical,
    )?;

    let preview = generate_icon_preview(
        paths,
        GeneratePreviewRequest {
            collection_id,
            icon_id: &payload.icon_id,
            source_path: &source_path,
            source_extension: &preview_record.original_extension,
            shape: &preview_record.shape,
            crop: CropRect {
                x: preview_record.crop_x,
                y: preview_record.crop_y,
                width: preview_record.crop_w,
                height: preview_record.crop_h,
            },
            cell_width: preview_record.cell_width,
            cell_height: preview_record.cell_height,
            transform,
            gif_loop_mode: &preview_record.gif_loop_mode,
            gif_loop_count: preview_record.gif_loop_count,
            source_gif_loop_mode: Some(&preview_record.original_loop_mode),
            source_gif_loop_count: preview_record.original_loop_count,
            text_overlay: text_overlay.clone(),
            effects: effects.recipe,
            motion: motion.recipe,
        },
    )?;
    validate_generated_piece_outputs(&preview, preview_record.max_bytes)?;

    let transaction = connection.transaction()?;
    ensure_icon_still_editable(&transaction, collection_id, &payload.icon_id)?;
    update_text_overlay_record(
        &transaction,
        collection_id,
        &payload,
        preview.current_preview_path.to_string_lossy().as_ref(),
    )?;
    reconcile_icon_pieces(
        &transaction,
        collection_id,
        &record_as_crop(&preview_record, &payload.icon_id),
        &preview.piece_paths,
    )?;
    transaction.commit()?;

    get_icon_editor_state(connection, collection_id, &payload.icon_id)
}

pub fn preview_icon_effects(
    connection: &Connection,
    paths: &AppPaths,
    collection_id: &str,
    payload: PreviewIconEffectsPayload,
) -> AppResult<EffectPreviewDto> {
    validate_effect_recipe(&payload.recipe)?;
    ai_repository::resolve_effective_visual_source(connection, collection_id, &payload.icon_id)?;
    let record = text_overlay_preview_record(connection, collection_id, &payload.icon_id)?;
    let text_overlay =
        text_overlay_render_spec_for_icon(connection, collection_id, &payload.icon_id)?;
    let motion =
        motion_repository::motion_recipe_for_icon(connection, collection_id, &payload.icon_id)?;
    let signature = effect_preview_signature(
        &record,
        text_overlay.as_ref(),
        &payload.recipe,
        &motion.recipe,
    )?;
    let mut preview_request =
        OwnedEffectPreviewRequest::create(paths, &payload.icon_id, &signature)?;
    let started = Instant::now();
    let generated = render_effect_preview(
        preview_request.directory(),
        collection_id,
        &payload.icon_id,
        &record,
        text_overlay,
        payload.recipe.clone(),
        motion.recipe,
    )?;
    let preview = effect_preview_dto(
        generated,
        &record,
        &payload.recipe,
        signature,
        started.elapsed().as_millis(),
    )?;
    preview_request.mark_completed()?;
    prune_completed_effect_preview_requests(
        preview_request.icon_root(),
        preview_request.directory(),
    );
    Ok(preview)
}

pub fn update_icon_effects(
    connection: &mut Connection,
    paths: &AppPaths,
    collection_id: &str,
    payload: UpdateIconEffectsPayload,
) -> AppResult<IconEditorStateDto> {
    validate_effect_recipe(&payload.recipe)?;
    ai_repository::resolve_effective_visual_source(connection, collection_id, &payload.icon_id)?;
    if payload.expected_revision < 0 {
        return Err(AppError::new(
            "validation",
            "효과 recipe revision이 올바르지 않습니다.",
        ));
    }
    let record = text_overlay_preview_record(connection, collection_id, &payload.icon_id)?;
    let text_overlay =
        text_overlay_render_spec_for_icon(connection, collection_id, &payload.icon_id)?;
    let motion =
        motion_repository::motion_recipe_for_icon(connection, collection_id, &payload.icon_id)?;
    let signature = effect_preview_signature(
        &record,
        text_overlay.as_ref(),
        &payload.recipe,
        &motion.recipe,
    )?;
    let _expected_next_revision = payload
        .expected_revision
        .checked_add(1)
        .ok_or_else(|| AppError::new("validation", "효과 revision이 너무 큽니다."))?;
    let effect_root = paths
        .collection_previews_dir
        .join(collection_id)
        .join(&payload.icon_id)
        .join("effects");
    let mut artifact = OwnedEffectArtifact::create(&effect_root, &signature)?;
    let mut generated = render_effect_preview(
        artifact.staging_dir(),
        collection_id,
        &payload.icon_id,
        &record,
        text_overlay,
        payload.recipe.clone(),
        motion.recipe,
    )?;
    validate_generated_piece_formats(&generated)?;

    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let current_record =
        text_overlay_preview_record(&transaction, collection_id, &payload.icon_id)?;
    let current_text_overlay =
        text_overlay_render_spec_for_icon(&transaction, collection_id, &payload.icon_id)?;
    let current_motion =
        motion_repository::motion_recipe_for_icon(&transaction, collection_id, &payload.icon_id)?;
    let current_signature = effect_preview_signature(
        &current_record,
        current_text_overlay.as_ref(),
        &payload.recipe,
        &current_motion.recipe,
    )?;
    if current_signature != signature {
        return Err(AppError::new(
            "conflict",
            "편집 기준이 변경되었습니다. 최신 상태를 다시 불러온 뒤 시도해 주세요.",
        ));
    }

    let next_revision = effect_repository::upsert_effect_recipe(
        &transaction,
        collection_id,
        &payload.icon_id,
        payload.expected_revision,
        &payload.recipe,
    )?;
    let final_dir = artifact.promote(next_revision)?;
    rebase_generated_preview(&mut generated, artifact.staging_dir(), &final_dir)?;

    let update_result = (|| -> AppResult<()> {
        ensure_icon_still_editable(&transaction, collection_id, &payload.icon_id)?;
        update_effect_preview_record(
            &transaction,
            collection_id,
            &payload.icon_id,
            generated.current_preview_path.to_string_lossy().as_ref(),
        )?;
        reconcile_icon_pieces(
            &transaction,
            collection_id,
            &record_as_crop(&record, &payload.icon_id),
            &generated.piece_paths,
        )?;
        transaction.commit()?;
        Ok(())
    })();

    if let Err(error) = update_result {
        return Err(error);
    }
    artifact.keep_final();
    cleanup_previous_effect_preview(
        connection,
        &effect_root,
        record.current_preview_path.as_deref(),
        &final_dir,
    );

    get_icon_editor_state(connection, collection_id, &payload.icon_id)
}

#[derive(Debug)]
struct ApplyRecord {
    source_width: i64,
    source_height: i64,
    original_path_in_library: String,
    original_extension: String,
    original_loop_mode: String,
    original_loop_count: Option<i64>,
    default_cell_width: i64,
    default_cell_height: i64,
    max_bytes: i64,
}

#[derive(Debug)]
struct TextOverlayPreviewRecord {
    original_path_in_library: String,
    original_extension: String,
    source_hash: String,
    original_loop_mode: String,
    original_loop_count: Option<i64>,
    current_preview_path: Option<String>,
    shape: String,
    cell_width: i64,
    cell_height: i64,
    transform_quarter_turns: i64,
    transform_flip_horizontal: bool,
    transform_flip_vertical: bool,
    gif_loop_mode: String,
    gif_loop_count: Option<i64>,
    crop_mode: String,
    crop_x: f64,
    crop_y: f64,
    crop_w: f64,
    crop_h: f64,
    preset_position: String,
    max_bytes: i64,
}

#[derive(Debug)]
struct PieceRecord {
    id: String,
    piece_index: i64,
    alt_text: String,
}

fn validate_apply_payload(payload: &ApplyIconCropPayload) -> AppResult<()> {
    match payload.shape.as_str() {
        "single" | "horizontal_double" | "vertical_double" => {}
        _ => {
            return Err(AppError::new(
                "validation",
                "지원하지 않는 아이콘 모양입니다.",
            ));
        }
    }

    match payload.crop_mode.as_str() {
        "free" | "fixed" => {}
        _ => {
            return Err(AppError::new(
                "validation",
                "지원하지 않는 크롭 모드입니다.",
            ));
        }
    }

    match payload.preset_position.as_str() {
        "center" | "top_left" | "top" | "top_right" | "left" | "right" | "bottom_left"
        | "bottom" | "bottom_right" | "custom" => {}
        _ => {
            return Err(AppError::new(
                "validation",
                "지원하지 않는 크롭 위치입니다.",
            ));
        }
    }

    match payload.gif_loop_mode.as_str() {
        "preserve" | "infinite" | "once" | "count" | "pingpong" => {}
        _ => {
            return Err(AppError::new(
                "validation",
                "지원하지 않는 GIF 반복 설정입니다.",
            ));
        }
    }

    if payload.gif_loop_mode == "count" && payload.gif_loop_count.unwrap_or(0) <= 0 {
        return Err(AppError::new(
            "validation",
            "사용자 지정 반복 횟수는 1 이상이어야 합니다.",
        ));
    }

    let cell_width = u32::try_from(payload.cell_width)
        .map_err(|_| AppError::new("validation", "셀 너비가 올바르지 않습니다."))?;
    let cell_height = u32::try_from(payload.cell_height)
        .map_err(|_| AppError::new("validation", "셀 높이가 올바르지 않습니다."))?;
    validate_import_dimensions(cell_width, cell_height)?;
    validate_crop_rect(
        payload.crop_x,
        payload.crop_y,
        payload.crop_w,
        payload.crop_h,
    )?;

    let _ = payload_transform(payload)?;

    let expected_piece_count = if payload.shape == "single" { 1 } else { 2 };
    if payload.piece_ids.len() > expected_piece_count {
        return Err(AppError::new(
            "validation",
            "아이콘 조각 순서가 현재 모양과 일치하지 않습니다.",
        ));
    }
    let unique_piece_ids = payload.piece_ids.iter().collect::<HashSet<_>>();
    if unique_piece_ids.len() != payload.piece_ids.len() {
        return Err(AppError::new(
            "validation",
            "같은 아이콘 조각을 두 위치에 배치할 수 없습니다.",
        ));
    }

    Ok(())
}

fn payload_transform(payload: &ApplyIconCropPayload) -> AppResult<ImageTransform> {
    ImageTransform::new(
        payload.transform_quarter_turns,
        payload.transform_flip_horizontal,
        payload.transform_flip_vertical,
    )
}

fn apply_record_for_icon(
    connection: &Connection,
    collection_id: &str,
    icon_id: &str,
) -> AppResult<ApplyRecord> {
    connection
        .query_row(
            "SELECT
               s.width,
               s.height,
               s.original_path_in_library,
               s.original_extension,
               COALESCE(s.original_loop_mode, 'preserve') AS original_loop_mode,
               s.original_loop_count,
               c.default_cell_width,
               c.default_cell_height,
               c.max_bytes
             FROM icons i
             JOIN effective_visual_sources evs ON evs.icon_id = i.id
             JOIN source_files s ON s.id = evs.effective_source_file_id
             JOIN collections c ON c.id = i.collection_id
             WHERE i.id = ?1
               AND i.collection_id = ?2
               AND i.deleted_at IS NULL
               AND c.deleted_at IS NULL",
            params![icon_id, collection_id],
            |row| {
                Ok(ApplyRecord {
                    source_width: row.get("width")?,
                    source_height: row.get("height")?,
                    original_path_in_library: row.get("original_path_in_library")?,
                    original_extension: row.get("original_extension")?,
                    original_loop_mode: row.get("original_loop_mode")?,
                    original_loop_count: row.get("original_loop_count")?,
                    default_cell_width: row.get("default_cell_width")?,
                    default_cell_height: row.get("default_cell_height")?,
                    max_bytes: row.get("max_bytes")?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("편집할 아이콘을 찾을 수 없습니다."))
}

fn validate_generated_piece_outputs(preview: &GeneratedPreview, max_bytes: i64) -> AppResult<()> {
    let max_bytes = u64::try_from(max_bytes.max(1)).unwrap_or(u64::MAX);

    for path in &preview.piece_paths {
        validate_generated_piece_format(path)?;
        let byte_size = fs::metadata(path)?.len();
        if byte_size > max_bytes {
            return Err(AppError::new(
                "validation",
                format!(
                    "처리된 이미지가 모음 용량 제한 {}를 초과했습니다.",
                    format_bytes(max_bytes),
                ),
            ));
        }
    }

    Ok(())
}

fn validate_generated_piece_format(path: &Path) -> AppResult<()> {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    match extension.as_str() {
        "png" | "gif" => Ok(()),
        _ => Err(AppError::new(
            "validation",
            "처리된 미리보기는 png 또는 gif 형식이어야 합니다.",
        )),
    }
}

fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{bytes}B");
    }

    if bytes < 1024 * 1024 {
        return format!("{:.1}KB", bytes as f64 / 1024.0);
    }

    format!("{:.1}MB", bytes as f64 / (1024.0 * 1024.0))
}

fn source_file_for_icon(
    connection: &Connection,
    collection_id: &str,
    icon_id: &str,
) -> AppResult<SourceFileDto> {
    connection
        .query_row(
            "SELECT
               s.id,
               s.original_filename,
               s.original_path_in_library,
               s.original_extension,
               s.mime_type,
               s.sha256,
               s.has_alpha,
               s.width,
               s.height,
               s.byte_size,
               s.is_animated,
               s.frame_count,
               COALESCE(s.original_loop_mode, 'preserve') AS original_loop_mode,
               s.original_loop_count
             FROM source_files s
             JOIN icons i ON i.source_file_id = s.id
             WHERE i.id = ?1
               AND i.collection_id = ?2
               AND i.deleted_at IS NULL",
            params![icon_id, collection_id],
            |row| {
                let is_animated: i64 = row.get("is_animated")?;
                Ok(SourceFileDto {
                    id: row.get("id")?,
                    original_filename: row.get("original_filename")?,
                    original_image_url: row.get("original_path_in_library")?,
                    original_extension: row.get("original_extension")?,
                    mime_type: row.get("mime_type")?,
                    sha256: row.get("sha256")?,
                    has_alpha: row
                        .get::<_, Option<i64>>("has_alpha")?
                        .map(|value| value != 0),
                    width: row.get("width")?,
                    height: row.get("height")?,
                    byte_size: row.get("byte_size")?,
                    is_animated: is_animated != 0,
                    frame_count: row.get("frame_count")?,
                    original_loop_mode: row.get("original_loop_mode")?,
                    original_loop_count: row.get("original_loop_count")?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("원본 이미지를 찾을 수 없습니다."))
}

fn text_overlay_for_icon(
    connection: &Connection,
    collection_id: &str,
    icon_id: &str,
) -> AppResult<TextOverlayDto> {
    connection
        .query_row(
            "SELECT
               text_overlay_enabled,
               text_overlay_text,
               text_overlay_font_path,
               text_overlay_font_size,
               text_overlay_x,
               text_overlay_y,
               text_overlay_color,
               text_overlay_stroke_color,
               text_overlay_stroke_width
             FROM icons
             WHERE id = ?1
               AND collection_id = ?2
               AND deleted_at IS NULL",
            params![icon_id, collection_id],
            |row| {
                let enabled: i64 = row.get("text_overlay_enabled")?;
                Ok(TextOverlayDto {
                    enabled: enabled != 0,
                    text: row.get("text_overlay_text")?,
                    font_path: row.get("text_overlay_font_path")?,
                    font_size: row.get("text_overlay_font_size")?,
                    x: row.get("text_overlay_x")?,
                    y: row.get("text_overlay_y")?,
                    color: row.get("text_overlay_color")?,
                    stroke_color: row.get("text_overlay_stroke_color")?,
                    stroke_width: row.get("text_overlay_stroke_width")?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("텍스트 설정을 찾을 수 없습니다."))
}

fn text_overlay_render_spec_for_icon(
    connection: &Connection,
    collection_id: &str,
    icon_id: &str,
) -> AppResult<Option<TextOverlayRenderSpec>> {
    let overlay = text_overlay_for_icon(connection, collection_id, icon_id)?;
    text_overlay_from_fields(
        overlay.enabled,
        Some(overlay.text),
        overlay.font_path,
        Some(overlay.font_size),
        Some(overlay.x),
        Some(overlay.y),
        Some(overlay.color),
        Some(overlay.stroke_color),
        Some(overlay.stroke_width),
    )
}

fn text_overlay_render_spec_from_payload(
    payload: &UpdateIconTextOverlayPayload,
) -> AppResult<Option<TextOverlayRenderSpec>> {
    text_overlay_from_fields(
        payload.enabled,
        Some(payload.text.clone()),
        payload.font_path.clone(),
        Some(payload.font_size),
        Some(payload.x),
        Some(payload.y),
        Some(payload.color.clone()),
        Some(payload.stroke_color.clone()),
        Some(payload.stroke_width),
    )
}

fn validate_text_overlay_payload(payload: &UpdateIconTextOverlayPayload) -> AppResult<()> {
    if payload.text.chars().count() > 120 {
        return Err(AppError::new(
            "validation",
            "텍스트는 120자 이하로 입력해 주세요.",
        ));
    }
    if !(1.0..=512.0).contains(&payload.font_size) {
        return Err(AppError::new(
            "validation",
            "글자 크기는 1~512px 사이여야 합니다.",
        ));
    }
    if !(0.0..=1.0).contains(&payload.x) || !(0.0..=1.0).contains(&payload.y) {
        return Err(AppError::new(
            "validation",
            "텍스트 위치는 0~100% 범위여야 합니다.",
        ));
    }
    if !(0.0..=64.0).contains(&payload.stroke_width) {
        return Err(AppError::new(
            "validation",
            "외곽선 두께는 0~64px 사이여야 합니다.",
        ));
    }
    let _ = text_overlay_render_spec_from_payload(payload)?;
    Ok(())
}

fn text_overlay_preview_record(
    connection: &Connection,
    collection_id: &str,
    icon_id: &str,
) -> AppResult<TextOverlayPreviewRecord> {
    connection
        .query_row(
            "SELECT
               s.original_path_in_library,
               s.original_extension,
               s.sha256 AS source_hash,
               COALESCE(s.original_loop_mode, 'preserve') AS original_loop_mode,
               s.original_loop_count,
               i.current_preview_path,
               i.shape,
               COALESCE(i.cell_width_override, c.default_cell_width) AS cell_width,
               COALESCE(i.cell_height_override, c.default_cell_height) AS cell_height,
               i.transform_quarter_turns,
               i.transform_flip_horizontal,
               i.transform_flip_vertical,
               CASE WHEN i.gif_pingpong = 1 THEN 'pingpong' ELSE i.gif_loop_mode END AS gif_loop_mode,
               i.gif_loop_count,
               cs.crop_mode,
               cs.crop_x,
               cs.crop_y,
               cs.crop_w,
               cs.crop_h,
               cs.preset_position,
               c.max_bytes
             FROM icons i
             JOIN effective_visual_sources evs ON evs.icon_id = i.id
             JOIN source_files s ON s.id = evs.effective_source_file_id
             JOIN collections c ON c.id = i.collection_id
             JOIN crop_settings cs ON cs.icon_id = i.id
             WHERE i.id = ?1
               AND i.collection_id = ?2
               AND i.deleted_at IS NULL
               AND c.deleted_at IS NULL",
            params![icon_id, collection_id],
            |row| {
                Ok(TextOverlayPreviewRecord {
                    original_path_in_library: row.get("original_path_in_library")?,
                    original_extension: row.get("original_extension")?,
                    source_hash: row.get("source_hash")?,
                    original_loop_mode: row.get("original_loop_mode")?,
                    original_loop_count: row.get("original_loop_count")?,
                    current_preview_path: row.get("current_preview_path")?,
                    shape: row.get("shape")?,
                    cell_width: row.get("cell_width")?,
                    cell_height: row.get("cell_height")?,
                    transform_quarter_turns: row.get("transform_quarter_turns")?,
                    transform_flip_horizontal:
                        row.get::<_, i64>("transform_flip_horizontal")? != 0,
                    transform_flip_vertical:
                        row.get::<_, i64>("transform_flip_vertical")? != 0,
                    gif_loop_mode: row.get("gif_loop_mode")?,
                    gif_loop_count: row.get("gif_loop_count")?,
                    crop_mode: row.get("crop_mode")?,
                    crop_x: row.get("crop_x")?,
                    crop_y: row.get("crop_y")?,
                    crop_w: row.get("crop_w")?,
                    crop_h: row.get("crop_h")?,
                    preset_position: row.get("preset_position")?,
                    max_bytes: row.get("max_bytes")?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("텍스트를 적용할 아이콘을 찾을 수 없습니다."))
}

fn update_text_overlay_record(
    transaction: &Transaction<'_>,
    collection_id: &str,
    payload: &UpdateIconTextOverlayPayload,
    current_preview_path: &str,
) -> AppResult<()> {
    transaction.execute(
        "UPDATE icons
         SET text_overlay_enabled = ?1,
             text_overlay_text = ?2,
             text_overlay_font_path = ?3,
             text_overlay_font_size = ?4,
             text_overlay_x = ?5,
             text_overlay_y = ?6,
             text_overlay_color = ?7,
             text_overlay_stroke_color = ?8,
             text_overlay_stroke_width = ?9,
             current_preview_path = ?10,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?11
           AND collection_id = ?12
           AND deleted_at IS NULL",
        params![
            if payload.enabled { 1 } else { 0 },
            payload.text.trim(),
            payload
                .font_path
                .as_ref()
                .map(|value| value.trim())
                .filter(|value| !value.is_empty()),
            payload.font_size,
            payload.x,
            payload.y,
            payload.color,
            payload.stroke_color,
            payload.stroke_width,
            current_preview_path,
            payload.icon_id,
            collection_id,
        ],
    )?;
    Ok(())
}

fn record_as_crop(record: &TextOverlayPreviewRecord, icon_id: &str) -> ApplyIconCropPayload {
    ApplyIconCropPayload {
        icon_id: icon_id.to_string(),
        shape: record.shape.clone(),
        crop_mode: record.crop_mode.clone(),
        crop_x: record.crop_x,
        crop_y: record.crop_y,
        crop_w: record.crop_w,
        crop_h: record.crop_h,
        preset_position: record.preset_position.clone(),
        cell_width: record.cell_width,
        cell_height: record.cell_height,
        transform_quarter_turns: record.transform_quarter_turns,
        transform_flip_horizontal: record.transform_flip_horizontal,
        transform_flip_vertical: record.transform_flip_vertical,
        piece_ids: Vec::new(),
        gif_loop_mode: record.gif_loop_mode.clone(),
        gif_loop_count: record.gif_loop_count,
    }
}

#[derive(Debug)]
struct OwnedEffectPreviewRequest {
    icon_root: PathBuf,
    request_dir: PathBuf,
    completed: bool,
}

impl OwnedEffectPreviewRequest {
    fn create(paths: &AppPaths, icon_id: &str, signature: &str) -> AppResult<Self> {
        if !is_safe_effect_preview_signature(signature) {
            return Err(effect_preview_path_error(
                "효과 미리보기 서명이 올바르지 않습니다.",
            ));
        }

        let icon_root = effect_preview_icon_root(paths, icon_id)?;
        let signature_dir = prepare_real_child_directory(&icon_root, signature)?;
        for _ in 0..32 {
            let request_token = create_id("fxpreview");
            if !is_safe_effect_preview_request_name(&request_token) {
                continue;
            }
            let request_dir = signature_dir.join(&request_token);
            match fs::create_dir(&request_dir) {
                Ok(()) => {
                    let marker = request_dir.join(EFFECT_PREVIEW_IN_PROGRESS_MARKER);
                    match fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(&marker)
                    {
                        Ok(_) => {
                            return Ok(Self {
                                icon_root,
                                request_dir,
                                completed: false,
                            });
                        }
                        Err(error) => {
                            let _ = fs::remove_dir(&request_dir);
                            return Err(AppError::new(
                                "effect_preview_path",
                                format!("효과 미리보기 진행 상태를 만들지 못했습니다: {error}"),
                            ));
                        }
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            }
        }

        Err(AppError::new(
            "effect_preview_path",
            "효과 미리보기 요청 디렉터리를 안전하게 만들지 못했습니다.",
        ))
    }

    fn directory(&self) -> &Path {
        &self.request_dir
    }

    fn icon_root(&self) -> &Path {
        &self.icon_root
    }

    fn mark_completed(&mut self) -> AppResult<()> {
        let in_progress = self.request_dir.join(EFFECT_PREVIEW_IN_PROGRESS_MARKER);
        let completed = self.request_dir.join(EFFECT_PREVIEW_COMPLETE_MARKER);
        fs::rename(&in_progress, &completed).map_err(|error| {
            AppError::new(
                "effect_preview_path",
                format!("효과 미리보기 완료 상태를 기록하지 못했습니다: {error}"),
            )
        })?;
        self.completed = true;
        Ok(())
    }
}

impl Drop for OwnedEffectPreviewRequest {
    fn drop(&mut self) {
        if !self.completed {
            remove_effect_preview_request_directory(&self.icon_root, &self.request_dir, true);
        }
    }
}

#[derive(Debug)]
struct CompletedEffectPreviewRequest {
    path: PathBuf,
    completed_at: SystemTime,
}

fn effect_preview_icon_root(paths: &AppPaths, icon_id: &str) -> AppResult<PathBuf> {
    if !is_safe_effect_preview_component(icon_id) {
        return Err(effect_preview_path_error(
            "아이콘 ID가 안전한 경로 구성 요소가 아닙니다.",
        ));
    }

    let root_metadata = fs::symlink_metadata(&paths.root)?;
    let temp_metadata = fs::symlink_metadata(&paths.temp_export_dir)?;
    if root_metadata.file_type().is_symlink()
        || !root_metadata.is_dir()
        || temp_metadata.file_type().is_symlink()
        || !temp_metadata.is_dir()
    {
        return Err(effect_preview_path_error(
            "효과 미리보기 임시 경로가 실제 디렉터리가 아닙니다.",
        ));
    }

    let canonical_root = paths.root.canonicalize()?;
    let canonical_temp = paths.temp_export_dir.canonicalize()?;
    if canonical_temp != canonical_root.join("temp").join("export") {
        return Err(effect_preview_path_error(
            "효과 미리보기 임시 경로가 앱 데이터 경로를 벗어났습니다.",
        ));
    }

    let previews_root =
        prepare_real_child_directory(&paths.temp_export_dir, EFFECT_PREVIEW_DIRECTORY)?;
    prepare_real_child_directory(&previews_root, icon_id)
}

fn prepare_real_child_directory(parent: &Path, component: &str) -> AppResult<PathBuf> {
    if !is_safe_effect_preview_component(component) {
        return Err(effect_preview_path_error(
            "효과 미리보기 경로 구성 요소가 올바르지 않습니다.",
        ));
    }

    let parent_metadata = fs::symlink_metadata(parent)?;
    if parent_metadata.file_type().is_symlink() || !parent_metadata.is_dir() {
        return Err(effect_preview_path_error(
            "효과 미리보기 상위 경로가 실제 디렉터리가 아닙니다.",
        ));
    }
    let canonical_parent = parent.canonicalize()?;
    let target = parent.join(component);
    match fs::create_dir(&target) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error.into()),
    }

    let metadata = fs::symlink_metadata(&target)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(effect_preview_path_error(
            "효과 미리보기 경로가 심볼릭 링크 또는 정션입니다.",
        ));
    }
    let canonical_target = target.canonicalize()?;
    if canonical_target != canonical_parent.join(component) {
        return Err(effect_preview_path_error(
            "효과 미리보기 경로가 허용된 상위 경로를 벗어났습니다.",
        ));
    }
    Ok(target)
}

fn is_safe_effect_preview_component(value: &str) -> bool {
    if value.is_empty() || value.len() > 160 {
        return false;
    }
    let mut components = Path::new(value).components();
    matches!(components.next(), Some(Component::Normal(_)))
        && components.next().is_none()
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
}

fn is_safe_effect_preview_signature(value: &str) -> bool {
    value.len() == 64 && value.chars().all(|character| character.is_ascii_hexdigit())
}

fn is_safe_effect_preview_request_name(value: &str) -> bool {
    value.starts_with("fxpreview_")
        && value.len() <= 128
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
}

fn effect_preview_path_error(message: impl Into<String>) -> AppError {
    AppError::new("effect_preview_path", message)
}

fn prune_completed_effect_preview_requests(icon_root: &Path, current: &Path) {
    let mut candidates = completed_effect_preview_requests(icon_root);
    candidates.sort_by(|left, right| {
        right
            .completed_at
            .cmp(&left.completed_at)
            .then_with(|| right.path.cmp(&left.path))
    });

    let mut retained = HashSet::with_capacity(MAX_COMPLETED_EFFECT_PREVIEWS_PER_ICON);
    retained.insert(current.to_path_buf());
    for candidate in &candidates {
        if retained.len() >= MAX_COMPLETED_EFFECT_PREVIEWS_PER_ICON {
            break;
        }
        retained.insert(candidate.path.clone());
    }

    for candidate in candidates {
        if !retained.contains(&candidate.path) {
            remove_effect_preview_request_directory(icon_root, &candidate.path, false);
        }
    }
}

fn completed_effect_preview_requests(icon_root: &Path) -> Vec<CompletedEffectPreviewRequest> {
    let Ok(root_metadata) = fs::symlink_metadata(icon_root) else {
        return Vec::new();
    };
    if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
        return Vec::new();
    }

    let Ok(signature_entries) = fs::read_dir(icon_root) else {
        return Vec::new();
    };
    let mut candidates = Vec::new();
    for signature_entry in signature_entries.flatten() {
        let signature_path = signature_entry.path();
        let Some(signature_name) = signature_entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let Ok(file_type) = signature_entry.file_type() else {
            continue;
        };
        if !is_safe_effect_preview_signature(&signature_name)
            || file_type.is_symlink()
            || !file_type.is_dir()
        {
            continue;
        }
        let Ok(request_entries) = fs::read_dir(&signature_path) else {
            continue;
        };

        for request_entry in request_entries.flatten() {
            let request_path = request_entry.path();
            let Some(request_name) = request_entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let Ok(file_type) = request_entry.file_type() else {
                continue;
            };
            if !is_safe_effect_preview_request_name(&request_name)
                || file_type.is_symlink()
                || !file_type.is_dir()
                || canonical_effect_preview_request(icon_root, &request_path).is_none()
            {
                continue;
            }

            match fs::symlink_metadata(request_path.join(EFFECT_PREVIEW_IN_PROGRESS_MARKER)) {
                Ok(_) => continue,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => continue,
            }

            let completed_at =
                match fs::symlink_metadata(request_path.join(EFFECT_PREVIEW_COMPLETE_MARKER)) {
                    Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => {
                        metadata.modified().unwrap_or(UNIX_EPOCH)
                    }
                    Ok(_) => continue,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => request_entry
                        .metadata()
                        .and_then(|metadata| metadata.modified())
                        .unwrap_or(UNIX_EPOCH),
                    Err(_) => continue,
                };
            candidates.push(CompletedEffectPreviewRequest {
                path: request_path,
                completed_at,
            });
        }
    }
    candidates
}

fn canonical_effect_preview_request(icon_root: &Path, candidate: &Path) -> Option<PathBuf> {
    let mut relative = candidate.strip_prefix(icon_root).ok()?.components();
    let Component::Normal(signature_component) = relative.next()? else {
        return None;
    };
    let Component::Normal(request_component) = relative.next()? else {
        return None;
    };
    if relative.next().is_some() {
        return None;
    }
    let signature = signature_component.to_str()?;
    let request = request_component.to_str()?;
    if !is_safe_effect_preview_signature(signature) || !is_safe_effect_preview_request_name(request)
    {
        return None;
    }

    let root_metadata = fs::symlink_metadata(icon_root).ok()?;
    let signature_path = icon_root.join(signature_component);
    let signature_metadata = fs::symlink_metadata(&signature_path).ok()?;
    let candidate_metadata = fs::symlink_metadata(candidate).ok()?;
    if root_metadata.file_type().is_symlink()
        || signature_metadata.file_type().is_symlink()
        || candidate_metadata.file_type().is_symlink()
        || !root_metadata.is_dir()
        || !signature_metadata.is_dir()
        || !candidate_metadata.is_dir()
    {
        return None;
    }

    let canonical_root = icon_root.canonicalize().ok()?;
    let canonical_signature = signature_path.canonicalize().ok()?;
    let canonical_candidate = candidate.canonicalize().ok()?;
    if canonical_signature != canonical_root.join(signature_component)
        || canonical_candidate != canonical_signature.join(request_component)
    {
        return None;
    }
    Some(canonical_candidate)
}

fn remove_effect_preview_request_directory(
    icon_root: &Path,
    candidate: &Path,
    allow_in_progress: bool,
) {
    let Some(canonical_candidate) = canonical_effect_preview_request(icon_root, candidate) else {
        return;
    };
    if !allow_in_progress
        && fs::symlink_metadata(canonical_candidate.join(EFFECT_PREVIEW_IN_PROGRESS_MARKER)).is_ok()
    {
        return;
    }
    let _ = fs::remove_dir_all(canonical_candidate);
}

#[derive(Debug)]
struct OwnedEffectArtifact {
    effect_root: PathBuf,
    signature_prefix: String,
    request_token: String,
    staging_dir: PathBuf,
    final_dir: Option<PathBuf>,
    keep_final: bool,
}

impl OwnedEffectArtifact {
    fn create(effect_root: &Path, signature: &str) -> AppResult<Self> {
        let signature_prefix = signature
            .get(..16)
            .ok_or_else(|| AppError::new("validation", "effect signature is too short"))?
            .to_string();
        let staging_root = effect_root.join(".staging");
        fs::create_dir_all(&staging_root)?;

        for _ in 0..32 {
            let request_token = create_id("fxsave");
            let staging_dir = staging_root.join(&request_token);
            match fs::create_dir(&staging_dir) {
                Ok(()) => {
                    return Ok(Self {
                        effect_root: effect_root.to_path_buf(),
                        signature_prefix,
                        request_token,
                        staging_dir,
                        final_dir: None,
                        keep_final: false,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            }
        }

        Err(AppError::new(
            "effect_artifact_collision",
            "효과 저장용 임시 폴더를 안전하게 만들 수 없습니다.",
        ))
    }

    fn staging_dir(&self) -> &Path {
        &self.staging_dir
    }

    fn promote(&mut self, revision: i64) -> AppResult<PathBuf> {
        if self.final_dir.is_some() {
            return Err(AppError::new(
                "effect_artifact_state",
                "효과 저장 결과가 이미 승격되었습니다.",
            ));
        }
        let final_dir = self.effect_root.join(format!(
            "{revision}-{}-{}",
            self.signature_prefix, self.request_token
        ));
        if final_dir.exists() {
            return Err(AppError::new(
                "effect_artifact_collision",
                "효과 저장 결과 폴더가 이미 존재합니다.",
            ));
        }

        // The staging directory is deliberately a child of the same effect root.
        // A same-volume rename therefore avoids partially copied directories and,
        // on Windows, fails instead of replacing an existing destination.
        fs::rename(&self.staging_dir, &final_dir).map_err(|error| {
            AppError::new(
                "effect_artifact_promote_failed",
                format!("효과 저장 결과를 확정하지 못했습니다: {error}"),
            )
        })?;
        self.final_dir = Some(final_dir.clone());
        Ok(final_dir)
    }

    fn keep_final(&mut self) {
        self.keep_final = true;
    }

    fn owns_staging_dir(&self) -> bool {
        self.staging_dir == self.effect_root.join(".staging").join(&self.request_token)
    }

    fn owns_final_dir(&self, path: &Path) -> bool {
        path.parent() == Some(self.effect_root.as_path())
            && path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(&format!("-{}", self.request_token)))
    }
}

impl Drop for OwnedEffectArtifact {
    fn drop(&mut self) {
        if self.keep_final {
            return;
        }
        if let Some(final_dir) = &self.final_dir {
            if self.owns_final_dir(final_dir) {
                let _ = fs::remove_dir_all(final_dir);
            }
        }
        if self.owns_staging_dir() {
            let _ = fs::remove_dir_all(&self.staging_dir);
        }
    }
}

fn rebase_generated_preview(
    generated: &mut GeneratedPreview,
    staging_dir: &Path,
    final_dir: &Path,
) -> AppResult<()> {
    generated.current_preview_path =
        rebase_effect_artifact_path(&generated.current_preview_path, staging_dir, final_dir)?;
    generated.poster_path =
        rebase_effect_artifact_path(&generated.poster_path, staging_dir, final_dir)?;
    for piece_path in &mut generated.piece_paths {
        *piece_path = rebase_effect_artifact_path(piece_path, staging_dir, final_dir)?;
    }
    Ok(())
}

fn rebase_effect_artifact_path(
    path: &Path,
    staging_dir: &Path,
    final_dir: &Path,
) -> AppResult<PathBuf> {
    let relative = path.strip_prefix(staging_dir).map_err(|_| {
        AppError::new(
            "effect_artifact_path",
            "효과 저장 결과가 요청 전용 임시 폴더 밖을 가리킵니다.",
        )
    })?;
    Ok(final_dir.join(relative))
}

fn render_effect_preview(
    preview_dir: &Path,
    collection_id: &str,
    icon_id: &str,
    record: &TextOverlayPreviewRecord,
    text_overlay: Option<TextOverlayRenderSpec>,
    effects: EffectRecipe,
    motion: MotionRecipe,
) -> AppResult<GeneratedPreview> {
    let transform = ImageTransform::new(
        record.transform_quarter_turns,
        record.transform_flip_horizontal,
        record.transform_flip_vertical,
    )?;
    generate_icon_preview_in_directory(
        preview_dir,
        GeneratePreviewRequest {
            collection_id,
            icon_id,
            source_path: Path::new(&record.original_path_in_library),
            source_extension: &record.original_extension,
            shape: &record.shape,
            crop: CropRect {
                x: record.crop_x,
                y: record.crop_y,
                width: record.crop_w,
                height: record.crop_h,
            },
            cell_width: record.cell_width,
            cell_height: record.cell_height,
            transform,
            gif_loop_mode: &record.gif_loop_mode,
            gif_loop_count: record.gif_loop_count,
            source_gif_loop_mode: Some(&record.original_loop_mode),
            source_gif_loop_count: record.original_loop_count,
            text_overlay,
            effects,
            motion,
        },
    )
}

fn effect_preview_signature(
    record: &TextOverlayPreviewRecord,
    text_overlay: Option<&TextOverlayRenderSpec>,
    recipe: &EffectRecipe,
    motion: &MotionRecipe,
) -> AppResult<String> {
    let mut parts = vec![
        "effect_preview_v2_motion".to_string(),
        record.source_hash.clone(),
        record.shape.clone(),
        record.crop_x.to_bits().to_string(),
        record.crop_y.to_bits().to_string(),
        record.crop_w.to_bits().to_string(),
        record.crop_h.to_bits().to_string(),
        record.cell_width.to_string(),
        record.cell_height.to_string(),
        record.transform_quarter_turns.to_string(),
        record.transform_flip_horizontal.to_string(),
        record.transform_flip_vertical.to_string(),
        record.gif_loop_mode.clone(),
        record.gif_loop_count.unwrap_or_default().to_string(),
    ];
    if let Some(text_overlay) = text_overlay {
        parts.extend(text_overlay.normalized_hash_parts());
    }
    parts.extend(recipe.normalized_hash_parts()?);
    parts.extend(motion.normalized_hash_parts()?);
    Ok(hash_text(&parts))
}

fn effect_preview_dto(
    generated: GeneratedPreview,
    record: &TextOverlayPreviewRecord,
    recipe: &EffectRecipe,
    recipe_signature: String,
    processing_ms: u128,
) -> AppResult<EffectPreviewDto> {
    validate_generated_piece_formats(&generated)?;
    let byte_size = metadata_size(&generated.current_preview_path)?;
    let max_piece_byte_size = generated
        .piece_paths
        .iter()
        .map(|path| metadata_size(path))
        .collect::<AppResult<Vec<_>>>()?
        .into_iter()
        .max()
        .unwrap_or(0);
    let mut warnings = Vec::new();
    if max_piece_byte_size > record.max_bytes {
        warnings.push(format!(
            "가장 큰 출력 조각이 {}로 모음 제한 {}를 넘습니다. 효과는 저장할 수 있지만 내보내기 전에 줄여야 합니다.",
            format_bytes(u64::try_from(max_piece_byte_size.max(0)).unwrap_or(u64::MAX)),
            format_bytes(u64::try_from(record.max_bytes.max(0)).unwrap_or(u64::MAX)),
        ));
    }
    if recipe.effects.iter().any(|effect| {
        effect.enabled()
            && matches!(
                effect,
                EffectStep::Outline { .. } | EffectStep::Shadow { .. }
            )
    }) {
        warnings.push(
            "윤곽선·그림자는 현재 출력 캔버스 안에서 처리되어 가장자리에서 잘릴 수 있습니다."
                .to_string(),
        );
    }
    let frame_count = i64::try_from(generated.frame_count)
        .unwrap_or(i64::MAX)
        .max(1);
    if frame_count > 1
        && recipe.effects.iter().any(|effect| {
            effect.enabled()
                && matches!(
                    effect,
                    EffectStep::Blur { .. }
                        | EffectStep::Outline { .. }
                        | EffectStep::Shadow { .. }
                )
        })
    {
        warnings.push(format!(
            "비용이 큰 효과를 GIF {frame_count}프레임 전체에 적용했습니다. 실제 처리 시간과 용량을 확인하세요."
        ));
    }

    let generated_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().to_string())
        .unwrap_or_else(|_| recipe_signature.clone());
    Ok(EffectPreviewDto {
        preview_path: generated.current_preview_path.to_string_lossy().to_string(),
        byte_size,
        max_piece_byte_size,
        max_bytes: record.max_bytes,
        frame_count,
        processing_ms: i64::try_from(processing_ms).unwrap_or(i64::MAX),
        warnings,
        recipe_signature,
        generated_at,
    })
}

fn metadata_size(path: &Path) -> AppResult<i64> {
    Ok(i64::try_from(fs::metadata(path)?.len()).unwrap_or(i64::MAX))
}

fn validate_generated_piece_formats(preview: &GeneratedPreview) -> AppResult<()> {
    validate_generated_piece_format(&preview.current_preview_path)?;
    for path in &preview.piece_paths {
        validate_generated_piece_format(path)?;
    }
    Ok(())
}

fn update_effect_preview_record(
    transaction: &Transaction<'_>,
    collection_id: &str,
    icon_id: &str,
    current_preview_path: &str,
) -> AppResult<()> {
    transaction.execute(
        "UPDATE icons
         SET current_preview_path = ?1,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?2
           AND collection_id = ?3
           AND deleted_at IS NULL",
        params![current_preview_path, icon_id, collection_id],
    )?;
    transaction.execute(
        "UPDATE collections
         SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?1
           AND cover_icon_id = ?2
           AND deleted_at IS NULL",
        params![collection_id, icon_id],
    )?;
    Ok(())
}

fn cleanup_previous_effect_preview(
    connection: &Connection,
    effect_root: &Path,
    previous: Option<&str>,
    current: &Path,
) {
    let Some(previous) = previous else {
        return;
    };
    let previous = Path::new(previous);
    let Some(previous_dir) = previous.parent() else {
        return;
    };
    if previous_dir == current || !is_owned_effect_preview_directory(effect_root, previous_dir) {
        return;
    }

    let (Ok(canonical_root), Ok(canonical_previous)) =
        (effect_root.canonicalize(), previous_dir.canonicalize())
    else {
        return;
    };
    let Some(directory_name) = previous_dir.file_name() else {
        return;
    };
    if canonical_previous != canonical_root.join(directory_name) {
        return;
    }
    if current
        .canonicalize()
        .ok()
        .is_some_and(|canonical_current| canonical_current == canonical_previous)
    {
        return;
    }
    if matches!(
        effect_preview_directory_is_referenced(connection, previous_dir),
        Ok(false)
    ) {
        let _ = fs::remove_dir_all(canonical_previous);
    }
}

fn is_owned_effect_preview_directory(effect_root: &Path, candidate: &Path) -> bool {
    let Some(relative) = candidate.strip_prefix(effect_root).ok() else {
        return false;
    };
    let mut components = relative.components();
    let Some(Component::Normal(name)) = components.next() else {
        return false;
    };
    if components.next().is_some() {
        return false;
    }
    let Some(name) = name.to_str() else {
        return false;
    };
    let mut parts = name.splitn(3, '-');
    let Some(revision) = parts.next() else {
        return false;
    };
    let Some(signature) = parts.next() else {
        return false;
    };
    let Some(request_token) = parts.next() else {
        return false;
    };
    !revision.is_empty()
        && revision.chars().all(|character| character.is_ascii_digit())
        && signature.len() == 16
        && signature
            .chars()
            .all(|character| character.is_ascii_hexdigit())
        && request_token.starts_with("fxsave_")
        && request_token
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
}
fn effect_preview_directory_is_referenced(
    connection: &Connection,
    directory: &Path,
) -> AppResult<bool> {
    let mut statement = connection.prepare(
        "SELECT current_preview_path AS referenced_path
         FROM icons
         WHERE current_preview_path IS NOT NULL
         UNION ALL
         SELECT generated_preview_path AS referenced_path
         FROM icon_pieces
         WHERE generated_preview_path IS NOT NULL",
    )?;
    let referenced_paths = statement.query_map([], |row| row.get::<_, String>(0))?;
    for referenced_path in referenced_paths {
        if Path::new(&referenced_path?).starts_with(directory) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn crop_settings_for_icon(connection: &Connection, icon_id: &str) -> AppResult<CropSettingsDto> {
    connection
        .query_row(
            "SELECT
               crop_mode,
               crop_x,
               crop_y,
               crop_w,
               crop_h,
               preset_position,
               source_width_at_apply,
               source_height_at_apply,
               viewport_width_at_apply,
               viewport_height_at_apply,
               updated_at
             FROM crop_settings
             WHERE icon_id = ?1",
            params![icon_id],
            |row| {
                Ok(CropSettingsDto {
                    crop_mode: row.get("crop_mode")?,
                    crop_x: row.get("crop_x")?,
                    crop_y: row.get("crop_y")?,
                    crop_w: row.get("crop_w")?,
                    crop_h: row.get("crop_h")?,
                    preset_position: row.get("preset_position")?,
                    source_width_at_apply: row.get("source_width_at_apply")?,
                    source_height_at_apply: row.get("source_height_at_apply")?,
                    viewport_width_at_apply: row.get("viewport_width_at_apply")?,
                    viewport_height_at_apply: row.get("viewport_height_at_apply")?,
                    updated_at: row.get("updated_at")?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("크롭 설정을 찾을 수 없습니다."))
}

fn ensure_icon_still_editable(
    transaction: &Transaction<'_>,
    collection_id: &str,
    icon_id: &str,
) -> AppResult<()> {
    let exists = transaction
        .query_row(
            "SELECT id
             FROM icons
             WHERE id = ?1
               AND collection_id = ?2
               AND deleted_at IS NULL",
            params![icon_id, collection_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .is_some();

    if exists {
        Ok(())
    } else {
        Err(AppError::not_found("편집할 아이콘을 찾을 수 없습니다."))
    }
}

fn update_icon_record(
    transaction: &Transaction<'_>,
    collection_id: &str,
    payload: &ApplyIconCropPayload,
    apply_record: &ApplyRecord,
    transform: ImageTransform,
    current_preview_path: &str,
) -> AppResult<()> {
    let cell_width_override = if payload.cell_width == apply_record.default_cell_width {
        None
    } else {
        Some(payload.cell_width)
    };
    let cell_height_override = if payload.cell_height == apply_record.default_cell_height {
        None
    } else {
        Some(payload.cell_height)
    };
    let gif_loop_count = if payload.gif_loop_mode == "count" {
        payload.gif_loop_count
    } else {
        None
    };
    let gif_pingpong = payload.gif_loop_mode == "pingpong";
    let stored_gif_loop_mode = if gif_pingpong {
        "infinite"
    } else {
        payload.gif_loop_mode.as_str()
    };

    transaction.execute(
        "UPDATE icons
         SET shape = ?1,
             cell_width_override = ?2,
             cell_height_override = ?3,
             current_preview_path = ?4,
             transform_quarter_turns = ?5,
             transform_flip_horizontal = ?6,
             transform_flip_vertical = ?7,
             gif_loop_mode = ?8,
             gif_loop_count = ?9,
             gif_pingpong = ?10,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?11
           AND collection_id = ?12
           AND deleted_at IS NULL",
        params![
            payload.shape,
            cell_width_override,
            cell_height_override,
            current_preview_path,
            transform.quarter_turns,
            if transform.flip_horizontal { 1 } else { 0 },
            if transform.flip_vertical { 1 } else { 0 },
            stored_gif_loop_mode,
            gif_loop_count,
            if gif_pingpong { 1 } else { 0 },
            payload.icon_id,
            collection_id,
        ],
    )?;
    transaction.execute(
        "UPDATE collections
         SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?1
           AND cover_icon_id = ?2
           AND deleted_at IS NULL",
        params![collection_id, payload.icon_id],
    )?;

    Ok(())
}

fn upsert_crop_settings(
    transaction: &Transaction<'_>,
    payload: &ApplyIconCropPayload,
    source_width: i64,
    source_height: i64,
    viewport_width: i64,
    viewport_height: i64,
) -> AppResult<()> {
    transaction.execute(
        "INSERT INTO crop_settings (
           id,
           icon_id,
           crop_mode,
           crop_x,
           crop_y,
           crop_w,
           crop_h,
           preset_position,
           source_width_at_apply,
           source_height_at_apply,
           viewport_width_at_apply,
           viewport_height_at_apply,
           updated_at
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
           strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         )
         ON CONFLICT(icon_id) DO UPDATE SET
           crop_mode = excluded.crop_mode,
           crop_x = excluded.crop_x,
           crop_y = excluded.crop_y,
           crop_w = excluded.crop_w,
           crop_h = excluded.crop_h,
           preset_position = excluded.preset_position,
           source_width_at_apply = excluded.source_width_at_apply,
           source_height_at_apply = excluded.source_height_at_apply,
           viewport_width_at_apply = excluded.viewport_width_at_apply,
           viewport_height_at_apply = excluded.viewport_height_at_apply,
           updated_at = excluded.updated_at",
        params![
            create_id("crop"),
            payload.icon_id,
            payload.crop_mode,
            payload.crop_x,
            payload.crop_y,
            payload.crop_w,
            payload.crop_h,
            payload.preset_position,
            source_width,
            source_height,
            viewport_width,
            viewport_height,
        ],
    )?;

    Ok(())
}

fn reconcile_icon_pieces(
    transaction: &Transaction<'_>,
    collection_id: &str,
    payload: &ApplyIconCropPayload,
    piece_paths: &[PathBuf],
) -> AppResult<()> {
    let roles = piece_roles(&payload.shape)?;
    if roles.len() != piece_paths.len() {
        return Err(AppError::new(
            "validation",
            "아이콘 조각 수와 생성된 미리보기 수가 일치하지 않습니다.",
        ));
    }

    let existing_pieces = pieces_for_icon(transaction, &payload.icon_id)?;
    let existing_ids = existing_pieces
        .iter()
        .map(|piece| piece.id.as_str())
        .collect::<HashSet<_>>();
    for piece_id in &payload.piece_ids {
        if !existing_ids.contains(piece_id.as_str()) {
            return Err(AppError::new(
                "validation",
                "다른 아이콘의 조각은 이 아이콘에 배치할 수 없습니다.",
            ));
        }
    }

    let mut used_alt_texts = collection_alt_texts(transaction, collection_id, &payload.icon_id)?;
    for piece in &existing_pieces {
        used_alt_texts.insert(piece.alt_text.clone());
    }

    transaction.execute(
        "UPDATE icon_pieces
         SET piece_index = piece_index + 1000
         WHERE icon_id = ?1",
        params![payload.icon_id],
    )?;

    let mut used_piece_ids = HashSet::new();
    for (piece_index, role) in roles.iter().enumerate() {
        let path = piece_paths[piece_index].to_string_lossy().to_string();
        let requested_id = payload.piece_ids.get(piece_index);
        let existing = requested_id
            .and_then(|piece_id| existing_pieces.iter().find(|piece| piece.id == *piece_id))
            .or_else(|| {
                existing_pieces.iter().find(|piece| {
                    piece.piece_index == piece_index as i64
                        && !used_piece_ids.contains(piece.id.as_str())
                })
            })
            .or_else(|| {
                existing_pieces
                    .iter()
                    .find(|piece| !used_piece_ids.contains(piece.id.as_str()))
            });

        if let Some(existing) = existing {
            used_piece_ids.insert(existing.id.as_str());
            transaction.execute(
                "UPDATE icon_pieces
                 SET piece_index = ?1,
                     piece_role = ?2,
                     generated_preview_path = ?3,
                     export_status = 'ready',
                     updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE id = ?4",
                params![piece_index as i64, role, path, existing.id],
            )?;
        } else {
            let alt_text = next_unique_alt(&mut used_alt_texts, role);
            transaction.execute(
                "INSERT INTO icon_pieces (
                   id,
                   icon_id,
                   piece_index,
                   piece_role,
                   alt_text,
                   generated_preview_path,
                   export_status,
                   created_at,
                   updated_at
                 )
                 VALUES (
                   ?1,
                   ?2,
                   ?3,
                   ?4,
                   ?5,
                   ?6,
                   'ready',
                   strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                   strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 )",
                params![
                    create_id("piece"),
                    payload.icon_id,
                    piece_index as i64,
                    role,
                    alt_text,
                    path,
                ],
            )?;
        }
    }

    transaction.execute(
        "DELETE FROM icon_pieces
         WHERE icon_id = ?1
           AND piece_index >= ?2",
        params![payload.icon_id, roles.len() as i64],
    )?;

    Ok(())
}

fn pieces_for_icon(transaction: &Transaction<'_>, icon_id: &str) -> AppResult<Vec<PieceRecord>> {
    let mut statement = transaction.prepare(
        "SELECT id, piece_index, alt_text
         FROM icon_pieces
         WHERE icon_id = ?1
         ORDER BY piece_index ASC",
    )?;

    let pieces = statement
        .query_map(params![icon_id], |row| {
            Ok(PieceRecord {
                id: row.get("id")?,
                piece_index: row.get("piece_index")?,
                alt_text: row.get("alt_text")?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(pieces)
}

fn collection_alt_texts(
    transaction: &Transaction<'_>,
    collection_id: &str,
    edited_icon_id: &str,
) -> AppResult<HashSet<String>> {
    let mut statement = transaction.prepare(
        "SELECT p.alt_text
         FROM icon_pieces p
         JOIN icons i ON i.id = p.icon_id
         WHERE i.collection_id = ?1
           AND i.id <> ?2
           AND i.deleted_at IS NULL",
    )?;

    let values = statement
        .query_map(params![collection_id, edited_icon_id], |row| {
            row.get::<_, String>(0)
        })?
        .collect::<Result<HashSet<_>, _>>()?;

    Ok(values)
}

fn next_unique_alt(used_alt_texts: &mut HashSet<String>, role: &str) -> String {
    let preferred = match role {
        "left" => ["좌", "왼", "가"],
        "right" => ["우", "오", "나"],
        "top" => ["상", "위", "가"],
        "bottom" => ["하", "아", "나"],
        _ => ["가", "나", "다"],
    };

    for candidate in preferred {
        if !used_alt_texts.contains(candidate) {
            used_alt_texts.insert(candidate.to_string());
            return candidate.to_string();
        }
    }

    for character in [
        "다", "라", "마", "바", "사", "자", "차", "카", "타", "파", "A", "B", "C",
    ] {
        if !used_alt_texts.contains(character) {
            used_alt_texts.insert(character.to_string());
            return character.to_string();
        }
    }

    "가".to_string()
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};
    use rusqlite::{params, Connection};

    use crate::db::migrations;
    use crate::db::repositories::collections::create_collection;
    use crate::db::repositories::imports::import_image_files;
    use crate::imaging::effects::{EffectRecipe, EffectStep, EFFECT_RECIPE_VERSION};
    use crate::models::{
        ApplyIconCropPayload, ImportImageFilePayload, PreviewIconEffectsPayload,
        UpdateIconEffectsPayload,
    };
    use crate::paths::AppPaths;

    use super::{
        apply_icon_crop, cleanup_previous_effect_preview, completed_effect_preview_requests,
        effect_preview_icon_root, get_icon_editor_state, preview_icon_effects,
        prune_completed_effect_preview_requests, update_icon_effects, OwnedEffectPreviewRequest,
    };

    #[derive(Debug)]
    struct GifSummary {
        repeat: gif::Repeat,
        frame_sizes: Vec<(u16, u16)>,
        delays: Vec<u16>,
    }

    fn connection() -> Connection {
        let mut connection = Connection::open_in_memory().unwrap();
        migrations::run(&mut connection).unwrap();
        connection
    }

    fn temp_paths() -> AppPaths {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        AppPaths::prepare(std::env::temp_dir().join(format!("pmtconcon-editor-{suffix}"))).unwrap()
    }

    fn png_bytes() -> Vec<u8> {
        let image = ImageBuffer::from_pixel(40, 20, Rgba([0, 255, 0, 255]));
        let mut cursor = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image)
            .write_to(&mut cursor, ImageFormat::Png)
            .unwrap();
        cursor.into_inner()
    }

    fn two_color_png_bytes() -> Vec<u8> {
        let image = ImageBuffer::from_fn(6, 2, |x, _| {
            if x < 3 {
                Rgba([255, 0, 0, 255])
            } else {
                Rgba([0, 0, 255, 255])
            }
        });
        let mut cursor = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image)
            .write_to(&mut cursor, ImageFormat::Png)
            .unwrap();
        cursor.into_inner()
    }

    fn animated_gif_bytes(repeat: gif::Repeat) -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut encoder = gif::Encoder::new(&mut bytes, 12, 8, &[]).unwrap();
            encoder.set_repeat(repeat).unwrap();

            for (color, delay) in [([255, 0, 0, 255], 5_u16), ([0, 0, 255, 255], 7_u16)] {
                let mut pixels = Vec::with_capacity(12 * 8 * 4);
                for _ in 0..(12 * 8) {
                    pixels.extend_from_slice(&color);
                }
                let mut frame = gif::Frame::from_rgba_speed(12, 8, &mut pixels, 10);
                frame.delay = delay;
                encoder.write_frame(&frame).unwrap();
            }
        }

        bytes
    }

    fn gif_summary(path: &Path) -> GifSummary {
        let mut options = gif::DecodeOptions::new();
        options.set_color_output(gif::ColorOutput::RGBA);
        let file = std::fs::File::open(path).unwrap();
        let mut reader = options.read_info(file).unwrap();
        let repeat = reader.repeat();
        let mut frame_sizes = Vec::new();
        let mut delays = Vec::new();

        while let Some(frame) = reader.read_next_frame().unwrap() {
            frame_sizes.push((frame.width, frame.height));
            delays.push(frame.delay);
        }

        GifSummary {
            repeat,
            frame_sizes,
            delays,
        }
    }

    #[test]
    fn apply_crop_rejects_extreme_geometry_before_preview_or_db_mutation() {
        let mut connection = connection();
        let paths = temp_paths();
        let collection =
            create_collection(&mut connection, Some("crop validation".to_string())).unwrap();
        let imported = import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![ImportImageFilePayload {
                original_filename: "source.png".to_string(),
                bytes: png_bytes(),
            }],
        )
        .unwrap();
        let icon_id = imported.imported_icons[0].id.clone();
        let before = get_icon_editor_state(&connection, &collection.id, &icon_id).unwrap();
        let preview_dir = paths
            .collection_previews_dir
            .join(&collection.id)
            .join(&icon_id);
        assert!(!preview_dir.exists());

        let payload = |crop_x, crop_w, crop_h, cell_width| ApplyIconCropPayload {
            icon_id: icon_id.clone(),
            shape: "single".to_string(),
            crop_mode: "fixed".to_string(),
            crop_x,
            crop_y: 0.0,
            crop_w,
            crop_h,
            preset_position: "center".to_string(),
            cell_width,
            cell_height: 20,
            transform_quarter_turns: 0,
            transform_flip_horizontal: false,
            transform_flip_vertical: false,
            piece_ids: Vec::new(),
            gif_loop_mode: "preserve".to_string(),
            gif_loop_count: None,
        };

        for invalid in [
            payload(f64::MAX, 20.0, 20.0, 20),
            payload(0.0, f64::MAX, 20.0, 20),
            payload(0.0, 8_000.0, 8_000.0, 20),
            payload(0.0, 20.0, 20.0, i64::MAX),
        ] {
            let error = apply_icon_crop(&mut connection, &paths, &collection.id, invalid)
                .expect_err("extreme crop payload must fail before rendering");
            assert_eq!(error.code, "validation");
            assert!(!preview_dir.exists());
        }

        let after = get_icon_editor_state(&connection, &collection.id, &icon_id).unwrap();
        assert_eq!(before.crop.crop_x, after.crop.crop_x);
        assert_eq!(before.crop.crop_y, after.crop.crop_y);
        assert_eq!(before.crop.crop_w, after.crop.crop_w);
        assert_eq!(before.crop.crop_h, after.crop.crop_h);

        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn apply_crop_updates_metadata_and_generates_preview_without_touching_original() {
        let mut connection = connection();
        let paths = temp_paths();
        let collection =
            create_collection(&mut connection, Some("편집 테스트".to_string())).unwrap();
        let imported = import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![ImportImageFilePayload {
                original_filename: "source.png".to_string(),
                bytes: png_bytes(),
            }],
        )
        .unwrap();
        let icon_id = imported.imported_icons[0].id.clone();
        let original_path: String = connection
            .query_row(
                "SELECT s.original_path_in_library
                 FROM source_files s
                 JOIN icons i ON i.source_file_id = s.id
                 WHERE i.id = ?1",
                [&icon_id],
                |row| row.get(0),
            )
            .unwrap();
        assert!(std::path::Path::new(&original_path).exists());

        let updated = apply_icon_crop(
            &mut connection,
            &paths,
            &collection.id,
            ApplyIconCropPayload {
                icon_id: icon_id.clone(),
                shape: "horizontal_double".to_string(),
                crop_mode: "fixed".to_string(),
                crop_x: 0.0,
                crop_y: 0.0,
                crop_w: 40.0,
                crop_h: 20.0,
                preset_position: "center".to_string(),
                cell_width: 20,
                cell_height: 20,
                transform_quarter_turns: 0,
                transform_flip_horizontal: false,
                transform_flip_vertical: false,
                piece_ids: Vec::new(),
                gif_loop_mode: "preserve".to_string(),
                gif_loop_count: None,
            },
        )
        .unwrap();

        assert_eq!(updated.shape, "horizontal_double");
        assert_eq!(updated.pieces.len(), 2);
        assert_eq!(updated.pieces[0].piece_role, "left");
        assert_eq!(updated.pieces[1].piece_role, "right");
        assert!(updated.current_preview_url.is_some());
        assert!(std::path::Path::new(updated.current_preview_url.as_ref().unwrap()).exists());
        assert!(std::path::Path::new(&original_path).exists());

        let state = get_icon_editor_state(&connection, &collection.id, &icon_id).unwrap();
        assert_eq!(state.crop.crop_mode, "fixed");
        assert_eq!(state.crop.viewport_width_at_apply, 40);
        assert_eq!(state.crop.viewport_height_at_apply, 20);

        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn apply_gif_crop_generates_animated_preview_with_loop_metadata() {
        let mut connection = connection();
        let paths = temp_paths();
        let collection =
            create_collection(&mut connection, Some("GIF 편집 테스트".to_string())).unwrap();
        let source_bytes = animated_gif_bytes(gif::Repeat::Infinite);
        let imported = import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![ImportImageFilePayload {
                original_filename: "source.gif".to_string(),
                bytes: source_bytes.clone(),
            }],
        )
        .unwrap();
        let icon_id = imported.imported_icons[0].id.clone();
        let original_path: String = connection
            .query_row(
                "SELECT s.original_path_in_library
                 FROM source_files s
                 JOIN icons i ON i.source_file_id = s.id
                 WHERE i.id = ?1",
                [&icon_id],
                |row| row.get(0),
            )
            .unwrap();

        let updated = apply_icon_crop(
            &mut connection,
            &paths,
            &collection.id,
            ApplyIconCropPayload {
                icon_id: icon_id.clone(),
                shape: "horizontal_double".to_string(),
                crop_mode: "fixed".to_string(),
                crop_x: 0.0,
                crop_y: 0.0,
                crop_w: 12.0,
                crop_h: 8.0,
                preset_position: "center".to_string(),
                cell_width: 6,
                cell_height: 8,
                transform_quarter_turns: 0,
                transform_flip_horizontal: false,
                transform_flip_vertical: false,
                piece_ids: Vec::new(),
                gif_loop_mode: "count".to_string(),
                gif_loop_count: Some(2),
            },
        )
        .unwrap();

        let preview_path = Path::new(updated.current_preview_url.as_ref().unwrap());
        let preview = gif_summary(preview_path);
        assert_eq!(preview.repeat, gif::Repeat::Finite(2));
        assert_eq!(preview.frame_sizes, vec![(12, 8), (12, 8)]);
        assert_eq!(preview.delays, vec![5, 7]);

        let piece_path: String = connection
            .query_row(
                "SELECT generated_preview_path
                 FROM icon_pieces
                 WHERE icon_id = ?1
                   AND piece_index = 0",
                [&icon_id],
                |row| row.get(0),
            )
            .unwrap();
        let piece = gif_summary(Path::new(&piece_path));
        assert_eq!(piece.repeat, gif::Repeat::Finite(2));
        assert_eq!(piece.frame_sizes, vec![(6, 8), (6, 8)]);

        assert_eq!(std::fs::read(original_path).unwrap(), source_bytes);

        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn quarter_turn_persists_non_square_geometry_and_moves_piece_identity_with_content() {
        let mut connection = connection();
        let paths = temp_paths();
        let collection =
            create_collection(&mut connection, Some("변형 테스트".to_string())).unwrap();
        let source_bytes = two_color_png_bytes();
        let imported = import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![ImportImageFilePayload {
                original_filename: "two-colors.png".to_string(),
                bytes: source_bytes.clone(),
            }],
        )
        .unwrap();
        let icon_id = imported.imported_icons[0].id.clone();

        let horizontal = apply_icon_crop(
            &mut connection,
            &paths,
            &collection.id,
            ApplyIconCropPayload {
                icon_id: icon_id.clone(),
                shape: "horizontal_double".to_string(),
                crop_mode: "fixed".to_string(),
                crop_x: 0.0,
                crop_y: 0.0,
                crop_w: 6.0,
                crop_h: 2.0,
                preset_position: "center".to_string(),
                cell_width: 3,
                cell_height: 2,
                transform_quarter_turns: 0,
                transform_flip_horizontal: false,
                transform_flip_vertical: false,
                piece_ids: Vec::new(),
                gif_loop_mode: "preserve".to_string(),
                gif_loop_count: None,
            },
        )
        .unwrap();
        let left_piece_id = horizontal.pieces[0].id.clone();
        let right_piece_id = horizontal.pieces[1].id.clone();
        connection
            .execute(
                "UPDATE icon_pieces SET alt_text = 'A' WHERE id = ?1",
                [&left_piece_id],
            )
            .unwrap();
        connection
            .execute(
                "UPDATE icon_pieces SET alt_text = 'B' WHERE id = ?1",
                [&right_piece_id],
            )
            .unwrap();

        let rotated = apply_icon_crop(
            &mut connection,
            &paths,
            &collection.id,
            ApplyIconCropPayload {
                icon_id: icon_id.clone(),
                shape: "vertical_double".to_string(),
                crop_mode: "fixed".to_string(),
                crop_x: 0.0,
                crop_y: 0.0,
                crop_w: 6.0,
                crop_h: 2.0,
                preset_position: "center".to_string(),
                cell_width: 2,
                cell_height: 3,
                transform_quarter_turns: 3,
                transform_flip_horizontal: false,
                transform_flip_vertical: false,
                piece_ids: vec![right_piece_id.clone(), left_piece_id.clone()],
                gif_loop_mode: "preserve".to_string(),
                gif_loop_count: None,
            },
        )
        .unwrap();

        assert_eq!(rotated.transform_quarter_turns, 3);
        assert!(!rotated.transform_flip_horizontal);
        assert!(!rotated.transform_flip_vertical);
        assert_eq!(rotated.shape, "vertical_double");
        assert_eq!(rotated.cell_width_override, Some(2));
        assert_eq!(rotated.cell_height_override, Some(3));
        assert_eq!(rotated.pieces[0].id, right_piece_id);
        assert_eq!(rotated.pieces[0].piece_role, "top");
        assert_eq!(rotated.pieces[0].alt_text, "B");
        assert_eq!(rotated.pieces[1].id, left_piece_id);
        assert_eq!(rotated.pieces[1].piece_role, "bottom");
        assert_eq!(rotated.pieces[1].alt_text, "A");

        let preview = image::open(rotated.current_preview_url.as_ref().unwrap())
            .unwrap()
            .to_rgba8();
        assert_eq!((preview.width(), preview.height()), (2, 6));
        assert_eq!(preview.get_pixel(0, 0).0, [0, 0, 255, 255]);
        assert_eq!(preview.get_pixel(0, 5).0, [255, 0, 0, 255]);

        let state = get_icon_editor_state(&connection, &collection.id, &icon_id).unwrap();
        assert_eq!(state.crop.viewport_width_at_apply, 6);
        assert_eq!(state.crop.viewport_height_at_apply, 2);
        assert_eq!(state.icon.transform_quarter_turns, 3);

        let original_path: String = connection
            .query_row(
                "SELECT s.original_path_in_library
                 FROM source_files s
                 JOIN icons i ON i.source_file_id = s.id
                 WHERE i.id = ?1",
                [&icon_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(std::fs::read(original_path).unwrap(), source_bytes);

        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn quarter_turn_is_applied_to_every_gif_frame_without_losing_timing() {
        let mut connection = connection();
        let paths = temp_paths();
        let collection =
            create_collection(&mut connection, Some("GIF 변형 테스트".to_string())).unwrap();
        let source_bytes = animated_gif_bytes(gif::Repeat::Infinite);
        let imported = import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![ImportImageFilePayload {
                original_filename: "animated.gif".to_string(),
                bytes: source_bytes.clone(),
            }],
        )
        .unwrap();
        let icon_id = imported.imported_icons[0].id.clone();
        let piece_id = imported.imported_icons[0].pieces[0].id.clone();

        let rotated = apply_icon_crop(
            &mut connection,
            &paths,
            &collection.id,
            ApplyIconCropPayload {
                icon_id,
                shape: "single".to_string(),
                crop_mode: "fixed".to_string(),
                crop_x: 0.0,
                crop_y: 0.0,
                crop_w: 12.0,
                crop_h: 8.0,
                preset_position: "center".to_string(),
                cell_width: 8,
                cell_height: 12,
                transform_quarter_turns: 1,
                transform_flip_horizontal: false,
                transform_flip_vertical: false,
                piece_ids: vec![piece_id],
                gif_loop_mode: "preserve".to_string(),
                gif_loop_count: None,
            },
        )
        .unwrap();

        let summary = gif_summary(Path::new(rotated.current_preview_url.as_ref().unwrap()));
        assert_eq!(summary.repeat, gif::Repeat::Infinite);
        assert_eq!(summary.frame_sizes, vec![(8, 12), (8, 12)]);
        assert_eq!(summary.delays, vec![5, 7]);

        let original_path: String = connection
            .query_row(
                "SELECT s.original_path_in_library
                 FROM source_files s
                 JOIN icons i ON i.source_file_id = s.id
                 WHERE i.id = ?1",
                [&rotated.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(std::fs::read(original_path).unwrap(), source_bytes);

        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn effect_preview_is_non_destructive_and_save_uses_revision_conflicts() {
        let mut connection = connection();
        let paths = temp_paths();
        let collection =
            create_collection(&mut connection, Some("효과 저장 테스트".to_string())).unwrap();
        let source_bytes = two_color_png_bytes();
        let imported = import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![ImportImageFilePayload {
                original_filename: "effect-source.png".to_string(),
                bytes: source_bytes.clone(),
            }],
        )
        .unwrap();
        let icon_id = imported.imported_icons[0].id.clone();
        let original_path: String = connection
            .query_row(
                "SELECT s.original_path_in_library
                 FROM source_files s
                 JOIN icons i ON i.source_file_id = s.id
                 WHERE i.id = ?1",
                [&icon_id],
                |row| row.get(0),
            )
            .unwrap();
        let initial = get_icon_editor_state(&connection, &collection.id, &icon_id).unwrap();
        let initial_preview = initial.icon.current_preview_url.clone();
        assert_eq!(initial.effect_revision, 0);
        assert!(initial.effect_recipe.effects.is_empty());

        let recipe = EffectRecipe {
            version: EFFECT_RECIPE_VERSION,
            effects: vec![EffectStep::Pixelate {
                id: "pixelate-1".to_string(),
                enabled: true,
                block_size: 2,
            }],
        };
        let draft = preview_icon_effects(
            &connection,
            &paths,
            &collection.id,
            PreviewIconEffectsPayload {
                icon_id: icon_id.clone(),
                recipe: recipe.clone(),
            },
        )
        .unwrap();
        let same_draft = preview_icon_effects(
            &connection,
            &paths,
            &collection.id,
            PreviewIconEffectsPayload {
                icon_id: icon_id.clone(),
                recipe: recipe.clone(),
            },
        )
        .unwrap();
        assert_ne!(draft.preview_path, same_draft.preview_path);
        assert!(Path::new(&draft.preview_path).is_file());
        assert!(Path::new(&same_draft.preview_path).is_file());
        assert!(Path::new(&draft.preview_path).exists());
        let after_draft = get_icon_editor_state(&connection, &collection.id, &icon_id).unwrap();
        assert_eq!(after_draft.effect_revision, 0);
        assert_eq!(after_draft.icon.current_preview_url, initial_preview);

        let saved = update_icon_effects(
            &mut connection,
            &paths,
            &collection.id,
            UpdateIconEffectsPayload {
                icon_id: icon_id.clone(),
                expected_revision: 0,
                recipe: recipe.clone(),
            },
        )
        .unwrap();
        assert_eq!(saved.effect_revision, 1);
        assert_eq!(saved.effect_recipe, recipe);
        assert!(Path::new(saved.icon.current_preview_url.as_ref().unwrap()).exists());
        assert_eq!(std::fs::read(&original_path).unwrap(), source_bytes);

        let saved_preview_path = saved.icon.current_preview_url.clone().unwrap();
        let same_recipe_conflict = update_icon_effects(
            &mut connection,
            &paths,
            &collection.id,
            UpdateIconEffectsPayload {
                icon_id: icon_id.clone(),
                expected_revision: 0,
                recipe: recipe.clone(),
            },
        )
        .unwrap_err();
        assert_eq!(same_recipe_conflict.code, "conflict");
        assert!(
            Path::new(&saved_preview_path).exists(),
            "a conflicting save must never remove the successful save artifact"
        );

        let stale_recipe = EffectRecipe {
            version: EFFECT_RECIPE_VERSION,
            effects: vec![EffectStep::Blur {
                id: "blur-stale".to_string(),
                enabled: true,
                radius: 1,
            }],
        };
        let stale_error = update_icon_effects(
            &mut connection,
            &paths,
            &collection.id,
            UpdateIconEffectsPayload {
                icon_id: icon_id.clone(),
                expected_revision: 0,
                recipe: stale_recipe,
            },
        )
        .unwrap_err();
        assert_eq!(stale_error.code, "conflict");

        let after_conflict = get_icon_editor_state(&connection, &collection.id, &icon_id).unwrap();
        assert_eq!(after_conflict.effect_revision, 1);
        assert_eq!(after_conflict.effect_recipe, saved.effect_recipe);
        assert_eq!(
            after_conflict.icon.current_preview_url,
            saved.icon.current_preview_url
        );
        assert_eq!(std::fs::read(original_path).unwrap(), source_bytes);

        let effect_root = paths
            .collection_previews_dir
            .join(&collection.id)
            .join(&icon_id)
            .join("effects");
        let staging_root = effect_root.join(".staging");
        assert_eq!(std::fs::read_dir(&staging_root).unwrap().count(), 0);
        let final_dirs = std::fs::read_dir(&effect_root)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| {
                path.is_dir() && path.file_name().and_then(|name| name.to_str()) != Some(".staging")
            })
            .collect::<Vec<_>>();
        assert_eq!(final_dirs.len(), 1);
        assert_eq!(
            final_dirs[0].canonicalize().unwrap(),
            Path::new(&saved_preview_path)
                .parent()
                .unwrap()
                .canonicalize()
                .unwrap()
        );

        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn effect_save_does_not_remove_a_legacy_artifact_referenced_by_another_icon() {
        let mut connection = connection();
        let paths = temp_paths();
        let collection =
            create_collection(&mut connection, Some("효과 공유 경로 테스트".to_string())).unwrap();
        let imported = import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![
                ImportImageFilePayload {
                    original_filename: "owner.png".to_string(),
                    bytes: two_color_png_bytes(),
                },
                ImportImageFilePayload {
                    original_filename: "legacy-clone.png".to_string(),
                    bytes: png_bytes(),
                },
            ],
        )
        .unwrap();
        let owner_icon_id = imported.imported_icons[0].id.clone();
        let clone_icon_id = imported.imported_icons[1].id.clone();
        let clone_piece_id = imported.imported_icons[1].pieces[0].id.clone();
        let first_recipe = EffectRecipe {
            version: EFFECT_RECIPE_VERSION,
            effects: vec![EffectStep::Pixelate {
                id: "pixelate-owner".to_string(),
                enabled: true,
                block_size: 2,
            }],
        };
        let first_save = update_icon_effects(
            &mut connection,
            &paths,
            &collection.id,
            UpdateIconEffectsPayload {
                icon_id: owner_icon_id.clone(),
                expected_revision: 0,
                recipe: first_recipe,
            },
        )
        .unwrap();
        let shared_preview = first_save.icon.current_preview_url.clone().unwrap();
        let shared_piece: String = connection
            .query_row(
                "SELECT generated_preview_path
                 FROM icon_pieces
                 WHERE icon_id = ?1
                   AND piece_index = 0",
                [&owner_icon_id],
                |row| row.get(0),
            )
            .unwrap();
        connection
            .execute(
                "UPDATE icons SET current_preview_path = ?1 WHERE id = ?2",
                params![shared_preview, clone_icon_id],
            )
            .unwrap();
        connection
            .execute(
                "UPDATE icon_pieces SET generated_preview_path = ?1 WHERE id = ?2",
                params![shared_piece, clone_piece_id],
            )
            .unwrap();

        let second_save = update_icon_effects(
            &mut connection,
            &paths,
            &collection.id,
            UpdateIconEffectsPayload {
                icon_id: owner_icon_id.clone(),
                expected_revision: 1,
                recipe: EffectRecipe {
                    version: EFFECT_RECIPE_VERSION,
                    effects: vec![EffectStep::Blur {
                        id: "blur-owner".to_string(),
                        enabled: true,
                        radius: 1,
                    }],
                },
            },
        )
        .unwrap();
        assert!(Path::new(&shared_preview).exists());
        assert!(Path::new(&shared_piece).exists());

        let effect_root = paths
            .collection_previews_dir
            .join(&collection.id)
            .join(&owner_icon_id)
            .join("effects");
        let current_preview = second_save.icon.current_preview_url.unwrap();

        connection
            .execute(
                "UPDATE icons SET current_preview_path = NULL WHERE id = ?1",
                [&clone_icon_id],
            )
            .unwrap();
        cleanup_previous_effect_preview(
            &connection,
            &effect_root,
            Some(&shared_preview),
            Path::new(&current_preview).parent().unwrap(),
        );
        assert!(Path::new(&shared_preview).exists());
        assert!(Path::new(&shared_piece).exists());

        connection
            .execute(
                "UPDATE icons SET current_preview_path = ?1 WHERE id = ?2",
                params![shared_preview, clone_icon_id],
            )
            .unwrap();
        connection
            .execute(
                "UPDATE icon_pieces SET generated_preview_path = NULL WHERE id = ?1",
                [&clone_piece_id],
            )
            .unwrap();
        cleanup_previous_effect_preview(
            &connection,
            &effect_root,
            Some(&shared_preview),
            Path::new(&current_preview).parent().unwrap(),
        );
        assert!(Path::new(&shared_preview).exists());
        assert!(Path::new(&shared_piece).exists());

        connection
            .execute(
                "UPDATE icons SET current_preview_path = NULL WHERE id = ?1",
                [&clone_icon_id],
            )
            .unwrap();
        cleanup_previous_effect_preview(
            &connection,
            &effect_root,
            Some(&shared_preview),
            Path::new(&current_preview).parent().unwrap(),
        );
        assert!(!Path::new(&shared_preview).exists());

        std::fs::remove_dir_all(paths.root).unwrap();
        assert!(!Path::new(&shared_piece).exists());
    }

    #[test]
    fn completed_effect_preview_pruning_keeps_eight_per_icon_and_skips_in_progress() {
        let paths = temp_paths();
        let icon_id = "icon_preview_pruning";
        let mut latest_completed = None;

        for index in 0..12 {
            let signature = format!("{:064x}", index % 3);
            let mut request =
                OwnedEffectPreviewRequest::create(&paths, icon_id, &signature).unwrap();
            std::fs::write(request.directory().join("preview.png"), [index as u8]).unwrap();
            latest_completed = Some(request.directory().to_path_buf());
            request.mark_completed().unwrap();
            prune_completed_effect_preview_requests(request.icon_root(), request.directory());
        }

        let icon_root = effect_preview_icon_root(&paths, icon_id).unwrap();
        let completed = completed_effect_preview_requests(&icon_root);
        assert_eq!(completed.len(), 8);
        assert!(latest_completed.unwrap().is_dir());

        let in_progress_signature = "f".repeat(64);
        let in_progress =
            OwnedEffectPreviewRequest::create(&paths, icon_id, &in_progress_signature).unwrap();
        let in_progress_path = in_progress.directory().to_path_buf();
        let current = completed[0].path.clone();
        prune_completed_effect_preview_requests(&icon_root, &current);
        assert_eq!(completed_effect_preview_requests(&icon_root).len(), 8);
        assert!(in_progress_path.is_dir());
        drop(in_progress);
        assert!(!in_progress_path.exists());

        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn effect_preview_paths_reject_traversal_and_never_follow_directory_symlinks() {
        let paths = temp_paths();
        let traversal = effect_preview_icon_root(&paths, "../escaped").unwrap_err();
        assert_eq!(traversal.code, "effect_preview_path");
        assert!(!paths
            .temp_export_dir
            .parent()
            .unwrap()
            .join("escaped")
            .exists());

        let icon_root = effect_preview_icon_root(&paths, "icon_symlink_guard").unwrap();
        let outside = paths.root.join("outside-preview-target");
        let outside_request = outside.join("fxpreview_external_00000001");
        std::fs::create_dir_all(&outside_request).unwrap();
        std::fs::write(outside_request.join(".complete"), []).unwrap();
        let sentinel = outside_request.join("sentinel.txt");
        std::fs::write(&sentinel, b"keep").unwrap();
        let linked_signature = icon_root.join("e".repeat(64));

        #[cfg(windows)]
        let link_result = std::os::windows::fs::symlink_dir(&outside, &linked_signature);
        #[cfg(unix)]
        let link_result = std::os::unix::fs::symlink(&outside, &linked_signature);
        #[cfg(not(any(windows, unix)))]
        let link_result: std::io::Result<()> = Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "directory symlinks are not supported on this target",
        ));

        if let Err(error) = link_result {
            eprintln!("directory symlink assertion skipped: {error}");
            std::fs::remove_dir_all(paths.root).unwrap();
            return;
        }

        prune_completed_effect_preview_requests(&icon_root, &icon_root.join("not-a-request"));
        assert!(sentinel.is_file());
        assert!(outside_request.is_dir());

        #[cfg(windows)]
        std::fs::remove_dir(&linked_signature).unwrap();
        #[cfg(unix)]
        std::fs::remove_file(&linked_signature).unwrap();
        std::fs::remove_dir_all(paths.root).unwrap();
    }
}
