use std::collections::HashSet;
use std::fmt::Write as FmtWrite;
use std::fs;
use std::io::BufReader;
use std::path::Path;

use image::codecs::gif::GifDecoder;
use image::imageops::{self, FilterType};
use image::{AnimationDecoder, Rgba, RgbaImage};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::db::repositories::ai as ai_repository;
use crate::error::{AppError, AppResult};
use crate::imaging::effects::{apply_effect_recipe, parse_effect_recipe_json, EffectRecipe};
use crate::imaging::export_render::ExportCropRect;
use crate::imaging::import_limits::{
    validate_crop_rect, validate_import_dimensions, ValidatedCropRect, MAX_GIF_TOTAL_FRAME_PIXELS,
};
use crate::imaging::motion::{
    apply_motion_recipe, parse_motion_recipe_json, MotionFrameContext, MotionRecipe,
};
use crate::imaging::text_overlay::{
    apply_text_overlay, text_overlay_from_fields, TextOverlayRenderSpec,
};
use crate::imaging::transform::{apply_image_transform, source_viewport_geometry, ImageTransform};
use crate::optimization::cache::{hash_text, render_recipe_crop_hash};

use super::grid::{cells_for_settings, PageCellPlacement, SheetGridSettings};
use super::importer::png_bytes_from_rgba;
use super::manifest::ManifestVisualSource;

pub(crate) const AI_GRID_SCHEMA: &str = "pmtcon-ai-grid-v1";
pub(crate) const MIN_AI_GRID_ITEMS: usize = 2;
pub(crate) const MAX_AI_GRID_ITEMS: usize = 16;
pub(crate) const MAX_AI_GRID_AXIS: i64 = 4;
pub(crate) const MAX_AI_GRID_CANVAS_SIDE: u32 = 2_048;
pub(crate) const MAX_AI_GRID_CANVAS_PIXELS: u64 = 4_194_304;
pub(crate) const MAX_AI_GRID_INPUT_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuideLabelOptions {
    #[serde(default = "default_true")]
    pub cell_number: bool,
    #[serde(default)]
    pub icon_name: bool,
    #[serde(default)]
    pub alt_value: bool,
    #[serde(default)]
    pub export_number: bool,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum StaticRenderSelection<'a> {
    LegacyWorkSheet {
        source: &'a str,
        selected_icon_ids: &'a [String],
    },
    StrictAiEdit {
        selected_icon_ids: &'a [String],
    },
}

#[derive(Debug)]
pub(crate) struct IconRecord {
    pub id: String,
    pub display_name: String,
    pub shape: String,
    original_source_file_id: String,
    original_source_hash: String,
    original_lineage_id: String,
    original_lineage_generation: i64,
    effective_source_file_id: String,
    effective_source_hash: String,
    source_path: String,
    source_extension: String,
    source_mime_type: String,
    source_hash: String,
    source_width: i64,
    source_height: i64,
    pub source_is_animated: bool,
    configured_cell_width: i64,
    configured_cell_height: i64,
    crop_x: f64,
    crop_y: f64,
    crop_w: f64,
    crop_h: f64,
    transform: ImageTransform,
    text_overlay: Option<TextOverlayRenderSpec>,
    effects: EffectRecipe,
    pub motion: MotionRecipe,
}

#[derive(Debug)]
struct PieceRecord {
    id: String,
    piece_index: i64,
    alt_text: String,
}

#[derive(Debug)]
pub(crate) struct RenderedSheetItem {
    pub icon_id: String,
    pub piece_id: Option<String>,
    pub display_name: String,
    pub alt: String,
    pub icon_type: String,
    pub source_hash: Option<String>,
    pub visual_source: ManifestVisualSource,
    pub render_hash: String,
    pub render_recipe_hash: String,
    image: RgbaImage,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AiGridRect {
    pub x: i64,
    pub y: i64,
    pub width: i64,
    pub height: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AiGridLayout {
    pub canvas_width: i64,
    pub canvas_height: i64,
    pub rows: i64,
    pub columns: i64,
    pub cell_size: i64,
    pub gap_x: i64,
    pub gap_y: i64,
    pub border_left: i64,
    pub border_top: i64,
    pub border_right: i64,
    pub border_bottom: i64,
}

#[derive(Debug, Clone)]
pub(crate) struct ComposeAiEditGridRequest<'a> {
    pub collection_id: &'a str,
    pub selected_icon_ids: &'a [String],
    pub layout: AiGridLayout,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AiGridTargetSnapshot {
    pub item_index: i64,
    pub origin_icon_id: String,
    pub target_name_snapshot: String,
    pub shape: String,
    pub target_cell_width: i64,
    pub target_cell_height: i64,
    pub original_source_file_id: String,
    pub original_lineage_id: String,
    pub original_lineage_generation: i64,
    pub original_source_sha256: String,
    pub effective_source_file_id: String,
    pub effective_source_sha256: String,
    pub activation_revision: i64,
    pub native_recipe_signature: String,
    pub input_render_recipe_hash: String,
    pub input_render_sha256: String,
    pub input_rect: AiGridRect,
}

#[derive(Debug, Clone)]
pub(crate) struct ComposedAiGrid {
    pub png_bytes: Vec<u8>,
    pub png_sha256: String,
    pub manifest_json: String,
    pub manifest_sha256: String,
    pub layout: AiGridLayout,
    pub items: Vec<AiGridTargetSnapshot>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AiGridManifest<'a> {
    schema: &'static str,
    kind: &'static str,
    input_sheet_sha256: &'a str,
    layout: &'a AiGridLayout,
    items: &'a [AiGridTargetSnapshot],
}

pub(crate) fn default_ai_grid_layout(
    item_count: usize,
    canvas_size: i64,
) -> AppResult<AiGridLayout> {
    validate_ai_grid_item_count(item_count)?;
    let (rows, columns) = match item_count {
        2 => (1_i64, 2_i64),
        3..=4 => (2, 2),
        5..=9 => (3, 3),
        _ => (4, 4),
    };
    let gap = 16_i64;
    let dominant = rows.max(columns);
    let gap_extent = (dominant - 1)
        .checked_mul(gap)
        .ok_or_else(|| ai_grid_layout_error("grid 간격 계산이 지원 범위를 벗어났습니다."))?;
    let cell_size = canvas_size
        .checked_sub(gap_extent)
        .map(|available| available / dominant)
        .filter(|cell| *cell > 0)
        .ok_or_else(|| ai_grid_layout_error("canvas가 grid 배치보다 작습니다."))?;
    let horizontal_extent = axis_extent(columns, cell_size, gap)?;
    let vertical_extent = axis_extent(rows, cell_size, gap)?;
    let horizontal_remainder = canvas_size
        .checked_sub(horizontal_extent)
        .ok_or_else(|| ai_grid_layout_error("grid 가로 배치가 canvas를 벗어납니다."))?;
    let vertical_remainder = canvas_size
        .checked_sub(vertical_extent)
        .ok_or_else(|| ai_grid_layout_error("grid 세로 배치가 canvas를 벗어납니다."))?;
    let border_left = horizontal_remainder / 2;
    let border_top = vertical_remainder / 2;
    let layout = AiGridLayout {
        canvas_width: canvas_size,
        canvas_height: canvas_size,
        rows,
        columns,
        cell_size,
        gap_x: gap,
        gap_y: gap,
        border_left,
        border_top,
        border_right: horizontal_remainder - border_left,
        border_bottom: vertical_remainder - border_top,
    };
    validate_ai_grid_layout(&layout, item_count)?;
    Ok(layout)
}

pub(crate) fn default_ai_generation_layout(
    item_count: usize,
    canvas_size: i64,
) -> AppResult<AiGridLayout> {
    if item_count != 1 {
        return default_ai_grid_layout(item_count, canvas_size);
    }
    let side = u32::try_from(canvas_size)
        .map_err(|_| ai_grid_layout_error("AI 생성 canvas 크기가 올바르지 않습니다."))?;
    if side == 0 || side > MAX_AI_GRID_CANVAS_SIDE {
        return Err(ai_grid_layout_error(
            "AI 생성 canvas는 한 변이 1~2048px인 정사각형이어야 합니다.",
        ));
    }
    let pixels = u64::from(side)
        .checked_mul(u64::from(side))
        .ok_or_else(|| ai_grid_layout_error("AI 생성 canvas 픽셀 수가 너무 큽니다."))?;
    if pixels > MAX_AI_GRID_CANVAS_PIXELS {
        return Err(ai_grid_layout_error(
            "AI 생성 canvas의 전체 픽셀 수가 지원 범위를 벗어났습니다.",
        ));
    }
    Ok(AiGridLayout {
        canvas_width: canvas_size,
        canvas_height: canvas_size,
        rows: 1,
        columns: 1,
        cell_size: canvas_size,
        gap_x: 0,
        gap_y: 0,
        border_left: 0,
        border_top: 0,
        border_right: 0,
        border_bottom: 0,
    })
}

pub(crate) fn compose_ai_edit_grid(
    connection: &Connection,
    request: ComposeAiEditGridRequest<'_>,
) -> AppResult<ComposedAiGrid> {
    validate_ai_grid_item_count(request.selected_icon_ids.len())?;
    let cells = resolved_ai_grid_cells(&request.layout, request.selected_icon_ids.len())?;
    let icons = load_static_render_targets(
        connection,
        request.collection_id,
        StaticRenderSelection::StrictAiEdit {
            selected_icon_ids: request.selected_icon_ids,
        },
    )?;

    let aggregate_source_pixels = icons.iter().try_fold(0_u64, |total, icon| {
        let pixels = u64::try_from(icon.source_width)
            .ok()
            .and_then(|width| {
                u64::try_from(icon.source_height)
                    .ok()
                    .and_then(|height| width.checked_mul(height))
            })
            .ok_or_else(|| {
                AppError::new(
                    "ai_grid_source_workload",
                    "선택한 원본 이미지의 픽셀 수를 계산할 수 없습니다.",
                )
            })?;
        total.checked_add(pixels).ok_or_else(|| {
            AppError::new(
                "ai_grid_source_workload",
                "선택한 원본 이미지의 전체 픽셀 수가 너무 큽니다.",
            )
        })
    })?;
    if aggregate_source_pixels > MAX_GIF_TOTAL_FRAME_PIXELS {
        return Err(AppError::new(
            "ai_grid_source_workload",
            "선택한 원본 이미지의 전체 픽셀 수가 지원 범위를 벗어났습니다.",
        ));
    }

    let mut rendered_items = Vec::with_capacity(icons.len());
    let mut snapshots = Vec::with_capacity(icons.len());
    let mut placements = Vec::with_capacity(icons.len());
    for (item_index, icon) in icons.iter().enumerate() {
        let mut rendered = render_icon_items(
            connection,
            icon,
            request.layout.cell_size,
            request.layout.cell_size,
        )?;
        if rendered.len() != 1 {
            return Err(AppError::new(
                "ai_grid_shape_unsupported",
                "AI grid에는 정적 단일 아이콘만 넣을 수 있습니다.",
            ));
        }
        let item = rendered.remove(0);
        let cell = &cells[item_index];
        let piece_id = item.piece_id.as_deref().ok_or_else(|| {
            AppError::new(
                "ai_grid_piece_state",
                "AI grid 대상 아이콘의 단일 조각 정보를 확인할 수 없습니다.",
            )
        })?;
        let (current_source_hash, current_render_recipe_hash) = current_static_sheet_render_guard(
            connection,
            request.collection_id,
            &icon.id,
            Some(piece_id),
            request.layout.cell_size,
            request.layout.cell_size,
        )?;
        if item.source_hash.as_deref() != Some(current_source_hash.as_str())
            || item.render_recipe_hash != current_render_recipe_hash
        {
            return Err(AppError::new(
                "ai_grid_target_stale",
                "AI grid를 준비하는 동안 대상 아이콘의 원본 또는 편집값이 변경되었습니다.",
            ));
        }
        let current = ai_repository::resolve_effective_visual_source(
            connection,
            request.collection_id,
            &icon.id,
        )?;
        if current.render_source.sha256 != icon.effective_source_hash
            || current.original_source.sha256 != icon.original_source_hash
            || current.original_lineage_id != icon.original_lineage_id
            || current.original_lineage_generation != icon.original_lineage_generation
        {
            return Err(AppError::new(
                "ai_grid_target_stale",
                "AI grid를 준비하는 동안 대상 아이콘의 source 계보가 변경되었습니다.",
            ));
        }
        let native_recipe_signature =
            ai_repository::get_ai_review_state(connection, request.collection_id, &icon.id)?
                .native_recipe_signature;
        let input_rect = AiGridRect {
            x: cell.x,
            y: cell.y,
            width: cell.w,
            height: cell.h,
        };
        snapshots.push(AiGridTargetSnapshot {
            item_index: i64::try_from(item_index).map_err(|_| {
                AppError::new(
                    "ai_grid_item_count",
                    "AI grid 항목 번호가 올바르지 않습니다.",
                )
            })?,
            origin_icon_id: icon.id.clone(),
            target_name_snapshot: icon.display_name.clone(),
            shape: icon.shape.clone(),
            target_cell_width: icon.configured_cell_width,
            target_cell_height: icon.configured_cell_height,
            original_source_file_id: icon.original_source_file_id.clone(),
            original_lineage_id: icon.original_lineage_id.clone(),
            original_lineage_generation: icon.original_lineage_generation,
            original_source_sha256: icon.original_source_hash.clone(),
            effective_source_file_id: icon.effective_source_file_id.clone(),
            effective_source_sha256: icon.effective_source_hash.clone(),
            activation_revision: current.activation_revision,
            native_recipe_signature,
            input_render_recipe_hash: item.render_recipe_hash.clone(),
            input_render_sha256: item.render_hash.clone(),
            input_rect,
        });
        placements.push(PageCellPlacement {
            item_index,
            page_index: 0,
            row: cell.row,
            col: cell.col,
            x: cell.x,
            y: cell.y,
            w: cell.w,
            h: cell.h,
        });
        rendered_items.push(item);
    }

    let placement_refs = placements.iter().collect::<Vec<_>>();
    let sheet = render_sheet_page(
        &rendered_items,
        &placement_refs,
        request.layout.canvas_width,
        request.layout.canvas_height,
        "transparent",
        false,
        None,
    )?;
    let png_bytes = png_bytes_from_rgba(&sheet)?;
    if png_bytes.len() > MAX_AI_GRID_INPUT_BYTES {
        return Err(AppError::new(
            "ai_grid_input_too_large",
            "AI grid 입력 PNG는 최대 16MB까지 만들 수 있습니다.",
        ));
    }
    let png_sha256 = sha256_hex(&png_bytes);
    let manifest_json = serde_json::to_string(&AiGridManifest {
        schema: AI_GRID_SCHEMA,
        kind: "selected_icon_edit",
        input_sheet_sha256: &png_sha256,
        layout: &request.layout,
        items: &snapshots,
    })
    .map_err(|error| AppError::new("ai_grid_manifest", error.to_string()))?;
    let manifest_sha256 = sha256_hex(manifest_json.as_bytes());

    Ok(ComposedAiGrid {
        png_bytes,
        png_sha256,
        manifest_json,
        manifest_sha256,
        layout: request.layout,
        items: snapshots,
    })
}

pub(crate) fn load_static_render_targets(
    connection: &Connection,
    collection_id: &str,
    selection: StaticRenderSelection<'_>,
) -> AppResult<Vec<IconRecord>> {
    let (selected_icon_ids, strict, selected_source) = match selection {
        StaticRenderSelection::LegacyWorkSheet {
            source,
            selected_icon_ids,
        } => (selected_icon_ids, false, Some(source)),
        StaticRenderSelection::StrictAiEdit { selected_icon_ids } => {
            validate_ai_grid_item_count(selected_icon_ids.len())?;
            (selected_icon_ids, true, None)
        }
    };
    let selected_ids = selected_icon_ids
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    if strict && selected_ids.len() != selected_icon_ids.len() {
        return Err(AppError::new(
            "ai_grid_duplicate_target",
            "AI grid 대상 아이콘이 중복되었습니다.",
        ));
    }
    let icon_ids = {
        let mut statement = connection.prepare(
            "SELECT id FROM icons
             WHERE collection_id = ?1 AND deleted_at IS NULL AND icon_kind = 'image'
             ORDER BY order_index ASC, created_at ASC",
        )?;
        let rows = statement
            .query_map(params![collection_id], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        rows
    };
    let target_icon_ids = icon_ids
        .into_iter()
        .filter(|icon_id| {
            if strict {
                selected_ids.contains(icon_id.as_str())
            } else {
                selected_source != Some("selected_icons")
                    || selected_ids.is_empty()
                    || selected_ids.contains(icon_id.as_str())
            }
        })
        .collect::<Vec<_>>();
    if strict && target_icon_ids.len() != selected_icon_ids.len() {
        return Err(AppError::new(
            "ai_grid_target_missing",
            "선택한 AI grid 대상 중 현재 모음에서 사용할 수 없는 아이콘이 있습니다.",
        ));
    }
    for icon_id in &target_icon_ids {
        ai_repository::resolve_effective_visual_source(connection, collection_id, icon_id)?;
    }
    let query = format!(
        "{ICON_RECORD_SELECT}
         WHERE i.collection_id = ?1 AND i.deleted_at IS NULL AND i.icon_kind = 'image'
         ORDER BY i.order_index ASC, i.created_at ASC"
    );
    let mut statement = connection.prepare(&query)?;
    let icons = statement
        .query_map(params![collection_id], icon_record_from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    let icons = if strict || (selected_source == Some("selected_icons") && !selected_ids.is_empty())
    {
        icons
            .into_iter()
            .filter(|icon| selected_ids.contains(icon.id.as_str()))
            .collect::<Vec<_>>()
    } else {
        icons
    };
    if icons.len() != target_icon_ids.len()
        || icons
            .iter()
            .zip(target_icon_ids.iter())
            .any(|(icon, expected_id)| icon.id != *expected_id)
    {
        return Err(AppError::new(
            if strict {
                "ai_grid_target_state_missing"
            } else {
                "sheet_icon_state_missing"
            },
            if strict {
                "AI grid 대상 아이콘의 source 또는 자르기 상태가 누락되었습니다."
            } else {
                "편집 시트에 포함할 아이콘의 소스 또는 자르기 상태가 누락되었습니다. 아이콘을 복구한 뒤 다시 시도해 주세요."
            },
        ));
    }
    if strict {
        for icon in &icons {
            if icon.shape != "single" {
                return Err(AppError::new(
                    "ai_grid_shape_unsupported",
                    "AI grid에는 정적 단일 아이콘만 넣을 수 있습니다.",
                ));
            }
            if icon.source_is_animated {
                return Err(AppError::new(
                    "ai_grid_gif_unsupported",
                    "GIF는 아직 AI grid에 넣을 수 없습니다.",
                ));
            }
            if !matches!(icon.source_extension.as_str(), "png" | "jpg" | "jpeg")
                || !matches!(icon.source_mime_type.as_str(), "image/png" | "image/jpeg")
            {
                return Err(AppError::new(
                    "ai_grid_format_unsupported",
                    "AI grid에는 정적 JPG 또는 PNG 아이콘만 넣을 수 있습니다.",
                ));
            }
            if icon.configured_cell_width != icon.configured_cell_height {
                return Err(AppError::new(
                    "ai_grid_non_square_target",
                    "첫 AI grid 단계에서는 정사각형 단일 아이콘만 함께 편집할 수 있습니다.",
                ));
            }
        }
    }
    Ok(icons)
}
pub(crate) fn current_static_sheet_render_guard(
    connection: &Connection,
    collection_id: &str,
    icon_id: &str,
    piece_id: Option<&str>,
    cell_width: i64,
    cell_height: i64,
) -> AppResult<(String, String)> {
    if cell_width <= 0 || cell_height <= 0 {
        return Err(AppError::new(
            "validation",
            "작업 시트 셀 크기는 1 이상이어야 합니다.",
        ));
    }
    ai_repository::resolve_effective_visual_source(connection, collection_id, icon_id)?;
    let query = format!(
        "{ICON_RECORD_SELECT}
         WHERE i.id = ?1 AND i.collection_id = ?2 AND i.deleted_at IS NULL AND i.icon_kind = 'image'"
    );
    let icon = connection
        .query_row(
            &query,
            params![icon_id, collection_id],
            icon_record_from_row,
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("작업 시트 원본 아이콘을 찾을 수 없습니다."))?;
    let piece_index = match piece_id {
        Some(piece_id) => connection.query_row(
            "SELECT piece_index FROM icon_pieces WHERE id = ?1 AND icon_id = ?2",
            params![piece_id, icon_id], |row| row.get::<_, i64>(0),
        ).optional()?.ok_or_else(|| AppError::not_found("작업 시트 원본 조각을 찾을 수 없습니다."))?,
        None => connection.query_row(
            "SELECT piece_index FROM icon_pieces WHERE icon_id = ?1 ORDER BY piece_index ASC LIMIT 1",
            params![icon_id], |row| row.get::<_, i64>(0),
        ).optional()?.ok_or_else(|| AppError::not_found("작업 시트 원본 조각을 찾을 수 없습니다."))?,
    };
    let piece_index = usize::try_from(piece_index)
        .map_err(|_| AppError::new("validation", "작업 시트 조각 번호가 올바르지 않습니다."))?;
    let render_recipe_hash =
        static_sheet_render_recipe_hash(&icon, piece_index, cell_width, cell_height)?;
    Ok((icon.source_hash, render_recipe_hash))
}

pub(crate) fn render_icon_items(
    connection: &Connection,
    icon: &IconRecord,
    cell_width: i64,
    cell_height: i64,
) -> AppResult<Vec<RenderedSheetItem>> {
    let mut source = load_source_first_frame(Path::new(&icon.source_path), &icon.source_extension)?;
    apply_text_overlay(&mut source, icon.text_overlay.as_ref())?;
    let source_geometry =
        source_viewport_geometry(&icon.shape, cell_width, cell_height, icon.transform)?;
    let viewport = crop_and_resize(
        &source,
        icon.crop_x,
        icon.crop_y,
        icon.crop_w,
        icon.crop_h,
        source_geometry.viewport.width,
        source_geometry.viewport.height,
    )?;
    let viewport = apply_image_transform(viewport, icon.transform)?;
    let mut viewport = viewport;
    apply_effect_recipe(&mut viewport, &icon.effects)?;
    let motion_result = apply_motion_recipe(
        &viewport,
        &icon.motion,
        MotionFrameContext {
            elapsed_ms: 0,
            total_duration_ms: u64::try_from(icon.motion.duration_ms).unwrap_or(1).max(1),
        },
    )?;
    let viewport = motion_result.image;
    if i64::from(viewport.width()) != viewport_width(&icon.shape, cell_width)
        || i64::from(viewport.height()) != viewport_height(&icon.shape, cell_height)
    {
        return Err(AppError::new(
            "validation",
            "회전 후 작업 시트 조각 크기가 아이콘 모양과 일치하지 않습니다.",
        ));
    }
    let split = split_viewport(&viewport, &icon.shape, cell_width, cell_height)?;
    let pieces = load_pieces(connection, &icon.id)?;
    let mut items = Vec::new();
    if pieces.len() != split.len()
        || pieces
            .iter()
            .enumerate()
            .any(|(position, piece)| piece.piece_index != position as i64)
    {
        return Err(AppError::new(
            "sheet_piece_state_missing",
            "편집 시트에 포함할 아이콘의 조각 정보가 모양과 일치하지 않습니다. 아이콘을 복구한 뒤 다시 시도해 주세요.",
        ));
    }
    for (piece_position, piece_image) in split.into_iter().enumerate() {
        let piece = &pieces[piece_position];
        let render_hash = sha256_hex(&png_bytes_from_rgba(&piece_image)?);
        let render_recipe_hash =
            static_sheet_render_recipe_hash(icon, piece_position, cell_width, cell_height)?;
        items.push(RenderedSheetItem {
            icon_id: icon.id.clone(),
            piece_id: Some(piece.id.clone()),
            display_name: icon.display_name.clone(),
            alt: piece.alt_text.clone(),
            icon_type: icon.shape.clone(),
            source_hash: Some(icon.source_hash.clone()),
            visual_source: ManifestVisualSource {
                original_source_file_id: icon.original_source_file_id.clone(),
                original_source_hash: icon.original_source_hash.clone(),
                original_lineage_id: icon.original_lineage_id.clone(),
                original_lineage_generation: icon.original_lineage_generation,
                effective_source_file_id: icon.effective_source_file_id.clone(),
                effective_source_hash: icon.effective_source_hash.clone(),
            },
            render_hash,
            render_recipe_hash,
            image: piece_image,
        });
    }
    Ok(items)
}

pub(crate) fn render_sheet_page(
    items: &[RenderedSheetItem],
    placements: &[&PageCellPlacement],
    width: i64,
    height: i64,
    background: &str,
    guide: bool,
    labels: Option<&GuideLabelOptions>,
) -> AppResult<RgbaImage> {
    let width = u32::try_from(width)
        .map_err(|_| AppError::new("validation", "작업 시트 너비가 올바르지 않습니다."))?;
    let height = u32::try_from(height)
        .map_err(|_| AppError::new("validation", "작업 시트 높이가 올바르지 않습니다."))?;
    validate_import_dimensions(width, height)?;
    let mut sheet = background_image(width, height, background, guide);
    for placement in placements {
        let item = &items[placement.item_index];
        imageops::overlay(&mut sheet, &item.image, placement.x, placement.y);
    }
    if guide {
        let _text_labels_requested =
            labels.is_some_and(|labels| labels.icon_name || labels.alt_value);
        for (local_index, placement) in placements.iter().enumerate() {
            draw_grid_rect(&mut sheet, placement);
            let label_number = if labels.is_some_and(|labels| labels.export_number) {
                placement.item_index + 1
            } else {
                local_index + 1
            };
            if labels
                .map(|labels| labels.cell_number || labels.export_number)
                .unwrap_or(true)
            {
                draw_number_label(&mut sheet, placement.x + 4, placement.y + 4, label_number);
            }
        }
    }
    Ok(sheet)
}

pub(crate) fn crop_and_resize(
    image: &RgbaImage,
    crop_x: f64,
    crop_y: f64,
    crop_w: f64,
    crop_h: f64,
    width: i64,
    height: i64,
) -> AppResult<RgbaImage> {
    let crop = validate_crop_rect(crop_x, crop_y, crop_w, crop_h)?;
    let cropped = crop_with_padding(image, crop);
    let width = u32::try_from(width)
        .map_err(|_| AppError::new("validation", "작업 시트 조각 너비가 올바르지 않습니다."))?;
    let height = u32::try_from(height)
        .map_err(|_| AppError::new("validation", "작업 시트 조각 높이가 올바르지 않습니다."))?;
    validate_import_dimensions(width, height)?;
    Ok(imageops::resize(
        &cropped,
        width,
        height,
        FilterType::Lanczos3,
    ))
}

pub(crate) fn normalized_background(value: &str) -> String {
    match value {
        "checker" | "white" | "black" => value.to_string(),
        _ => "transparent".to_string(),
    }
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(64);
    for byte in digest {
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}
fn resolved_ai_grid_cells(
    layout: &AiGridLayout,
    item_count: usize,
) -> AppResult<Vec<super::grid::SheetCell>> {
    validate_ai_grid_layout(layout, item_count)?;
    let settings = SheetGridSettings {
        mode: "rows_columns".to_string(),
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
        empty_cell_threshold: None,
    };
    let cells = cells_for_settings(&settings, layout.canvas_width, layout.canvas_height)?;
    if cells.len() != usize::try_from(layout.rows * layout.columns).unwrap_or(usize::MAX)
        || cells
            .iter()
            .any(|cell| cell.page != 0 || cell.out_of_bounds)
    {
        return Err(ai_grid_layout_error(
            "AI grid는 정확히 한 페이지 안에 모든 cell이 들어가야 합니다.",
        ));
    }
    Ok(cells)
}

fn validate_ai_grid_layout(layout: &AiGridLayout, item_count: usize) -> AppResult<()> {
    validate_ai_grid_item_count(item_count)?;
    let width = u32::try_from(layout.canvas_width)
        .map_err(|_| ai_grid_layout_error("AI grid canvas 너비가 올바르지 않습니다."))?;
    let height = u32::try_from(layout.canvas_height)
        .map_err(|_| ai_grid_layout_error("AI grid canvas 높이가 올바르지 않습니다."))?;
    if width != height || width == 0 || width > MAX_AI_GRID_CANVAS_SIDE {
        return Err(ai_grid_layout_error(
            "AI grid canvas는 한 변 1~2048px의 정사각형이어야 합니다.",
        ));
    }
    let pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or_else(|| ai_grid_layout_error("AI grid canvas 픽셀 수가 너무 큽니다."))?;
    if pixels > MAX_AI_GRID_CANVAS_PIXELS {
        return Err(ai_grid_layout_error(
            "AI grid canvas의 전체 픽셀 수가 지원 범위를 벗어났습니다.",
        ));
    }
    if !(1..=MAX_AI_GRID_AXIS).contains(&layout.rows)
        || !(1..=MAX_AI_GRID_AXIS).contains(&layout.columns)
    {
        return Err(ai_grid_layout_error(
            "AI grid 행과 열은 각각 1~4여야 합니다.",
        ));
    }
    let capacity = layout
        .rows
        .checked_mul(layout.columns)
        .ok_or_else(|| ai_grid_layout_error("AI grid cell 수가 너무 큽니다."))?;
    if capacity < i64::try_from(item_count).unwrap_or(i64::MAX)
        || capacity > i64::try_from(MAX_AI_GRID_ITEMS).unwrap_or(i64::MAX)
    {
        return Err(ai_grid_layout_error(
            "AI grid cell 수가 대상 수와 맞지 않거나 16칸을 넘습니다.",
        ));
    }
    if layout.cell_size <= 0
        || layout.gap_x < 0
        || layout.gap_y < 0
        || layout.border_left < 0
        || layout.border_top < 0
        || layout.border_right < 0
        || layout.border_bottom < 0
    {
        return Err(ai_grid_layout_error(
            "AI grid cell, 간격, 테두리 값이 올바르지 않습니다.",
        ));
    }
    let expected_width = axis_extent(layout.columns, layout.cell_size, layout.gap_x)?
        .checked_add(layout.border_left)
        .and_then(|value| value.checked_add(layout.border_right))
        .ok_or_else(|| ai_grid_layout_error("AI grid 가로 좌표가 너무 큽니다."))?;
    let expected_height = axis_extent(layout.rows, layout.cell_size, layout.gap_y)?
        .checked_add(layout.border_top)
        .and_then(|value| value.checked_add(layout.border_bottom))
        .ok_or_else(|| ai_grid_layout_error("AI grid 세로 좌표가 너무 큽니다."))?;
    if expected_width != layout.canvas_width || expected_height != layout.canvas_height {
        return Err(ai_grid_layout_error(
            "AI grid cell, 간격, 테두리 합이 canvas 크기와 정확히 맞아야 합니다.",
        ));
    }
    Ok(())
}

fn validate_ai_grid_item_count(item_count: usize) -> AppResult<()> {
    if !(MIN_AI_GRID_ITEMS..=MAX_AI_GRID_ITEMS).contains(&item_count) {
        return Err(AppError::new(
            "ai_grid_item_count",
            "AI grid 편집 대상은 2~16개여야 합니다.",
        ));
    }
    Ok(())
}

fn axis_extent(count: i64, cell_size: i64, gap: i64) -> AppResult<i64> {
    count
        .checked_mul(cell_size)
        .and_then(|value| {
            count
                .saturating_sub(1)
                .checked_mul(gap)
                .and_then(|gap_extent| value.checked_add(gap_extent))
        })
        .ok_or_else(|| ai_grid_layout_error("AI grid 배치 계산이 지원 범위를 벗어났습니다."))
}

fn ai_grid_layout_error(message: &str) -> AppError {
    AppError::new("ai_grid_layout_invalid", message)
}

const ICON_RECORD_SELECT: &str = "SELECT
   i.id,
   i.display_name,
   i.shape,
   evs.original_source_file_id,
   evs.original_source_sha256,
   evs.original_lineage_id,
   evs.original_lineage_generation,
   evs.effective_source_file_id,
   evs.effective_source_sha256,
   s.original_path_in_library,
   s.original_extension,
   s.mime_type,
   s.sha256,
   s.width,
   s.height,
   s.is_animated,
   COALESCE(i.cell_width_override, c.default_cell_width) AS configured_cell_width,
   COALESCE(i.cell_height_override, c.default_cell_height) AS configured_cell_height,
   cs.crop_x,
   cs.crop_y,
   cs.crop_w,
   cs.crop_h,
   i.transform_quarter_turns,
   i.transform_flip_horizontal,
   i.transform_flip_vertical,
   i.text_overlay_enabled,
   i.text_overlay_text,
   i.text_overlay_font_path,
   i.text_overlay_font_size,
   i.text_overlay_x,
   i.text_overlay_y,
   i.text_overlay_color,
   i.text_overlay_stroke_color,
   i.text_overlay_stroke_width,
   er.effects_json AS effect_recipe_json,
   mr.motion_json AS motion_recipe_json
 FROM icons i
 JOIN collections c ON c.id = i.collection_id
 JOIN effective_visual_sources evs ON evs.icon_id = i.id
 JOIN source_files s ON s.id = evs.effective_source_file_id
 JOIN crop_settings cs ON cs.icon_id = i.id
 LEFT JOIN icon_effect_recipes er ON er.icon_id = i.id
 LEFT JOIN icon_motion_recipes mr ON mr.icon_id = i.id";

fn icon_record_from_row(row: &Row<'_>) -> rusqlite::Result<IconRecord> {
    Ok(IconRecord {
        id: row.get("id")?,
        display_name: row.get("display_name")?,
        shape: row.get("shape")?,
        original_source_file_id: row.get("original_source_file_id")?,
        original_source_hash: row.get("original_source_sha256")?,
        original_lineage_id: row.get("original_lineage_id")?,
        original_lineage_generation: row.get("original_lineage_generation")?,
        effective_source_file_id: row.get("effective_source_file_id")?,
        effective_source_hash: row.get("effective_source_sha256")?,
        source_path: row.get("original_path_in_library")?,
        source_extension: row.get("original_extension")?,
        source_mime_type: row.get("mime_type")?,
        source_hash: row.get("sha256")?,
        source_width: row.get("width")?,
        source_height: row.get("height")?,
        source_is_animated: row.get::<_, i64>("is_animated")? == 1,
        configured_cell_width: row.get("configured_cell_width")?,
        configured_cell_height: row.get("configured_cell_height")?,
        crop_x: row.get("crop_x")?,
        crop_y: row.get("crop_y")?,
        crop_w: row.get("crop_w")?,
        crop_h: row.get("crop_h")?,
        transform: ImageTransform {
            quarter_turns: row.get("transform_quarter_turns")?,
            flip_horizontal: row.get::<_, i64>("transform_flip_horizontal")? != 0,
            flip_vertical: row.get::<_, i64>("transform_flip_vertical")? != 0,
        },
        text_overlay: text_overlay_from_fields(
            row.get::<_, i64>("text_overlay_enabled")? != 0,
            row.get("text_overlay_text")?,
            row.get("text_overlay_font_path")?,
            row.get("text_overlay_font_size")?,
            row.get("text_overlay_x")?,
            row.get("text_overlay_y")?,
            row.get("text_overlay_color")?,
            row.get("text_overlay_stroke_color")?,
            row.get("text_overlay_stroke_width")?,
        )
        .map_err(sql_conversion_error)?,
        effects: parse_effect_recipe_json(
            row.get::<_, Option<String>>("effect_recipe_json")?
                .as_deref()
                .unwrap_or_default(),
        )
        .map_err(sql_conversion_error)?,
        motion: parse_motion_recipe_json(
            row.get::<_, Option<String>>("motion_recipe_json")?
                .as_deref()
                .unwrap_or_default(),
        )
        .map_err(sql_conversion_error)?,
    })
}

fn sql_conversion_error(error: AppError) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
}

fn static_sheet_render_recipe_hash(
    icon: &IconRecord,
    piece_index: usize,
    cell_width: i64,
    cell_height: i64,
) -> AppResult<String> {
    let recipe_hash = render_recipe_crop_hash(
        &icon.shape,
        &ExportCropRect {
            x: icon.crop_x,
            y: icon.crop_y,
            width: icon.crop_w,
            height: icon.crop_h,
        },
        cell_width,
        cell_height,
        piece_index,
        icon.transform,
        "static_sheet_poster",
        None,
        icon.text_overlay.as_ref(),
        &icon.effects,
        &icon.motion,
    )?;
    Ok(hash_text(&[
        "static_sheet_render_guard_v1".to_string(),
        icon.source_hash.clone(),
        icon.source_extension.to_ascii_lowercase(),
        "source_frame:0".to_string(),
        "motion_elapsed_ms:0".to_string(),
        "resize_filter:lanczos3".to_string(),
        recipe_hash,
    ]))
}

fn load_pieces(connection: &Connection, icon_id: &str) -> AppResult<Vec<PieceRecord>> {
    let mut statement = connection.prepare(
        "SELECT id, piece_index, alt_text FROM icon_pieces WHERE icon_id = ?1 ORDER BY piece_index ASC",
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
fn load_source_first_frame(path: &Path, extension: &str) -> AppResult<RgbaImage> {
    if extension == "gif" {
        let file = fs::File::open(path)?;
        let decoder = GifDecoder::new(BufReader::new(file))?;
        let mut frames = decoder.into_frames();
        let frame = frames
            .next()
            .transpose()?
            .ok_or_else(|| AppError::new("gif", "GIF 첫 프레임을 읽을 수 없습니다."))?;
        return Ok(frame.into_buffer());
    }
    Ok(image::open(path)?.to_rgba8())
}

fn crop_with_padding(source: &RgbaImage, crop: ValidatedCropRect) -> RgbaImage {
    let crop_x = crop.x;
    let crop_y = crop.y;
    let crop_width = crop.width;
    let crop_height = crop.height;
    let mut output = RgbaImage::from_pixel(crop_width, crop_height, Rgba([0, 0, 0, 0]));
    let source_width = i64::from(source.width());
    let source_height = i64::from(source.height());
    let src_x = crop_x.max(0);
    let src_y = crop_y.max(0);
    let dst_x = crop_x.saturating_neg().max(0);
    let dst_y = crop_y.saturating_neg().max(0);
    let copy_width = (source_width - src_x)
        .min(i64::from(crop_width) - dst_x)
        .max(0) as u32;
    let copy_height = (source_height - src_y)
        .min(i64::from(crop_height) - dst_y)
        .max(0) as u32;
    for y in 0..copy_height {
        for x in 0..copy_width {
            output.put_pixel(
                (dst_x as u32) + x,
                (dst_y as u32) + y,
                *source.get_pixel((src_x as u32) + x, (src_y as u32) + y),
            );
        }
    }
    output
}

fn split_viewport(
    viewport: &RgbaImage,
    shape: &str,
    cell_width: i64,
    cell_height: i64,
) -> AppResult<Vec<RgbaImage>> {
    let width = u32::try_from(cell_width)
        .map_err(|_| AppError::new("validation", "작업 시트 조각 너비가 올바르지 않습니다."))?;
    let height = u32::try_from(cell_height)
        .map_err(|_| AppError::new("validation", "작업 시트 조각 높이가 올바르지 않습니다."))?;
    validate_import_dimensions(width, height)?;
    match shape {
        "horizontal_double" => Ok(vec![
            imageops::crop_imm(viewport, 0, 0, width, height).to_image(),
            imageops::crop_imm(viewport, width, 0, width, height).to_image(),
        ]),
        "vertical_double" => Ok(vec![
            imageops::crop_imm(viewport, 0, 0, width, height).to_image(),
            imageops::crop_imm(viewport, 0, height, width, height).to_image(),
        ]),
        "single" => Ok(vec![viewport.clone()]),
        _ => Err(AppError::new(
            "validation",
            "지원하지 않는 아이콘 모양입니다.",
        )),
    }
}

fn background_image(width: u32, height: u32, background: &str, guide: bool) -> RgbaImage {
    let normalized = normalized_background(background);
    if normalized == "checker" || (guide && normalized == "transparent") {
        return checkerboard(width, height);
    }
    let pixel = match normalized.as_str() {
        "white" => Rgba([255, 255, 255, 255]),
        "black" => Rgba([0, 0, 0, 255]),
        _ => Rgba([0, 0, 0, 0]),
    };
    RgbaImage::from_pixel(width, height, pixel)
}

fn checkerboard(width: u32, height: u32) -> RgbaImage {
    let mut image = RgbaImage::from_pixel(width, height, Rgba([235, 238, 242, 255]));
    for y in 0..height {
        for x in 0..width {
            if ((x / 12) + (y / 12)) % 2 == 0 {
                image.put_pixel(x, y, Rgba([255, 255, 255, 255]));
            }
        }
    }
    image
}

fn draw_grid_rect(sheet: &mut RgbaImage, placement: &PageCellPlacement) {
    let color = Rgba([37, 48, 61, 210]);
    let x0 = placement.x.max(0) as u32;
    let y0 = placement.y.max(0) as u32;
    let x1 = (placement.x + placement.w - 1).max(0) as u32;
    let y1 = (placement.y + placement.h - 1).max(0) as u32;
    for x in x0..=x1.min(sheet.width().saturating_sub(1)) {
        if y0 < sheet.height() {
            sheet.put_pixel(x, y0, color);
        }
        if y1 < sheet.height() {
            sheet.put_pixel(x, y1, color);
        }
    }
    for y in y0..=y1.min(sheet.height().saturating_sub(1)) {
        if x0 < sheet.width() {
            sheet.put_pixel(x0, y, color);
        }
        if x1 < sheet.width() {
            sheet.put_pixel(x1, y, color);
        }
    }
}

fn draw_number_label(sheet: &mut RgbaImage, x: i64, y: i64, number: usize) {
    let mut cursor_x = x.max(0) as u32;
    let y = y.max(0) as u32;
    for character in number.to_string().chars() {
        draw_digit(sheet, cursor_x, y, character);
        cursor_x += 5;
    }
}

fn draw_digit(sheet: &mut RgbaImage, x: u32, y: u32, character: char) {
    let Some(pattern) = digit_pattern(character) else {
        return;
    };
    let background = Rgba([255, 255, 255, 210]);
    let foreground = Rgba([20, 28, 38, 255]);
    for yy in 0..7 {
        for xx in 0..4 {
            let px = x + xx;
            let py = y + yy;
            if px < sheet.width() && py < sheet.height() {
                sheet.put_pixel(px, py, background);
            }
        }
    }
    for (row, bits) in pattern.iter().enumerate() {
        for col in 0..3 {
            if bits & (1 << (2 - col)) != 0 {
                let px = x + col;
                let py = y + row as u32 + 1;
                if px < sheet.width() && py < sheet.height() {
                    sheet.put_pixel(px, py, foreground);
                }
            }
        }
    }
}

fn digit_pattern(character: char) -> Option<[u8; 5]> {
    match character {
        '0' => Some([0b111, 0b101, 0b101, 0b101, 0b111]),
        '1' => Some([0b010, 0b110, 0b010, 0b010, 0b111]),
        '2' => Some([0b111, 0b001, 0b111, 0b100, 0b111]),
        '3' => Some([0b111, 0b001, 0b111, 0b001, 0b111]),
        '4' => Some([0b101, 0b101, 0b111, 0b001, 0b001]),
        '5' => Some([0b111, 0b100, 0b111, 0b001, 0b111]),
        '6' => Some([0b111, 0b100, 0b111, 0b101, 0b111]),
        '7' => Some([0b111, 0b001, 0b010, 0b010, 0b010]),
        '8' => Some([0b111, 0b101, 0b111, 0b101, 0b111]),
        '9' => Some([0b111, 0b101, 0b111, 0b001, 0b111]),
        _ => None,
    }
}

fn viewport_width(shape: &str, cell_width: i64) -> i64 {
    if shape == "horizontal_double" {
        cell_width * 2
    } else {
        cell_width
    }
}

fn viewport_height(shape: &str, cell_height: i64) -> i64 {
    if shape == "vertical_double" {
        cell_height * 2
    } else {
        cell_height
    }
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::time::{SystemTime, UNIX_EPOCH};

    use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};
    use rusqlite::Connection;

    use crate::db::migrations;
    use crate::db::repositories::collections::create_collection;
    use crate::db::repositories::imports::import_image_files;
    use crate::models::ImportImageFilePayload;
    use crate::paths::AppPaths;
    use crate::sheet::exporter::{export_edit_sheet, ExportEditSheetRequest};

    use super::{
        compose_ai_edit_grid, default_ai_generation_layout, default_ai_grid_layout, AiGridLayout,
        ComposeAiEditGridRequest, MAX_AI_GRID_ITEMS,
    };

    fn temp_paths(label: &str) -> AppPaths {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        AppPaths::prepare(std::env::temp_dir().join(format!("pmtconcon-{label}-{suffix}"))).unwrap()
    }

    fn connection() -> Connection {
        let mut connection = Connection::open_in_memory().unwrap();
        migrations::run(&mut connection).unwrap();
        connection
    }

    fn fixture_png(index: usize) -> Vec<u8> {
        let image = ImageBuffer::from_fn(20, 20, |x, y| {
            Rgba([
                (index as u8).wrapping_mul(29).wrapping_add(x as u8),
                40_u8.wrapping_add(y as u8),
                180_u8.wrapping_sub(index as u8),
                160_u8.wrapping_add(((x + y) % 95) as u8),
            ])
        });
        let mut cursor = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image)
            .write_to(&mut cursor, ImageFormat::Png)
            .unwrap();
        cursor.into_inner()
    }

    fn setup_icons(count: usize, label: &str) -> (Connection, AppPaths, String, Vec<String>) {
        let mut connection = connection();
        let paths = temp_paths(label);
        let collection = create_collection(&mut connection, Some(label.to_string())).unwrap();
        let payloads = (0..count)
            .map(|index| ImportImageFilePayload {
                original_filename: format!("cell-{index:02}.png"),
                bytes: fixture_png(index),
            })
            .collect::<Vec<_>>();
        let imported =
            import_image_files(&mut connection, &paths, &collection.id, payloads).unwrap();
        let ids = imported
            .imported_icons
            .into_iter()
            .map(|icon| icon.id)
            .collect();
        (connection, paths, collection.id, ids)
    }

    #[test]
    fn default_layout_is_exact_one_page_for_all_supported_buckets() {
        let expected = [
            (2, 1, 2, 504),
            (3, 2, 2, 504),
            (4, 2, 2, 504),
            (5, 3, 3, 330),
            (9, 3, 3, 330),
            (10, 4, 4, 244),
            (16, 4, 4, 244),
        ];
        for (count, rows, columns, cell_size) in expected {
            let layout = default_ai_grid_layout(count, 1024).unwrap();
            assert_eq!((layout.rows, layout.columns), (rows, columns));
            assert_eq!(layout.cell_size, cell_size);
            assert_eq!(
                layout.columns * layout.cell_size
                    + (layout.columns - 1) * layout.gap_x
                    + layout.border_left
                    + layout.border_right,
                1024
            );
            assert_eq!(
                layout.rows * layout.cell_size
                    + (layout.rows - 1) * layout.gap_y
                    + layout.border_top
                    + layout.border_bottom,
                1024
            );
        }
        assert!(default_ai_grid_layout(1, 1024).is_err());
        assert!(default_ai_grid_layout(MAX_AI_GRID_ITEMS + 1, 1024).is_err());
        assert!(default_ai_grid_layout(2, 2049).is_err());
    }

    #[test]
    fn generation_default_layout_supports_single_without_loosening_edit_grid_minimum() {
        let layout = default_ai_generation_layout(1, 1024).unwrap();
        assert_eq!(
            layout,
            AiGridLayout {
                canvas_width: 1024,
                canvas_height: 1024,
                rows: 1,
                columns: 1,
                cell_size: 1024,
                gap_x: 0,
                gap_y: 0,
                border_left: 0,
                border_top: 0,
                border_right: 0,
                border_bottom: 0,
            }
        );
        assert!(default_ai_grid_layout(1, 1024).is_err());
        assert!(default_ai_generation_layout(0, 1024).is_err());
        assert!(default_ai_generation_layout(MAX_AI_GRID_ITEMS + 1, 1024).is_err());
    }

    #[test]
    fn strict_composer_is_ordered_byte_deterministic_and_matches_work_sheet_bytes() {
        let (connection, paths, collection_id, ids) = setup_icons(2, "ai-grid-determinism");
        let selected = vec![ids[1].clone(), ids[0].clone()];
        let layout = default_ai_grid_layout(2, 1024).unwrap();
        let compose = || {
            compose_ai_edit_grid(
                &connection,
                ComposeAiEditGridRequest {
                    collection_id: &collection_id,
                    selected_icon_ids: &selected,
                    layout: layout.clone(),
                },
            )
            .unwrap()
        };
        let first = compose();
        let second = compose();
        assert_eq!(first.png_bytes, second.png_bytes);
        assert_eq!(first.png_sha256, second.png_sha256);
        assert_eq!(first.manifest_json, second.manifest_json);
        assert_eq!(first.manifest_sha256, second.manifest_sha256);
        assert_eq!(first.layout, layout);
        assert_eq!(first.items.len(), 2);
        assert_eq!(first.items[0].origin_icon_id, ids[0]);
        assert_eq!(first.items[1].origin_icon_id, ids[1]);
        assert_eq!(first.items[0].item_index, 0);
        assert_eq!(first.items[1].item_index, 1);
        assert!(first.manifest_json.contains("pmtcon-ai-grid-v1"));

        let work = export_edit_sheet(
            &connection,
            &paths,
            ExportEditSheetRequest {
                collection_id: collection_id.clone(),
                selected_icon_ids: selected,
                source: "selected_icons".to_string(),
                cell_width: layout.cell_size,
                cell_height: layout.cell_size,
                columns: layout.columns,
                gap_x: layout.gap_x,
                gap_y: layout.gap_y,
                border_x: layout.border_left,
                border_y: layout.border_top,
                background: "transparent".to_string(),
                include_clean_sheet: true,
                include_guide_sheet: false,
                include_manifest: false,
                label_options: None,
                max_sheet_width: layout.canvas_width,
                max_sheet_height: layout.canvas_height,
                output_directory: Some(
                    paths
                        .root
                        .join("compat-output")
                        .to_string_lossy()
                        .to_string(),
                ),
                open_output_folder: false,
            },
        )
        .unwrap();
        let work_bytes = std::fs::read(&work.clean_sheet_paths[0]).unwrap();
        assert_eq!(first.png_bytes, work_bytes);
        assert_eq!(
            first.png_sha256,
            "e242ba2e97344233dc5ef9c46dbb7d2bef7cc5144661f848804c5722835a3454"
        );
        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn strict_composer_rejects_duplicate_targets_without_collection_fallback() {
        let (connection, paths, collection_id, ids) = setup_icons(2, "ai-grid-strict");
        let duplicate = vec![ids[0].clone(), ids[0].clone()];
        let error = compose_ai_edit_grid(
            &connection,
            ComposeAiEditGridRequest {
                collection_id: &collection_id,
                selected_icon_ids: &duplicate,
                layout: default_ai_grid_layout(2, 1024).unwrap(),
            },
        )
        .unwrap_err();
        assert_eq!(error.code, "ai_grid_duplicate_target");
        let one = vec![ids[0].clone()];
        let error = compose_ai_edit_grid(
            &connection,
            ComposeAiEditGridRequest {
                collection_id: &collection_id,
                selected_icon_ids: &one,
                layout: AiGridLayout {
                    canvas_width: 1024,
                    canvas_height: 1024,
                    rows: 1,
                    columns: 1,
                    cell_size: 1024,
                    gap_x: 0,
                    gap_y: 0,
                    border_left: 0,
                    border_top: 0,
                    border_right: 0,
                    border_bottom: 0,
                },
            },
        )
        .unwrap_err();
        assert_eq!(error.code, "ai_grid_item_count");
        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn ten_item_grid_keeps_unused_capacity_cells_transparent() {
        let (connection, paths, collection_id, ids) = setup_icons(10, "ai-grid-capacity");
        let layout = default_ai_grid_layout(10, 1024).unwrap();
        let composed = compose_ai_edit_grid(
            &connection,
            ComposeAiEditGridRequest {
                collection_id: &collection_id,
                selected_icon_ids: &ids,
                layout: layout.clone(),
            },
        )
        .unwrap();
        let image = image::load_from_memory_with_format(&composed.png_bytes, ImageFormat::Png)
            .unwrap()
            .to_rgba8();
        let unused_x = 2 * (layout.cell_size + layout.gap_x) + layout.cell_size / 2;
        let unused_y = 2 * (layout.cell_size + layout.gap_y) + layout.cell_size / 2;
        assert_eq!(
            image
                .get_pixel(
                    u32::try_from(unused_x).unwrap(),
                    u32::try_from(unused_y).unwrap()
                )
                .0[3],
            0
        );
        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn strict_composer_rejects_double_and_animated_targets() {
        let (connection, paths, collection_id, ids) = setup_icons(2, "ai-grid-kind-gates");
        connection
            .execute(
                "UPDATE icons SET shape = 'horizontal_double' WHERE id = ?1",
                [&ids[0]],
            )
            .unwrap();
        let error = compose_ai_edit_grid(
            &connection,
            ComposeAiEditGridRequest {
                collection_id: &collection_id,
                selected_icon_ids: &ids,
                layout: default_ai_grid_layout(2, 1024).unwrap(),
            },
        )
        .unwrap_err();
        assert_eq!(error.code, "ai_grid_shape_unsupported");

        connection
            .execute("UPDATE icons SET shape = 'single' WHERE id = ?1", [&ids[0]])
            .unwrap();
        let source_file_id: String = connection
            .query_row(
                "SELECT effective_source_file_id FROM effective_visual_sources WHERE icon_id = ?1",
                [&ids[0]],
                |row| row.get(0),
            )
            .unwrap();
        connection
            .execute(
                "UPDATE source_files SET is_animated = 1 WHERE id = ?1",
                [&source_file_id],
            )
            .unwrap();
        let error = compose_ai_edit_grid(
            &connection,
            ComposeAiEditGridRequest {
                collection_id: &collection_id,
                selected_icon_ids: &ids,
                layout: default_ai_grid_layout(2, 1024).unwrap(),
            },
        )
        .unwrap_err();
        assert_eq!(error.code, "ai_grid_gif_unsupported");
        std::fs::remove_dir_all(paths.root).unwrap();
    }
}
