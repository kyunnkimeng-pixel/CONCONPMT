use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use image::ImageFormat;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::imaging::import_limits::{
    validate_import_dimensions, MAX_GIF_TOTAL_FRAME_PIXELS, MAX_IMPORT_DIMENSION,
};
use crate::paths::AppPaths;

pub use super::composer::GuideLabelOptions;
#[cfg(test)]
use super::composer::{crop_and_resize, sha256_hex};
use super::composer::{
    load_static_render_targets, normalized_background, render_icon_items, render_sheet_page,
    StaticRenderSelection,
};
use super::grid::{split_pages, PageSplitPlan, PageSplitSettings, MAX_SHEET_CELLS};
#[cfg(test)]
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
    let icons = load_static_render_targets(
        connection,
        &request.collection_id,
        StaticRenderSelection::LegacyWorkSheet {
            source: &request.source,
            selected_icon_ids: &request.selected_icon_ids,
        },
    )?;
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
            visual_source: Some(item.visual_source.clone()),
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
