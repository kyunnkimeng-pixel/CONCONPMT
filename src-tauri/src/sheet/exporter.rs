use std::collections::HashSet;
use std::fmt::Write as FmtWrite;
use std::fs;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use image::codecs::gif::GifDecoder;
use image::imageops::{self, FilterType};
use image::{AnimationDecoder, ImageFormat, Rgba, RgbaImage};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{AppError, AppResult};
use crate::imaging::effects::{apply_effect_recipe, parse_effect_recipe_json, EffectRecipe};
use crate::imaging::export_render::ExportCropRect;
use crate::imaging::import_limits::{
    validate_crop_rect, validate_import_dimensions, ValidatedCropRect, MAX_GIF_TOTAL_FRAME_PIXELS,
    MAX_IMPORT_DIMENSION,
};
use crate::imaging::motion::{
    apply_motion_recipe, parse_motion_recipe_json, MotionFrameContext, MotionRecipe,
};
use crate::imaging::text_overlay::{
    apply_text_overlay, text_overlay_from_fields, TextOverlayRenderSpec,
};
use crate::imaging::transform::{apply_image_transform, source_viewport_geometry, ImageTransform};
use crate::optimization::cache::{hash_text, render_recipe_crop_hash};
use crate::paths::AppPaths;

use super::grid::{
    split_pages, PageCellPlacement, PageSplitPlan, PageSplitSettings, MAX_SHEET_CELLS,
};
use super::importer::png_bytes_from_rgba;
use super::manifest::{
    write_static_manifest, StaticSheetManifest, StaticSheetManifestItem, StaticSheetPage,
    StaticSheetProfile, APP_NAME, STATIC_SHEET_SCHEMA,
};
use super::path_string;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportEditSheetRequest {
    pub collection_id: String,
    #[serde(default)]
    pub selected_icon_ids: Vec<String>,
    #[serde(default = "default_sheet_source")]
    pub source: String,
    pub cell_width: i64,
    pub cell_height: i64,
    pub columns: i64,
    #[serde(default)]
    pub gap_x: i64,
    #[serde(default)]
    pub gap_y: i64,
    #[serde(default)]
    pub border_x: i64,
    #[serde(default)]
    pub border_y: i64,
    #[serde(default = "default_background")]
    pub background: String,
    #[serde(default = "default_true")]
    pub include_clean_sheet: bool,
    #[serde(default = "default_true")]
    pub include_guide_sheet: bool,
    #[serde(default = "default_true")]
    pub include_manifest: bool,
    pub label_options: Option<GuideLabelOptions>,
    #[serde(default = "default_max_sheet_size")]
    pub max_sheet_width: i64,
    #[serde(default = "default_max_sheet_size")]
    pub max_sheet_height: i64,
    pub output_directory: Option<String>,
    #[serde(default)]
    pub open_output_folder: bool,
}

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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportEditSheetResult {
    pub clean_sheet_paths: Vec<String>,
    pub guide_sheet_paths: Vec<String>,
    pub manifest_path: Option<String>,
    pub output_directory: String,
    pub item_count: i64,
    pub page_count: i64,
    pub warnings: Vec<String>,
}

#[derive(Debug)]
struct CollectionRecord {
    id: String,
    name: String,
}

#[derive(Debug)]
struct IconRecord {
    id: String,
    display_name: String,
    shape: String,
    source_path: String,
    source_extension: String,
    source_hash: String,
    source_is_animated: bool,
    crop_x: f64,
    crop_y: f64,
    crop_w: f64,
    crop_h: f64,
    transform: ImageTransform,
    text_overlay: Option<TextOverlayRenderSpec>,
    effects: EffectRecipe,
    motion: MotionRecipe,
}

#[derive(Debug)]
struct PieceRecord {
    id: String,
    piece_index: i64,
    alt_text: String,
}

#[derive(Debug)]
struct RenderedSheetItem {
    icon_id: String,
    piece_id: Option<String>,
    display_name: String,
    alt: String,
    icon_type: String,
    source_hash: Option<String>,
    render_hash: String,
    render_recipe_hash: String,
    image: RgbaImage,
}

pub fn export_edit_sheet(
    connection: &Connection,
    paths: &AppPaths,
    request: ExportEditSheetRequest,
) -> AppResult<ExportEditSheetResult> {
    validate_export_settings(&request)?;
    if !request.include_clean_sheet && !request.include_guide_sheet && !request.include_manifest {
        return Err(AppError::new(
            "validation",
            "내보낼 작업 시트 산출물을 하나 이상 선택해야 합니다.",
        ));
    }

    let collection = load_collection(connection, &request.collection_id)?;
    let icons = load_icons(connection, &request)?;
    if icons.is_empty() {
        return Err(AppError::new(
            "validation",
            "작업 시트로 내보낼 아이콘이 없습니다.",
        ));
    }
    let expected_item_count = icons.iter().try_fold(0_usize, |count, icon| {
        count
            .checked_add(if icon.shape == "single" { 1 } else { 2 })
            .ok_or_else(|| {
                AppError::new(
                    "validation",
                    "작업 시트 아이콘 조각 수가 지원 범위를 벗어났습니다.",
                )
            })
    })?;
    let _ = validated_page_plan(expected_item_count, &request)?;

    let mut warnings = Vec::new();
    let mut rendered_items = Vec::new();
    for icon in icons {
        if icon.source_is_animated {
            warnings.push(format!(
                "{}: GIF 첫 프레임만 정적 작업 시트에 포함했습니다.",
                icon.display_name
            ));
        }
        if icon.motion.has_enabled_motion() {
            warnings.push(format!(
                "{}: 모션 효과의 0ms 포스터 프레임만 정적 작업 시트에 포함했습니다.",
                icon.display_name
            ));
        }
        match render_icon_items(connection, &icon, request.cell_width, request.cell_height) {
            Ok(items) => rendered_items.extend(items),
            Err(error) => warnings.push(format!("{}: {}", icon.display_name, error.message)),
        }
    }

    if rendered_items.is_empty() {
        return Err(AppError::new(
            "validation",
            "작업 시트에 배치할 수 있는 렌더링 결과가 없습니다.",
        ));
    }

    let split = validated_page_plan(rendered_items.len(), &request)?;
    let _rows_per_page = split.rows_per_page;
    warnings.extend(split.warnings);

    let output_root = output_directory(paths, &request, &collection.name)?;
    let clean_dir = output_root.join("clean");
    let guide_dir = output_root.join("guide");
    fs::create_dir_all(&clean_dir)?;
    fs::create_dir_all(&guide_dir)?;

    let mut clean_paths = Vec::new();
    let mut guide_paths = Vec::new();
    let mut manifest_pages = Vec::new();
    let mut manifest_items = Vec::new();

    for page in &split.pages {
        let page_index = page.page_index;
        let clean_file = format!("sheet_{:03}.png", page_index + 1);
        let guide_file = format!("sheet_guide_{:03}.png", page_index + 1);
        let page_placements = split
            .placements
            .iter()
            .filter(|placement| placement.page_index == page_index)
            .collect::<Vec<_>>();

        if request.include_clean_sheet {
            let clean_image = render_sheet_page(
                &rendered_items,
                &page_placements,
                page.width,
                page.height,
                &request.background,
                false,
                request.label_options.as_ref(),
            )?;
            let clean_path = clean_dir.join(&clean_file);
            clean_image.save_with_format(&clean_path, ImageFormat::Png)?;
            clean_paths.push(path_string(&clean_path));
        }

        if request.include_guide_sheet {
            let guide_image = render_sheet_page(
                &rendered_items,
                &page_placements,
                page.width,
                page.height,
                &request.background,
                true,
                request.label_options.as_ref(),
            )?;
            let guide_path = guide_dir.join(&guide_file);
            guide_image.save_with_format(&guide_path, ImageFormat::Png)?;
            guide_paths.push(path_string(&guide_path));
        }

        manifest_pages.push(StaticSheetPage {
            page_index,
            clean_sheet_file: clean_file,
            guide_sheet_file: request.include_guide_sheet.then_some(guide_file),
            width: page.width,
            height: page.height,
        });
    }

    for (export_index, placement) in split.placements.iter().enumerate() {
        let item = &rendered_items[placement.item_index];
        manifest_items.push(StaticSheetManifestItem {
            icon_id: item.icon_id.clone(),
            piece_id: item.piece_id.clone(),
            page_index: placement.page_index,
            row: placement.row,
            col: placement.col,
            index: export_index as i64,
            export_number: export_index as i64 + 1,
            x: placement.x,
            y: placement.y,
            w: placement.w,
            h: placement.h,
            display_name: item.display_name.clone(),
            alt: item.alt.clone(),
            icon_type: item.icon_type.clone(),
            format: "png".to_string(),
            source_hash: item.source_hash.clone(),
            render_hash: Some(item.render_hash.clone()),
            render_recipe_hash: Some(item.render_recipe_hash.clone()),
        });
    }

    let manifest_path = if request.include_manifest {
        let manifest = StaticSheetManifest {
            schema: STATIC_SHEET_SCHEMA.to_string(),
            app: APP_NAME.to_string(),
            created_at: now_iso_like(),
            collection_id: collection.id,
            sheet_type: "static_edit_sheet".to_string(),
            profile: StaticSheetProfile {
                cell_width: request.cell_width,
                cell_height: request.cell_height,
                columns: split.columns_per_page,
                gap_x: request.gap_x.max(0),
                gap_y: request.gap_y.max(0),
                border_x: request.border_x.max(0),
                border_y: request.border_y.max(0),
                background: normalized_background(&request.background),
                read_order: "row_major".to_string(),
            },
            pages: manifest_pages,
            items: manifest_items,
        };
        let path = output_root.join("sheet_manifest.json");
        write_static_manifest(&path, &manifest)?;
        Some(path_string(&path))
    } else {
        None
    };

    if request.open_output_folder {
        crate::export::open_export_path(&path_string(&output_root))?;
    }

    Ok(ExportEditSheetResult {
        clean_sheet_paths: clean_paths,
        guide_sheet_paths: guide_paths,
        manifest_path,
        output_directory: path_string(&output_root),
        item_count: rendered_items.len() as i64,
        page_count: split.pages.len() as i64,
        warnings,
    })
}

fn validate_export_settings(request: &ExportEditSheetRequest) -> AppResult<()> {
    let cell_width = u32::try_from(request.cell_width)
        .map_err(|_| AppError::new("validation", "작업 시트 셀 너비가 올바르지 않습니다."))?;
    let cell_height = u32::try_from(request.cell_height)
        .map_err(|_| AppError::new("validation", "작업 시트 셀 높이가 올바르지 않습니다."))?;
    validate_import_dimensions(cell_width, cell_height)?;

    if !(1..=MAX_SHEET_CELLS).contains(&request.columns) {
        return Err(AppError::new(
            "validation",
            format!("작업 시트 열 수는 1 이상 {MAX_SHEET_CELLS} 이하여야 합니다."),
        ));
    }
    if request.gap_x < 0
        || request.gap_y < 0
        || request.border_x < 0
        || request.border_y < 0
        || request.gap_x > i64::from(MAX_IMPORT_DIMENSION)
        || request.gap_y > i64::from(MAX_IMPORT_DIMENSION)
        || request.border_x > i64::from(MAX_IMPORT_DIMENSION)
        || request.border_y > i64::from(MAX_IMPORT_DIMENSION)
    {
        return Err(AppError::new(
            "validation",
            "작업 시트 간격과 테두리는 0~12,000px 범위여야 합니다.",
        ));
    }
    if !(1..=i64::from(MAX_IMPORT_DIMENSION)).contains(&request.max_sheet_width)
        || !(1..=i64::from(MAX_IMPORT_DIMENSION)).contains(&request.max_sheet_height)
    {
        return Err(AppError::new(
            "validation",
            "작업 시트의 최대 가로·세로 크기는 1~12,000px 범위여야 합니다.",
        ));
    }
    Ok(())
}

fn validated_page_plan(
    item_count: usize,
    request: &ExportEditSheetRequest,
) -> AppResult<PageSplitPlan> {
    if item_count > usize::try_from(MAX_SHEET_CELLS).unwrap_or(usize::MAX) {
        return Err(AppError::new(
            "validation",
            format!("작업 시트는 최대 {MAX_SHEET_CELLS}개 셀까지 내보낼 수 있습니다."),
        ));
    }
    let plan = split_pages(
        item_count,
        PageSplitSettings {
            cell_width: request.cell_width,
            cell_height: request.cell_height,
            columns: request.columns,
            gap_x: request.gap_x,
            gap_y: request.gap_y,
            border_x: request.border_x,
            border_y: request.border_y,
            max_sheet_width: request.max_sheet_width,
            max_sheet_height: request.max_sheet_height,
        },
    )?;
    validate_page_plan_workload(&plan)?;
    Ok(plan)
}

fn validate_page_plan_workload(plan: &PageSplitPlan) -> AppResult<()> {
    if plan.pages.len() > usize::try_from(MAX_SHEET_CELLS).unwrap_or(usize::MAX)
        || plan.placements.len() > usize::try_from(MAX_SHEET_CELLS).unwrap_or(usize::MAX)
    {
        return Err(AppError::new(
            "validation",
            "작업 시트 페이지 또는 셀 수가 지원 범위를 벗어났습니다.",
        ));
    }

    let mut total_page_pixels = 0_u64;
    for page in &plan.pages {
        let width = u32::try_from(page.width)
            .map_err(|_| AppError::new("validation", "작업 시트 너비가 올바르지 않습니다."))?;
        let height = u32::try_from(page.height)
            .map_err(|_| AppError::new("validation", "작업 시트 높이가 올바르지 않습니다."))?;
        validate_import_dimensions(width, height)?;
        let page_pixels = u64::from(width)
            .checked_mul(u64::from(height))
            .ok_or_else(|| {
                AppError::new("validation", "작업 시트 페이지의 픽셀 수가 너무 큽니다.")
            })?;
        total_page_pixels = total_page_pixels.checked_add(page_pixels).ok_or_else(|| {
            AppError::new(
                "validation",
                "작업 시트 페이지의 전체 픽셀 수가 너무 큽니다.",
            )
        })?;
        if total_page_pixels > MAX_GIF_TOTAL_FRAME_PIXELS {
            return Err(AppError::new(
                "validation",
                "작업 시트 페이지의 전체 픽셀 수가 지원 범위를 벗어났습니다.",
            ));
        }
    }
    Ok(())
}

fn load_collection(connection: &Connection, collection_id: &str) -> AppResult<CollectionRecord> {
    connection
        .query_row(
            "SELECT id, name
             FROM collections
             WHERE id = ?1
               AND deleted_at IS NULL",
            params![collection_id],
            |row| {
                Ok(CollectionRecord {
                    id: row.get("id")?,
                    name: row.get("name")?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("작업 시트로 내보낼 모음을 찾을 수 없습니다."))
}

const ICON_RECORD_SELECT: &str = "SELECT
   i.id,
   i.display_name,
   i.shape,
   s.original_path_in_library,
   s.original_extension,
   s.sha256,
   s.is_animated,
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
 JOIN source_files s ON s.id = i.source_file_id
 JOIN crop_settings cs ON cs.icon_id = i.id
 LEFT JOIN icon_effect_recipes er ON er.icon_id = i.id
 LEFT JOIN icon_motion_recipes mr ON mr.icon_id = i.id";

fn load_icons(
    connection: &Connection,
    request: &ExportEditSheetRequest,
) -> AppResult<Vec<IconRecord>> {
    let selected_ids = request
        .selected_icon_ids
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let query = format!(
        "{ICON_RECORD_SELECT}
         WHERE i.collection_id = ?1
           AND i.deleted_at IS NULL
           AND i.icon_kind = 'image'
         ORDER BY i.order_index ASC, i.created_at ASC"
    );
    let mut statement = connection.prepare(&query)?;
    let icons = statement
        .query_map(params![request.collection_id], icon_record_from_row)?
        .collect::<Result<Vec<_>, _>>()?;

    if request.source == "selected_icons" && !selected_ids.is_empty() {
        Ok(icons
            .into_iter()
            .filter(|icon| selected_ids.contains(icon.id.as_str()))
            .collect())
    } else {
        Ok(icons)
    }
}

fn icon_record_from_row(row: &Row<'_>) -> rusqlite::Result<IconRecord> {
    Ok(IconRecord {
        id: row.get("id")?,
        display_name: row.get("display_name")?,
        shape: row.get("shape")?,
        source_path: row.get("original_path_in_library")?,
        source_extension: row.get("original_extension")?,
        source_hash: row.get("sha256")?,
        source_is_animated: row.get::<_, i64>("is_animated")? == 1,
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

pub(super) fn current_static_sheet_render_guard(
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
    let query = format!(
        "{ICON_RECORD_SELECT}
         WHERE i.id = ?1
           AND i.collection_id = ?2
           AND i.deleted_at IS NULL
           AND i.icon_kind = 'image'"
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
        Some(piece_id) => connection
            .query_row(
                "SELECT piece_index
                 FROM icon_pieces
                 WHERE id = ?1
                   AND icon_id = ?2",
                params![piece_id, icon_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .ok_or_else(|| AppError::not_found("작업 시트 원본 조각을 찾을 수 없습니다."))?,
        None => connection
            .query_row(
                "SELECT piece_index
                 FROM icon_pieces
                 WHERE icon_id = ?1
                 ORDER BY piece_index ASC
                 LIMIT 1",
                params![icon_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .ok_or_else(|| AppError::not_found("작업 시트 원본 조각을 찾을 수 없습니다."))?,
    };
    let piece_index = usize::try_from(piece_index)
        .map_err(|_| AppError::new("validation", "작업 시트 조각 번호가 올바르지 않습니다."))?;
    let render_recipe_hash =
        static_sheet_render_recipe_hash(&icon, piece_index, cell_width, cell_height)?;
    Ok((icon.source_hash, render_recipe_hash))
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

fn render_icon_items(
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

    for (piece_position, piece_image) in split.into_iter().enumerate() {
        let piece = pieces
            .iter()
            .find(|piece| piece.piece_index as usize == piece_position);
        let render_hash = sha256_hex(&png_bytes_from_rgba(&piece_image)?);
        let render_recipe_hash =
            static_sheet_render_recipe_hash(icon, piece_position, cell_width, cell_height)?;
        items.push(RenderedSheetItem {
            icon_id: icon.id.clone(),
            piece_id: piece.map(|piece| piece.id.clone()),
            display_name: icon.display_name.clone(),
            alt: piece
                .map(|piece| piece.alt_text.clone())
                .unwrap_or_default(),
            icon_type: icon.shape.clone(),
            source_hash: Some(icon.source_hash.clone()),
            render_hash,
            render_recipe_hash,
            image: piece_image,
        });
    }

    Ok(items)
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

fn crop_and_resize(
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

fn render_sheet_page(
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
    let label = number.to_string();
    let mut cursor_x = x.max(0) as u32;
    let y = y.max(0) as u32;
    for character in label.chars() {
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

fn output_directory(
    paths: &AppPaths,
    request: &ExportEditSheetRequest,
    collection_name: &str,
) -> AppResult<PathBuf> {
    let run_name = format!("{}-{}", sanitize_name(collection_name), timestamp_suffix());
    let output_root = request
        .output_directory
        .as_ref()
        .map(|path| PathBuf::from(path.trim()).join(&run_name))
        .unwrap_or_else(|| {
            paths
                .root
                .join("sheet_exports")
                .join("static")
                .join(&run_name)
        });
    fs::create_dir_all(&output_root)?;
    Ok(output_root)
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

fn normalized_background(value: &str) -> String {
    match value {
        "checker" | "white" | "black" => value.to_string(),
        _ => "transparent".to_string(),
    }
}

fn sanitize_name(value: &str) -> String {
    let name = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    if name.trim_matches('_').is_empty() {
        "sheet".to_string()
    } else {
        name
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(64);
    for byte in digest {
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}

fn timestamp_suffix() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

fn now_iso_like() -> String {
    format!("{}Z", timestamp_suffix())
}

fn default_sheet_source() -> String {
    "current_collection".to_string()
}

fn default_background() -> String {
    "transparent".to_string()
}

fn default_true() -> bool {
    true
}

fn default_max_sheet_size() -> i64 {
    2048
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};
    use rusqlite::Connection;

    use crate::db::migrations;
    use crate::db::repositories::collections::create_collection;
    use crate::db::repositories::editor::{
        apply_icon_crop, update_icon_effects, update_icon_text_overlay,
    };
    use crate::db::repositories::imports::import_image_files;
    use crate::imaging::effects::{EffectRecipe, EffectStep, ToneMode, EFFECT_RECIPE_VERSION};
    use crate::imaging::export_render::{
        render_icon_export, ExportCropRect, ExportRenderPiece, ExportRenderRequest,
    };
    use crate::imaging::text_overlay::text_overlay_from_fields;
    use crate::imaging::transform::ImageTransform;
    use crate::models::{
        ApplyIconCropPayload, ImportImageFilePayload, UpdateIconEffectsPayload,
        UpdateIconTextOverlayPayload,
    };
    use crate::paths::AppPaths;
    use crate::sheet::manifest::read_static_manifest;

    use super::{
        crop_and_resize, export_edit_sheet, png_bytes_from_rgba, sha256_hex,
        ExportEditSheetRequest, GuideLabelOptions,
    };

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
        AppPaths::prepare(std::env::temp_dir().join(format!("pmtconcon-sheet-export-{suffix}")))
            .unwrap()
    }

    fn png_bytes() -> Vec<u8> {
        let image = ImageBuffer::from_pixel(20, 20, Rgba([0, 255, 0, 96]));
        let mut cursor = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image)
            .write_to(&mut cursor, ImageFormat::Png)
            .unwrap();
        cursor.into_inner()
    }

    fn asymmetric_png_bytes() -> Vec<u8> {
        let image = ImageBuffer::from_fn(3, 2, |x, y| Rgba([(y * 3 + x) as u8, 0, 0, 255]));
        let mut cursor = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image)
            .write_to(&mut cursor, ImageFormat::Png)
            .unwrap();
        cursor.into_inner()
    }

    fn text_effect_png_bytes() -> Vec<u8> {
        let image = ImageBuffer::from_fn(64, 64, |x, y| {
            if ((x / 8) + (y / 8)) % 2 == 0 {
                Rgba([30, 120, 230, 255])
            } else {
                Rgba([230, 80, 40, 255])
            }
        });
        let mut cursor = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image)
            .write_to(&mut cursor, ImageFormat::Png)
            .unwrap();
        cursor.into_inner()
    }

    #[test]
    fn static_sheet_crop_rejects_extreme_geometry_before_allocation() {
        let source = ImageBuffer::from_pixel(10, 10, Rgba([255, 0, 0, 255]));
        assert!(crop_and_resize(&source, 0.0, 0.0, f64::MAX, 20.0, 20, 20).is_err());
        assert!(crop_and_resize(&source, f64::MIN, 0.0, 20.0, 20.0, 20, 20).is_err());
        assert!(crop_and_resize(&source, 0.0, 0.0, 20.0, 20.0, i64::MAX, 20).is_err());
    }

    #[test]
    fn edit_sheet_export_rejects_oversized_layout_before_output() {
        let mut connection = connection();
        let paths = temp_paths();
        let collection =
            create_collection(&mut connection, Some("oversized sheet export".to_string())).unwrap();
        import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![ImportImageFilePayload {
                original_filename: "cell.png".to_string(),
                bytes: png_bytes(),
            }],
        )
        .unwrap();
        let output_parent = paths.root.join("oversized-output");
        let request = |gap_x, border_x, border_y| ExportEditSheetRequest {
            collection_id: collection.id.clone(),
            selected_icon_ids: Vec::new(),
            source: "current_collection".to_string(),
            cell_width: 1,
            cell_height: 1,
            columns: 1,
            gap_x,
            gap_y: 0,
            border_x,
            border_y,
            background: "transparent".to_string(),
            include_clean_sheet: true,
            include_guide_sheet: false,
            include_manifest: false,
            label_options: None,
            max_sheet_width: 1,
            max_sheet_height: 1,
            output_directory: Some(output_parent.to_string_lossy().to_string()),
            open_output_folder: false,
        };

        let huge_border_error =
            export_edit_sheet(&connection, &paths, request(0, 6_000, 6_000)).unwrap_err();
        assert_eq!(huge_border_error.code, "validation");
        assert!(!output_parent.exists());

        let overflow_error =
            export_edit_sheet(&connection, &paths, request(i64::MAX, 0, 0)).unwrap_err();
        assert_eq!(overflow_error.code, "validation");
        assert!(!output_parent.exists());

        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn edit_sheet_export_writes_clean_guide_and_manifest_with_alpha() {
        let mut connection = connection();
        let paths = temp_paths();
        let collection =
            create_collection(&mut connection, Some("sheet export".to_string())).unwrap();
        import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![ImportImageFilePayload {
                original_filename: "cell.png".to_string(),
                bytes: png_bytes(),
            }],
        )
        .unwrap();

        let result = export_edit_sheet(
            &connection,
            &paths,
            ExportEditSheetRequest {
                collection_id: collection.id,
                selected_icon_ids: Vec::new(),
                source: "current_collection".to_string(),
                cell_width: 20,
                cell_height: 20,
                columns: 1,
                gap_x: 8,
                gap_y: 8,
                border_x: 16,
                border_y: 16,
                background: "transparent".to_string(),
                include_clean_sheet: true,
                include_guide_sheet: true,
                include_manifest: true,
                label_options: Some(GuideLabelOptions {
                    cell_number: true,
                    icon_name: false,
                    alt_value: false,
                    export_number: false,
                }),
                max_sheet_width: 2048,
                max_sheet_height: 2048,
                output_directory: None,
                open_output_folder: false,
            },
        )
        .unwrap();

        assert_eq!(result.item_count, 1);
        assert_eq!(result.page_count, 1);
        assert!(std::path::Path::new(result.manifest_path.as_ref().unwrap()).is_file());
        let clean = image::open(&result.clean_sheet_paths[0])
            .unwrap()
            .to_rgba8();
        assert_eq!((clean.width(), clean.height()), (52, 52));
        assert_eq!(clean.get_pixel(0, 0).0[3], 0);
        assert_eq!(clean.get_pixel(16, 16).0[3], 96);
        let guide = image::open(&result.guide_sheet_paths[0])
            .unwrap()
            .to_rgba8();
        assert_ne!(guide.get_pixel(16, 16), clean.get_pixel(16, 16));

        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn edit_sheet_export_uses_the_same_transform_recipe_as_preview_and_export() {
        let mut connection = connection();
        let paths = temp_paths();
        let collection =
            create_collection(&mut connection, Some("transform sheet".to_string())).unwrap();
        let imported = import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![ImportImageFilePayload {
                original_filename: "asymmetric.png".to_string(),
                bytes: asymmetric_png_bytes(),
            }],
        )
        .unwrap();
        let icon = &imported.imported_icons[0];
        apply_icon_crop(
            &mut connection,
            &paths,
            &collection.id,
            ApplyIconCropPayload {
                icon_id: icon.id.clone(),
                shape: "single".to_string(),
                crop_mode: "fixed".to_string(),
                crop_x: 0.0,
                crop_y: 0.0,
                crop_w: 3.0,
                crop_h: 2.0,
                preset_position: "center".to_string(),
                cell_width: 2,
                cell_height: 3,
                transform_quarter_turns: 1,
                transform_flip_horizontal: false,
                transform_flip_vertical: false,
                piece_ids: vec![icon.pieces[0].id.clone()],
                gif_loop_mode: "preserve".to_string(),
                gif_loop_count: None,
            },
        )
        .unwrap();

        let result = export_edit_sheet(
            &connection,
            &paths,
            ExportEditSheetRequest {
                collection_id: collection.id,
                selected_icon_ids: Vec::new(),
                source: "current_collection".to_string(),
                cell_width: 2,
                cell_height: 3,
                columns: 1,
                gap_x: 0,
                gap_y: 0,
                border_x: 0,
                border_y: 0,
                background: "transparent".to_string(),
                include_clean_sheet: true,
                include_guide_sheet: false,
                include_manifest: true,
                label_options: None,
                max_sheet_width: 2048,
                max_sheet_height: 2048,
                output_directory: None,
                open_output_folder: false,
            },
        )
        .unwrap();

        let clean = image::open(&result.clean_sheet_paths[0])
            .unwrap()
            .to_rgba8();
        assert_eq!((clean.width(), clean.height()), (2, 3));
        let values = clean.pixels().map(|pixel| pixel.0[0]).collect::<Vec<_>>();
        assert_eq!(values, vec![3, 0, 4, 1, 5, 2]);

        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn edit_sheet_text_and_effect_pixels_match_shared_preview_export_and_manifest() {
        let mut connection = connection();
        let paths = temp_paths();
        let collection =
            create_collection(&mut connection, Some("text effect sheet".to_string())).unwrap();
        let imported = import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![ImportImageFilePayload {
                original_filename: "text-effect.png".to_string(),
                bytes: text_effect_png_bytes(),
            }],
        )
        .unwrap();
        let icon = &imported.imported_icons[0];

        apply_icon_crop(
            &mut connection,
            &paths,
            &collection.id,
            ApplyIconCropPayload {
                icon_id: icon.id.clone(),
                shape: "single".to_string(),
                crop_mode: "fixed".to_string(),
                crop_x: 0.0,
                crop_y: 0.0,
                crop_w: 64.0,
                crop_h: 64.0,
                preset_position: "center".to_string(),
                cell_width: 64,
                cell_height: 64,
                transform_quarter_turns: 0,
                transform_flip_horizontal: false,
                transform_flip_vertical: false,
                piece_ids: vec![icon.pieces[0].id.clone()],
                gif_loop_mode: "preserve".to_string(),
                gif_loop_count: None,
            },
        )
        .unwrap();

        let font_path = [
            r"C:\Windows\Fonts\NotoSansKR-Regular.otf",
            r"C:\Windows\Fonts\NotoSansKR-Regular.ttf",
            r"C:\Windows\Fonts\NotoSansCJKkr-Regular.otf",
            r"C:\Windows\Fonts\NanumGothic.ttf",
            r"C:\Windows\Fonts\D2Coding.ttf",
        ]
        .iter()
        .find(|candidate| Path::new(candidate).is_file())
        .expect("text overlay regression test requires a supported Windows font")
        .to_string();
        let text_overlay = text_overlay_from_fields(
            true,
            Some("A".to_string()),
            Some(font_path.clone()),
            Some(36.0),
            Some(0.5),
            Some(0.55),
            Some("#FFE800".to_string()),
            Some("#000000".to_string()),
            Some(1.0),
        )
        .unwrap();
        let text_saved = update_icon_text_overlay(
            &mut connection,
            &paths,
            &collection.id,
            UpdateIconTextOverlayPayload {
                icon_id: icon.id.clone(),
                enabled: true,
                text: "A".to_string(),
                font_path: Some(font_path),
                font_size: 36.0,
                x: 0.5,
                y: 0.55,
                color: "#FFE800".to_string(),
                stroke_color: "#000000".to_string(),
                stroke_width: 1.0,
            },
        )
        .unwrap();
        let text_only = image::open(text_saved.icon.current_preview_url.as_ref().unwrap())
            .unwrap()
            .to_rgba8();

        let recipe = EffectRecipe {
            version: EFFECT_RECIPE_VERSION,
            effects: vec![EffectStep::Tone {
                id: "grayscale-sheet-test".to_string(),
                enabled: true,
                mode: ToneMode::Grayscale,
                amount: 100,
            }],
        };
        let effect_saved = update_icon_effects(
            &mut connection,
            &paths,
            &collection.id,
            UpdateIconEffectsPayload {
                icon_id: icon.id.clone(),
                expected_revision: 0,
                recipe: recipe.clone(),
            },
        )
        .unwrap();
        let shared_preview = image::open(effect_saved.icon.current_preview_url.as_ref().unwrap())
            .unwrap()
            .to_rgba8();
        assert_ne!(shared_preview.as_raw(), text_only.as_raw());

        let source_path: String = connection
            .query_row(
                "SELECT s.original_path_in_library
                 FROM source_files s
                 JOIN icons i ON i.source_file_id = s.id
                 WHERE i.id = ?1",
                [&icon.id],
                |row| row.get(0),
            )
            .unwrap();
        let source_hash: String = connection
            .query_row(
                "SELECT s.sha256
                 FROM source_files s
                 JOIN icons i ON i.source_file_id = s.id
                 WHERE i.id = ?1",
                [&icon.id],
                |row| row.get(0),
            )
            .unwrap();
        let shared_export_dir = paths.root.join("shared-export");
        let shared_export_paths = render_icon_export(ExportRenderRequest {
            source_path: Path::new(&source_path),
            source_extension: "png",
            shape: "single",
            crop: ExportCropRect {
                x: 0.0,
                y: 0.0,
                width: 64.0,
                height: 64.0,
            },
            cell_width: 64,
            cell_height: 64,
            transform: ImageTransform::new(0, false, false).unwrap(),
            output_format: "png",
            resize_filter: "lanczos3",
            gif_loop_mode: "preserve",
            gif_loop_count: None,
            source_gif_loop_mode: "preserve",
            source_gif_loop_count: None,
            text_overlay,
            effects: recipe,
            motion: crate::imaging::motion::MotionRecipe::default(),
            output_dir: &shared_export_dir,
            pieces: &[ExportRenderPiece {
                piece_index: 0,
                file_name: "piece.png".to_string(),
            }],
        })
        .unwrap();
        let shared_export = image::open(&shared_export_paths[0]).unwrap().to_rgba8();
        assert_eq!(shared_export.as_raw(), shared_preview.as_raw());

        let result = export_edit_sheet(
            &connection,
            &paths,
            ExportEditSheetRequest {
                collection_id: collection.id,
                selected_icon_ids: Vec::new(),
                source: "current_collection".to_string(),
                cell_width: 64,
                cell_height: 64,
                columns: 1,
                gap_x: 0,
                gap_y: 0,
                border_x: 0,
                border_y: 0,
                background: "transparent".to_string(),
                include_clean_sheet: true,
                include_guide_sheet: false,
                include_manifest: true,
                label_options: None,
                max_sheet_width: 2048,
                max_sheet_height: 2048,
                output_directory: None,
                open_output_folder: false,
            },
        )
        .unwrap();

        let sheet = image::open(&result.clean_sheet_paths[0])
            .unwrap()
            .to_rgba8();
        assert_eq!(sheet.as_raw(), shared_preview.as_raw());
        assert_eq!(sheet.as_raw(), shared_export.as_raw());

        let manifest =
            read_static_manifest(Path::new(result.manifest_path.as_ref().unwrap())).unwrap();
        assert_eq!(manifest.items.len(), 1);
        assert_eq!(
            manifest.items[0].source_hash.as_deref(),
            Some(source_hash.as_str())
        );
        let sheet_hash = sha256_hex(&png_bytes_from_rgba(&sheet).unwrap());
        assert_eq!(
            manifest.items[0].render_hash.as_deref(),
            Some(sheet_hash.as_str())
        );

        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn edit_sheet_export_selected_icons_only_in_grid_order() {
        let mut connection = connection();
        let paths = temp_paths();
        let collection =
            create_collection(&mut connection, Some("selected sheet export".to_string())).unwrap();
        let import_result = import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![
                ImportImageFilePayload {
                    original_filename: "first.png".to_string(),
                    bytes: png_bytes(),
                },
                ImportImageFilePayload {
                    original_filename: "second.png".to_string(),
                    bytes: png_bytes(),
                },
            ],
        )
        .unwrap();
        let second_id = import_result.imported_icons[1].id.clone();

        let result = export_edit_sheet(
            &connection,
            &paths,
            ExportEditSheetRequest {
                collection_id: collection.id,
                selected_icon_ids: vec![second_id],
                source: "selected_icons".to_string(),
                cell_width: 20,
                cell_height: 20,
                columns: 1,
                gap_x: 0,
                gap_y: 0,
                border_x: 0,
                border_y: 0,
                background: "transparent".to_string(),
                include_clean_sheet: true,
                include_guide_sheet: false,
                include_manifest: true,
                label_options: Some(GuideLabelOptions {
                    cell_number: true,
                    icon_name: true,
                    alt_value: true,
                    export_number: true,
                }),
                max_sheet_width: 2048,
                max_sheet_height: 2048,
                output_directory: None,
                open_output_folder: false,
            },
        )
        .unwrap();

        assert_eq!(result.item_count, 1);
        let manifest = std::fs::read_to_string(result.manifest_path.as_ref().unwrap()).unwrap();
        assert!(manifest.contains("second"));
        assert!(!manifest.contains("first"));

        std::fs::remove_dir_all(paths.root).unwrap();
    }
}
