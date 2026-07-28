use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::fmt::Write as FmtWrite;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Cursor};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use image::codecs::gif::{GifDecoder, GifEncoder, Repeat as ImageGifRepeat};
use image::imageops::{self, FilterType};
use image::{AnimationDecoder, Delay, DynamicImage, Frame, ImageFormat, Rgba, RgbaImage};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::db::repositories::ai as ai_repository;
use crate::db::repositories::optimization::{insert_variant, NewProcessedAssetVariant};
use crate::error::{AppError, AppResult};
use crate::ids::create_id;
use crate::imaging::effects::{apply_effect_recipe, parse_effect_recipe_json, EffectRecipe};
use crate::imaging::export_render::ExportCropRect;
use crate::imaging::gif_pipeline::{
    is_pingpong_loop_mode, output_repeat_for_settings, pingpong_sequence, pingpong_sequence_len,
    GifOutputRepeat,
};
use crate::imaging::import_limits::{
    decode_import_image, read_import_file_bytes, validate_crop_rect, validate_gif_workload,
    validate_import_dimensions, validate_import_file_size, ValidatedCropRect, MAX_GIF_FRAMES,
    MAX_GIF_TOTAL_FRAME_PIXELS, MAX_IMPORT_DIMENSION, MAX_IMPORT_FILE_BYTES,
};
use crate::imaging::motion::{
    apply_motion_recipe, parse_motion_recipe_json, static_motion_schedule, MotionFrameContext,
    MotionFrameTiming, MotionRecipe,
};
use crate::imaging::text_overlay::{
    apply_text_overlay, text_overlay_from_fields, TextOverlayRenderSpec,
};
use crate::imaging::transform::{apply_image_transform, source_viewport_geometry, ImageTransform};
use crate::models::ImportImageFilePayload;
use crate::optimization::analyzer::{analyze_file, load_target, move_temp_file};
use crate::optimization::cache::{hash_text, render_recipe_crop_hash};
use crate::paths::AppPaths;

use super::grid::{split_pages, PageCellPlacement, PageSplitSettings};
use super::manifest::{
    read_gif_manifest_bytes, validate_gif_manifest, write_gif_manifest, GifFrameManifestItem,
    GifFrameSheetManifest, GifFrameSheetPage, ManifestVisualSource, APP_NAME,
    GIF_FRAME_SHEET_SCHEMA, LEGACY_GIF_FRAME_SHEET_SCHEMA,
};
use super::path_string;

const MAX_REIMPORT_TOTAL_ENCODED_BYTES: usize = MAX_IMPORT_FILE_BYTES;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GifFrameSheetSettings {
    pub frame_cell_width: i64,
    pub frame_cell_height: i64,
    pub columns: i64,
    pub frames_per_page: Option<i64>,
    #[serde(default)]
    pub gap_x: i64,
    #[serde(default)]
    pub gap_y: i64,
    #[serde(default)]
    pub border_x: i64,
    #[serde(default)]
    pub border_y: i64,
    #[serde(default = "default_max_sheet_size")]
    pub max_sheet_width: i64,
    #[serde(default = "default_max_sheet_size")]
    pub max_sheet_height: i64,
    #[serde(default = "default_background")]
    pub background: String,
    #[serde(default = "default_true")]
    pub include_clean_sheet: bool,
    #[serde(default = "default_true")]
    pub include_guide_sheet: bool,
    #[serde(default = "default_true")]
    pub include_manifest: bool,
    pub output_directory: Option<String>,
    #[serde(default)]
    pub open_output_folder: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzeGifFrameSheetExportRequest {
    pub icon_id: String,
    pub settings: GifFrameSheetSettings,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GifFrameSheetExportAnalysis {
    pub icon_id: String,
    pub display_name: String,
    pub source_format: String,
    pub frame_count: i64,
    pub duration_ms: i64,
    pub loop_mode: String,
    pub loop_count: Option<i64>,
    pub page_count: i64,
    pub sheet_width: i64,
    pub sheet_height: i64,
    pub columns: i64,
    pub rows_per_page: i64,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GifFrameSheetExportRequest {
    pub icon_id: String,
    pub settings: GifFrameSheetSettings,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GifFrameSheetExportResult {
    pub frame_sheet_paths: Vec<String>,
    pub guide_sheet_paths: Vec<String>,
    pub manifest_path: Option<String>,
    pub output_directory: String,
    pub frame_count: i64,
    pub page_count: i64,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidateGifFrameSheetReimportRequest {
    pub manifest_path: String,
    pub manifest_file: Option<ImportImageFilePayload>,
    #[serde(default)]
    pub edited_frame_sheet_paths: Vec<String>,
    #[serde(default)]
    pub edited_frame_sheet_files: Vec<ImportImageFilePayload>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GifFrameSheetReimportValidation {
    pub frame_count: i64,
    pub detected_frame_count: i64,
    pub page_count: i64,
    pub missing_pages: Vec<i64>,
    pub wrong_dimension_pages: Vec<i64>,
    pub loop_mode: String,
    pub loop_count: Option<i64>,
    pub duration_ms: i64,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GifFrameSheetReimportRequest {
    pub manifest_path: String,
    pub manifest_file: Option<ImportImageFilePayload>,
    #[serde(default)]
    pub edited_frame_sheet_paths: Vec<String>,
    #[serde(default)]
    pub edited_frame_sheet_files: Vec<ImportImageFilePayload>,
    pub target_icon_id: String,
    #[serde(default = "default_true")]
    pub create_variant: bool,
    #[serde(default)]
    pub set_active_variant: bool,
    pub target_profile_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GifFrameSheetReimportResult {
    pub variant_id: Option<String>,
    pub output_path: Option<String>,
    pub frame_count: i64,
    pub duration_ms: i64,
    pub active_variant_set: bool,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct GifFrameTiming {
    pub frame_index: i64,
    pub duration_ms: i64,
    pub disposal_method: Option<String>,
    pub source_frame_hash: Option<String>,
}

#[derive(Debug)]
struct GifIconRecord {
    id: String,
    original_source_file_id: String,
    original_source_hash: String,
    original_lineage_id: String,
    original_lineage_generation: i64,
    active_version_id: Option<String>,
    source_file_id: String,
    display_name: String,
    source_path: String,
    source_extension: String,
    source_hash: String,
    source_is_animated: bool,
    source_gif_loop_mode: String,
    source_gif_loop_count: Option<i64>,
    shape: String,
    crop_x: f64,
    crop_y: f64,
    crop_w: f64,
    crop_h: f64,
    gif_loop_mode: String,
    gif_loop_count: Option<i64>,
    transform: ImageTransform,
    text_overlay_enabled: bool,
    text_overlay_text: String,
    text_overlay_font_path: Option<String>,
    text_overlay_font_size: f64,
    text_overlay_x: f64,
    text_overlay_y: f64,
    text_overlay_color: String,
    text_overlay_stroke_color: String,
    text_overlay_stroke_width: f64,
    effects: EffectRecipe,
    motion: MotionRecipe,
}

#[derive(Debug, Clone)]
struct DecodedFrame {
    image: RgbaImage,
    duration_ms: i64,
    source_frame_hash: String,
}

#[derive(Debug, Clone)]
struct SourceTimelineFrame {
    image: Arc<RgbaImage>,
    duration_ms: i64,
}

#[derive(Debug)]
struct ReimportValidationInternal {
    manifest: GifFrameSheetManifest,
    page_images: HashMap<i64, RgbaImage>,
    public: GifFrameSheetReimportValidation,
}

#[derive(Debug)]
enum PageImageSource {
    Path(PathBuf),
    Bytes(Vec<u8>),
}

pub fn analyze_gif_frame_sheet_export(
    connection: &Connection,
    request: AnalyzeGifFrameSheetExportRequest,
) -> AppResult<GifFrameSheetExportAnalysis> {
    validate_export_settings(&request.settings)?;
    let icon = load_gif_icon(connection, &request.icon_id)?;
    let decoded = decode_rendered_frames(&icon, &request.settings)?;
    let split = page_split_for_settings(decoded.len(), &request.settings)?;
    let mut warnings = split.warnings.clone();
    warnings.extend(analysis_warnings(&icon, &request.settings));

    let first_page = split.pages.first();
    Ok(GifFrameSheetExportAnalysis {
        icon_id: icon.id.clone(),
        display_name: icon.display_name.clone(),
        source_format: icon.source_extension.clone(),
        frame_count: decoded.len() as i64,
        duration_ms: decoded.iter().map(|frame| frame.duration_ms).sum(),
        loop_mode: effective_loop_mode(&icon),
        loop_count: effective_loop_count(&icon),
        page_count: split.pages.len() as i64,
        sheet_width: first_page.map(|page| page.width).unwrap_or(0),
        sheet_height: first_page.map(|page| page.height).unwrap_or(0),
        columns: split.columns_per_page,
        rows_per_page: split.rows_per_page,
        warnings,
    })
}

pub fn export_gif_frame_sheet(
    connection: &Connection,
    paths: &AppPaths,
    request: GifFrameSheetExportRequest,
) -> AppResult<GifFrameSheetExportResult> {
    validate_export_settings(&request.settings)?;
    let icon = load_gif_icon(connection, &request.icon_id)?;
    let decoded = decode_rendered_frames(&icon, &request.settings)?;
    let split = page_split_for_settings(decoded.len(), &request.settings)?;
    let mut warnings = split.warnings.clone();
    warnings.extend(analysis_warnings(&icon, &request.settings));

    let output_root = gif_output_directory(paths, &request.settings, &icon)?;
    let mut frame_sheet_paths = Vec::new();
    let mut guide_sheet_paths = Vec::new();

    for page in &split.pages {
        let page_placements = split
            .placements
            .iter()
            .filter(|placement| placement.page_index == page.page_index)
            .collect::<Vec<_>>();
        let clean_file = format!("frames_sheet_{:03}.png", page.page_index + 1);
        let guide_file = format!("frames_guide_{:03}.png", page.page_index + 1);

        if request.settings.include_clean_sheet {
            let clean = render_frame_sheet_page(
                &decoded,
                &page_placements,
                page.width,
                page.height,
                &request.settings.background,
                false,
            )?;
            let clean_path = output_root.join(&clean_file);
            save_png_atomic(&clean_path, &clean)?;
            frame_sheet_paths.push(path_string(&clean_path));
        }

        if request.settings.include_guide_sheet {
            let guide = render_frame_sheet_page(
                &decoded,
                &page_placements,
                page.width,
                page.height,
                &request.settings.background,
                true,
            )?;
            let guide_path = output_root.join(&guide_file);
            save_png_atomic(&guide_path, &guide)?;
            guide_sheet_paths.push(path_string(&guide_path));
        }
    }

    let manifest_path = if request.settings.include_manifest {
        let timings = decoded
            .iter()
            .enumerate()
            .map(|(index, frame)| GifFrameTiming {
                frame_index: index as i64,
                duration_ms: frame.duration_ms,
                disposal_method: None,
                source_frame_hash: Some(frame.source_frame_hash.clone()),
            })
            .collect::<Vec<_>>();
        let mut manifest =
            build_gif_frame_manifest_plan(&icon, &request.settings, &timings, &split)?;
        manifest.created_at = now_iso_like();
        let path = output_root.join("frames_manifest.json");
        write_gif_manifest(&path, &manifest)?;
        Some(path_string(&path))
    } else {
        None
    };

    if request.settings.open_output_folder {
        crate::export::open_export_path(&path_string(&output_root))?;
    }

    Ok(GifFrameSheetExportResult {
        frame_sheet_paths,
        guide_sheet_paths,
        manifest_path,
        output_directory: path_string(&output_root),
        frame_count: decoded.len() as i64,
        page_count: split.pages.len() as i64,
        warnings,
    })
}

pub fn validate_gif_frame_sheet_reimport(
    request: ValidateGifFrameSheetReimportRequest,
) -> AppResult<GifFrameSheetReimportValidation> {
    Ok(validate_reimport_inputs(
        request.manifest_path,
        request.manifest_file,
        request.edited_frame_sheet_paths,
        request.edited_frame_sheet_files,
    )?
    .public)
}

pub fn reimport_gif_frame_sheet(
    connection: &Connection,
    paths: &AppPaths,
    request: GifFrameSheetReimportRequest,
) -> AppResult<GifFrameSheetReimportResult> {
    if !request.create_variant {
        return Err(AppError::new(
            "validation",
            "GIF 프레임 시트 다시 가져오기는 원본을 덮어쓰지 않고 새 processed variant를 만들어야 합니다.",
        ));
    }

    let validation = validate_reimport_inputs(
        request.manifest_path,
        request.manifest_file,
        request.edited_frame_sheet_paths,
        request.edited_frame_sheet_files,
    )?;
    let mut warnings = validation.public.warnings.clone();
    let errors = validation.public.errors.clone();
    if !errors.is_empty() {
        return Ok(GifFrameSheetReimportResult {
            variant_id: None,
            output_path: None,
            frame_count: validation.manifest.frame_count,
            duration_ms: validation.manifest.duration_ms,
            active_variant_set: false,
            warnings,
            errors,
        });
    }

    if request.target_icon_id != validation.manifest.icon_id {
        return Ok(GifFrameSheetReimportResult {
            variant_id: None,
            output_path: None,
            frame_count: validation.manifest.frame_count,
            duration_ms: validation.manifest.duration_ms,
            active_variant_set: false,
            warnings,
            errors: vec![
                "선택한 대상 GIF 아이콘과 매니페스트의 icon_id가 달라 다시 가져오기를 중단했습니다."
                    .to_string(),
            ],
        });
    }

    let target_icon = load_gif_icon(connection, &request.target_icon_id)?;
    validate_manifest_visual_source(&validation.manifest, &target_icon)?;

    let settings_json = serde_json::json!({
        "source": "gif_frame_sheet_reimport",
        "schema": validation.manifest.schema,
        "renderRecipeHash": validation.manifest.render_recipe_hash,
        "frameCellWidth": validation.manifest.frame_cell_width,
        "frameCellHeight": validation.manifest.frame_cell_height,
        "frameCount": validation.manifest.frame_count,
    })
    .to_string();

    let mut profile_id = None;
    let mut piece_id = None;
    let mut crop_hash = hash_text(&[
        "gif_frame_sheet_reimport".to_string(),
        target_icon.id.clone(),
        validation.manifest.frame_count.to_string(),
    ]);
    let mut profile_hash = "gif_frame_sheet_reimport".to_string();
    let mut source_hash = validation
        .manifest
        .source_hash
        .clone()
        .unwrap_or_else(|| target_icon.source_hash.clone());
    let source_file_id = Some(target_icon.source_file_id.clone());
    let mut active_variant_set = false;

    if request.set_active_variant {
        if let Some(target_profile_id) = request.target_profile_id.as_deref() {
            match load_target(connection, &target_icon.id, target_profile_id, None) {
                Ok(target) => {
                    if target.shape != "single" {
                        warnings.push(
                            "현재 MVP는 single GIF 아이콘에만 GIF 프레임 시트 variant를 export 활성 항목으로 설정합니다."
                                .to_string(),
                        );
                    } else if target.cell_width != validation.manifest.frame_cell_width
                        || target.cell_height != validation.manifest.frame_cell_height
                    {
                        warnings.push(
                            "선택한 export profile의 셀 크기와 프레임 시트 셀 크기가 달라 active variant로 설정하지 않았습니다."
                                .to_string(),
                        );
                    } else if validation.manifest.source_hash.as_deref()
                        != Some(target.source_hash.as_str())
                        || validation.manifest.render_recipe_hash.as_deref()
                            != Some(target.crop_hash.as_str())
                    {
                        warnings.push(
                            "프레임 시트를 내보낸 뒤 원본 또는 crop·회전·반전·텍스트·효과·반복 recipe가 바뀌어 active variant로 설정하지 않았습니다. 파일 variant만 만들었습니다."
                                .to_string(),
                        );
                    } else {
                        profile_id = Some(target.profile.id.clone());
                        piece_id = Some(target.piece_id.clone());
                        crop_hash = target.crop_hash;
                        profile_hash = target.profile_hash;
                        source_hash = target.source_hash;
                    }
                }
                Err(error) => warnings.push(format!(
                    "active variant 대상 profile을 확인할 수 없어 파일 variant만 만들었습니다: {}",
                    error.message
                )),
            }
        } else {
            warnings.push(
                "active variant로 설정하려면 export profile을 선택해야 합니다. 파일 variant만 만들었습니다."
                    .to_string(),
            );
        }
    }

    let frames = crop_reimport_frames(&validation.manifest, &validation.page_images)?;
    let repeat = repeat_from_manifest(&validation.manifest)?;
    let variant_id = create_id("variant");
    let output_dir = paths
        .processed_variants_dir
        .join("gif_frame_reimports")
        .join(&target_icon.id);
    fs::create_dir_all(&output_dir)?;
    let output_path = output_dir.join(format!("{variant_id}.gif"));
    let temp_path = output_path.with_extension("gif.tmp");
    if let Err(error) = write_gif_atomic(&temp_path, &output_path, frames, repeat) {
        cleanup_failed_gif_variant(&temp_path, &output_path, &output_dir);
        return Err(error);
    }

    let persisted_variant = (|| {
        let file_analysis = analyze_file(&output_path, "gif")?;
        let metadata = fs::metadata(&output_path)?;
        insert_variant(
            connection,
            &NewProcessedAssetVariant {
                id: variant_id.clone(),
                icon_id: target_icon.id.clone(),
                piece_id: piece_id.clone(),
                profile_id: profile_id.clone(),
                source_file_id,
                kind: "optimized_gif".to_string(),
                preset: Some("custom".to_string()),
                path: path_string(&output_path),
                format: "gif".to_string(),
                width: file_analysis.width,
                height: file_analysis.height,
                byte_size: i64::try_from(metadata.len()).unwrap_or(i64::MAX),
                frame_count: file_analysis.frame_count,
                duration_ms: file_analysis.duration_ms,
                loop_mode: file_analysis
                    .loop_mode
                    .or_else(|| Some(validation.manifest.loop_mode.clone())),
                settings_json: settings_json.clone(),
                source_hash,
                crop_hash,
                profile_hash,
                settings_hash: hash_text(&[settings_json]),
            },
        )
    })();
    let variant = match persisted_variant {
        Ok(variant) => variant,
        Err(error) => {
            cleanup_failed_gif_variant(&temp_path, &output_path, &output_dir);
            return Err(error);
        }
    };

    if request.set_active_variant && profile_id.is_some() && piece_id.is_some() {
        match crate::db::repositories::optimization::set_active_variant(connection, &variant.id) {
            Ok(_) => active_variant_set = true,
            Err(error) => warnings.push(format!(
                "processed variant 파일은 만들었지만 export 활성 항목으로 설정하지 못했습니다: {}",
                error.message
            )),
        }
    }

    Ok(GifFrameSheetReimportResult {
        variant_id: Some(variant.id),
        output_path: Some(path_string(&output_path)),
        frame_count: validation.manifest.frame_count,
        duration_ms: validation.manifest.duration_ms,
        active_variant_set,
        warnings,
        errors: Vec::new(),
    })
}

fn cleanup_failed_gif_variant(temp_path: &Path, output_path: &Path, output_dir: &Path) {
    let _ = fs::remove_file(temp_path);
    let _ = fs::remove_file(output_path);
    let is_empty = fs::read_dir(output_dir)
        .ok()
        .is_some_and(|mut entries| entries.next().is_none());
    if is_empty {
        let _ = fs::remove_dir(output_dir);
    }
}
fn build_gif_frame_manifest_plan(
    icon: &GifIconRecord,
    settings: &GifFrameSheetSettings,
    frames: &[GifFrameTiming],
    split: &super::grid::PageSplitPlan,
) -> AppResult<GifFrameSheetManifest> {
    let pages = split
        .pages
        .iter()
        .map(|page| GifFrameSheetPage {
            page_index: page.page_index,
            clean_sheet_file: format!("frames_sheet_{:03}.png", page.page_index + 1),
            guide_sheet_file: settings
                .include_guide_sheet
                .then(|| format!("frames_guide_{:03}.png", page.page_index + 1)),
            width: page.width,
            height: page.height,
        })
        .collect::<Vec<_>>();
    let frame_items = split
        .placements
        .iter()
        .map(|placement| {
            let timing = &frames[placement.item_index];
            GifFrameManifestItem {
                frame_index: timing.frame_index,
                sheet_file: format!("frames_sheet_{:03}.png", placement.page_index + 1),
                page_index: placement.page_index,
                row: placement.row,
                col: placement.col,
                x: placement.x,
                y: placement.y,
                w: placement.w,
                h: placement.h,
                duration_ms: timing.duration_ms,
                disposal_method: timing.disposal_method.clone(),
                source_frame_hash: timing.source_frame_hash.clone(),
            }
        })
        .collect::<Vec<_>>();

    Ok(GifFrameSheetManifest {
        schema: GIF_FRAME_SHEET_SCHEMA.to_string(),
        app: APP_NAME.to_string(),
        created_at: "generated-by-runtime".to_string(),
        icon_id: icon.id.clone(),
        source_file_id: Some(icon.source_file_id.clone()),
        source_hash: Some(icon.source_hash.clone()),
        visual_source: Some(ManifestVisualSource {
            original_source_file_id: icon.original_source_file_id.clone(),
            original_source_hash: icon.original_source_hash.clone(),
            original_lineage_id: icon.original_lineage_id.clone(),
            original_lineage_generation: icon.original_lineage_generation,
            effective_source_file_id: icon.source_file_id.clone(),
            effective_source_hash: icon.source_hash.clone(),
        }),
        render_recipe_hash: Some(render_recipe_crop_hash(
            &icon.shape,
            &ExportCropRect {
                x: icon.crop_x,
                y: icon.crop_y,
                width: icon.crop_w,
                height: icon.crop_h,
            },
            settings.frame_cell_width.max(1),
            settings.frame_cell_height.max(1),
            0,
            ImageTransform::new(
                icon.transform.quarter_turns,
                icon.transform.flip_horizontal,
                icon.transform.flip_vertical,
            )?,
            &icon.gif_loop_mode,
            icon.gif_loop_count,
            gif_icon_text_overlay(icon)?.as_ref(),
            &icon.effects,
            &icon.motion,
        )?),
        display_name: icon.display_name.clone(),
        loop_mode: effective_loop_mode(icon),
        loop_count: effective_loop_count(icon),
        frame_count: frames.len() as i64,
        duration_ms: frames.iter().map(|frame| frame.duration_ms).sum(),
        frame_cell_width: settings.frame_cell_width.max(1),
        frame_cell_height: settings.frame_cell_height.max(1),
        columns: split.columns_per_page,
        gap_x: settings.gap_x.max(0),
        gap_y: settings.gap_y.max(0),
        border_x: settings.border_x.max(0),
        border_y: settings.border_y.max(0),
        background: normalized_background(&settings.background),
        pages,
        frames: frame_items,
    })
}

fn validate_manifest_visual_source(
    manifest: &GifFrameSheetManifest,
    icon: &GifIconRecord,
) -> AppResult<()> {
    if manifest.schema == LEGACY_GIF_FRAME_SHEET_SCHEMA {
        let legacy_matches = icon.active_version_id.is_none()
            && icon.original_lineage_generation == 0
            && manifest.source_file_id.as_deref() == Some(icon.original_source_file_id.as_str())
            && manifest.source_hash.as_deref() == Some(icon.original_source_hash.as_str());
        if legacy_matches {
            return Ok(());
        }
        return Err(AppError::new(
            "manifest_stale",
            "AI 버전이 활성화되었거나 원본 계보가 바뀐 아이콘에는 legacy v1 GIF 시트를 다시 가져올 수 없습니다.",
        ));
    }

    let expected = ManifestVisualSource {
        original_source_file_id: icon.original_source_file_id.clone(),
        original_source_hash: icon.original_source_hash.clone(),
        original_lineage_id: icon.original_lineage_id.clone(),
        original_lineage_generation: icon.original_lineage_generation,
        effective_source_file_id: icon.source_file_id.clone(),
        effective_source_hash: icon.source_hash.clone(),
    };
    if manifest.visual_source.as_ref() != Some(&expected)
        || manifest.source_file_id.as_deref() != Some(icon.source_file_id.as_str())
        || manifest.source_hash.as_deref() != Some(icon.source_hash.as_str())
    {
        return Err(AppError::new(
            "manifest_stale",
            "GIF 프레임 시트를 내보낸 뒤 원본 계보 또는 AI 렌더 소스가 바뀌었습니다. 현재 상태에서 새 시트를 내보내세요.",
        ));
    }
    Ok(())
}

fn load_gif_icon(connection: &Connection, icon_id: &str) -> AppResult<GifIconRecord> {
    let collection_id = connection
        .query_row(
            "SELECT collection_id FROM icons WHERE id = ?1 AND deleted_at IS NULL",
            params![icon_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .ok_or_else(|| {
            AppError::not_found("GIF 프레임 시트로 내보낼 아이콘을 찾을 수 없습니다.")
        })?;
    ai_repository::resolve_effective_visual_source(connection, &collection_id, icon_id)?;
    connection
        .query_row(
            "SELECT
               i.id,
               evs.original_source_file_id,
               evs.original_source_sha256,
               evs.original_lineage_id,
               evs.original_lineage_generation,
               evs.active_version_id,
               evs.effective_source_file_id AS source_file_id,
               i.display_name,
               s.original_path_in_library,
               s.original_extension,
               s.sha256,
               s.is_animated,
               COALESCE(s.original_loop_mode, 'preserve') AS source_loop_mode,
               s.original_loop_count,
               i.shape,
               cs.crop_x,
               cs.crop_y,
               cs.crop_w,
               cs.crop_h,
               CASE WHEN i.gif_pingpong = 1 THEN 'pingpong' ELSE i.gif_loop_mode END AS gif_loop_mode,
               i.gif_loop_count,
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
             JOIN effective_visual_sources evs ON evs.icon_id = i.id
             JOIN source_files s ON s.id = evs.effective_source_file_id
             JOIN crop_settings cs ON cs.icon_id = i.id
             LEFT JOIN icon_effect_recipes er ON er.icon_id = i.id
             LEFT JOIN icon_motion_recipes mr ON mr.icon_id = i.id
             WHERE i.id = ?1
               AND i.deleted_at IS NULL
               AND i.icon_kind = 'image'",
            params![icon_id],
            |row| {
                Ok((
                    GifIconRecord {
                        id: row.get("id")?,
                        original_source_file_id: row.get("original_source_file_id")?,
                        original_source_hash: row.get("original_source_sha256")?,
                        original_lineage_id: row.get("original_lineage_id")?,
                        original_lineage_generation: row.get("original_lineage_generation")?,
                        active_version_id: row.get("active_version_id")?,
                        source_file_id: row.get("source_file_id")?,
                        display_name: row.get("display_name")?,
                        source_path: row.get("original_path_in_library")?,
                        source_extension: row.get("original_extension")?,
                        source_hash: row.get("sha256")?,
                        source_is_animated: row.get::<_, i64>("is_animated")? != 0,
                        source_gif_loop_mode: row.get("source_loop_mode")?,
                        source_gif_loop_count: row.get("original_loop_count")?,
                        shape: row.get("shape")?,
                        crop_x: row.get("crop_x")?,
                        crop_y: row.get("crop_y")?,
                        crop_w: row.get("crop_w")?,
                        crop_h: row.get("crop_h")?,
                        gif_loop_mode: row.get("gif_loop_mode")?,
                        gif_loop_count: row.get("gif_loop_count")?,
                        transform: ImageTransform {
                            quarter_turns: row.get("transform_quarter_turns")?,
                            flip_horizontal:
                                row.get::<_, i64>("transform_flip_horizontal")? != 0,
                            flip_vertical:
                                row.get::<_, i64>("transform_flip_vertical")? != 0,
                        },
                        text_overlay_enabled: row.get::<_, i64>("text_overlay_enabled")? != 0,
                        text_overlay_text: row.get("text_overlay_text")?,
                        text_overlay_font_path: row.get("text_overlay_font_path")?,
                        text_overlay_font_size: row.get("text_overlay_font_size")?,
                        text_overlay_x: row.get("text_overlay_x")?,
                        text_overlay_y: row.get("text_overlay_y")?,
                        text_overlay_color: row.get("text_overlay_color")?,
                        text_overlay_stroke_color: row.get("text_overlay_stroke_color")?,
                        text_overlay_stroke_width: row.get("text_overlay_stroke_width")?,
                        effects: EffectRecipe::default(),
                        motion: MotionRecipe::default(),
                    },
                    row.get::<_, Option<String>>("effect_recipe_json")?,
                    row.get::<_, Option<String>>("motion_recipe_json")?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("GIF 프레임 시트로 내보낼 아이콘을 찾을 수 없습니다."))
        .and_then(|(mut icon, effect_recipe_json, motion_recipe_json)| {
            icon.effects =
                parse_effect_recipe_json(effect_recipe_json.as_deref().unwrap_or_default())?;
            icon.motion =
                parse_motion_recipe_json(motion_recipe_json.as_deref().unwrap_or_default())?;
            if icon.source_extension != "gif" && !icon.motion.has_enabled_motion() {
                return Err(AppError::new(
                    "validation",
                    "GIF 프레임 시트는 기존 GIF 또는 모션 효과가 켜진 아이콘에서만 내보낼 수 있습니다.",
                ));
            }
            if icon.source_extension == "gif"
                && !icon.source_is_animated
                && !icon.motion.has_enabled_motion()
            {
                return Err(AppError::new(
                    "validation",
                    "프레임 시트로 내보낼 애니메이션 프레임이 없습니다.",
                ));
            }
            Ok(icon)
        })
}

fn load_source_timeline_frames(icon: &GifIconRecord) -> AppResult<Vec<SourceTimelineFrame>> {
    if icon.source_extension.eq_ignore_ascii_case("gif") {
        let file = File::open(&icon.source_path)?;
        let decoder = GifDecoder::new(BufReader::new(file))?;
        let frames = decoder.into_frames().collect_frames()?;
        if frames.is_empty() {
            return Err(AppError::new("gif", "GIF 프레임을 찾을 수 없습니다."));
        }
        return Ok(frames
            .into_iter()
            .map(|frame| {
                let duration_ms = delay_ms(frame.delay());
                SourceTimelineFrame {
                    image: Arc::new(frame.into_buffer()),
                    duration_ms,
                }
            })
            .collect());
    }

    let source = image::open(&icon.source_path)?.to_rgba8();
    let schedule = static_motion_schedule(&icon.motion)?;
    if schedule.is_empty() {
        return Err(AppError::new(
            "validation",
            "정적 이미지의 모션 프레임 일정이 비어 있습니다.",
        ));
    }

    Ok(shared_static_source_frames(source, schedule))
}

fn shared_static_source_frames(
    source: RgbaImage,
    schedule: Vec<MotionFrameTiming>,
) -> Vec<SourceTimelineFrame> {
    let source = Arc::new(source);
    schedule
        .into_iter()
        .map(|timing| SourceTimelineFrame {
            image: Arc::clone(&source),
            duration_ms: i64::from(timing.duration_ms),
        })
        .collect()
}

fn decode_rendered_frames(
    icon: &GifIconRecord,
    settings: &GifFrameSheetSettings,
) -> AppResult<Vec<DecodedFrame>> {
    let frames = load_source_timeline_frames(icon)?;
    let is_pingpong = is_pingpong_loop_mode(&icon.gif_loop_mode);
    let output_frame_count = if is_pingpong {
        pingpong_sequence_len(frames.len())
    } else {
        frames.len()
    };

    let viewport_width = viewport_width(&icon.shape, settings.frame_cell_width.max(1));
    let viewport_height = viewport_height(&icon.shape, settings.frame_cell_height.max(1));
    let source_geometry = source_viewport_geometry(
        &icon.shape,
        settings.frame_cell_width.max(1),
        settings.frame_cell_height.max(1),
        icon.transform,
    )?;
    let output_width = u32::try_from(viewport_width)
        .map_err(|_| AppError::new("validation", "GIF 프레임 너비가 올바르지 않습니다."))?;
    let output_height = u32::try_from(viewport_height)
        .map_err(|_| AppError::new("validation", "GIF 프레임 높이가 올바르지 않습니다."))?;
    validate_import_dimensions(output_width, output_height)?;
    let frame_cell_width = u32::try_from(settings.frame_cell_width)
        .map_err(|_| AppError::new("validation", "GIF 프레임 셀 너비가 올바르지 않습니다."))?;
    let frame_cell_height = u32::try_from(settings.frame_cell_height)
        .map_err(|_| AppError::new("validation", "GIF 프레임 셀 높이가 올바르지 않습니다."))?;
    validate_import_dimensions(frame_cell_width, frame_cell_height)?;
    validate_gif_workload(
        output_width,
        output_height,
        i64::try_from(output_frame_count).unwrap_or(i64::MAX),
    )
    .map_err(|message| AppError::new("validation", message))?;
    let repeat = output_repeat_for_settings(
        &icon.gif_loop_mode,
        icon.gif_loop_count,
        &icon.source_gif_loop_mode,
        icon.source_gif_loop_count,
    )?;
    let total_duration_ms = frames
        .iter()
        .map(|frame| u64::try_from(frame.duration_ms.max(0)).unwrap_or(u64::MAX))
        .sum::<u64>()
        .max(1);
    let final_frame_index = frames.len().saturating_sub(1);
    let mut elapsed_ms = 0_u64;
    let text_overlay = gif_icon_text_overlay(icon)?;
    let mut decoded = Vec::with_capacity(output_frame_count);

    for (frame_index, frame) in frames.into_iter().enumerate() {
        let duration_ms = frame.duration_ms;
        let source_frame = image_with_text_overlay(frame.image.as_ref(), text_overlay.as_ref())?;
        let viewport = crop_and_resize(
            source_frame.as_ref(),
            icon.crop_x,
            icon.crop_y,
            icon.crop_w,
            icon.crop_h,
            source_geometry.viewport.width,
            source_geometry.viewport.height,
        )?;
        let mut viewport = apply_image_transform(viewport, icon.transform)?;
        apply_effect_recipe(&mut viewport, &icon.effects)?;
        let context_elapsed_ms = if repeat == GifOutputRepeat::Once
            && frame_index == final_frame_index
            && icon.motion.has_enabled_motion()
        {
            total_duration_ms
        } else {
            elapsed_ms
        };
        let motion_result = apply_motion_recipe(
            &viewport,
            &icon.motion,
            MotionFrameContext {
                elapsed_ms: context_elapsed_ms,
                total_duration_ms,
            },
        )?;
        let viewport = motion_result.image;
        if i64::from(viewport.width()) != viewport_width
            || i64::from(viewport.height()) != viewport_height
        {
            return Err(AppError::new(
                "validation",
                "회전 후 GIF 프레임 시트 크기가 아이콘 모양과 일치하지 않습니다.",
            ));
        }
        let rendered_frame = match icon.shape.as_str() {
            "single" => viewport,
            "horizontal_double" => imageops::resize(
                &viewport,
                frame_cell_width,
                frame_cell_height,
                FilterType::Lanczos3,
            ),
            "vertical_double" => imageops::resize(
                &viewport,
                frame_cell_width,
                frame_cell_height,
                FilterType::Lanczos3,
            ),
            _ => {
                return Err(AppError::new(
                    "validation",
                    "지원하지 않는 아이콘 모양입니다.",
                ));
            }
        };
        let source_frame_hash = sha256_hex(&png_bytes_from_rgba(&rendered_frame)?);
        decoded.push(DecodedFrame {
            image: rendered_frame,
            duration_ms,
            source_frame_hash,
        });
        elapsed_ms = elapsed_ms.saturating_add(u64::try_from(duration_ms.max(0)).unwrap_or(0));
    }

    if is_pingpong {
        pingpong_sequence(&mut decoded);
    }
    Ok(decoded)
}

fn gif_icon_text_overlay(icon: &GifIconRecord) -> AppResult<Option<TextOverlayRenderSpec>> {
    text_overlay_from_fields(
        icon.text_overlay_enabled,
        Some(icon.text_overlay_text.clone()),
        icon.text_overlay_font_path.clone(),
        Some(icon.text_overlay_font_size),
        Some(icon.text_overlay_x),
        Some(icon.text_overlay_y),
        Some(icon.text_overlay_color.clone()),
        Some(icon.text_overlay_stroke_color.clone()),
        Some(icon.text_overlay_stroke_width),
    )
}

fn image_with_text_overlay<'a>(
    image: &'a RgbaImage,
    text_overlay: Option<&TextOverlayRenderSpec>,
) -> AppResult<Cow<'a, RgbaImage>> {
    if text_overlay.is_none() {
        return Ok(Cow::Borrowed(image));
    }
    let mut source = image.clone();
    apply_text_overlay(&mut source, text_overlay)?;
    Ok(Cow::Owned(source))
}

fn page_split_for_settings(
    item_count: usize,
    settings: &GifFrameSheetSettings,
) -> AppResult<super::grid::PageSplitPlan> {
    validate_export_settings(settings)?;
    let effective_max_height = if let Some(frames_per_page) = settings.frames_per_page {
        let rows = ((frames_per_page + settings.columns - 1) / settings.columns).max(1);
        settings.max_sheet_height.min(sheet_extent(
            rows,
            settings.frame_cell_height,
            settings.gap_y,
            settings.border_y,
        ))
    } else {
        settings.max_sheet_height
    };

    let plan = split_pages(
        item_count,
        PageSplitSettings {
            cell_width: settings.frame_cell_width,
            cell_height: settings.frame_cell_height,
            columns: settings.columns,
            gap_x: settings.gap_x,
            gap_y: settings.gap_y,
            border_x: settings.border_x,
            border_y: settings.border_y,
            max_sheet_width: settings.max_sheet_width,
            max_sheet_height: effective_max_height,
        },
    )?;
    validate_page_plan_workload(&plan)?;
    Ok(plan)
}

fn validate_export_settings(settings: &GifFrameSheetSettings) -> AppResult<()> {
    let frame_cell_width = u32::try_from(settings.frame_cell_width)
        .map_err(|_| AppError::new("validation", "GIF 프레임 시트 셀 너비가 올바르지 않습니다."))?;
    let frame_cell_height = u32::try_from(settings.frame_cell_height)
        .map_err(|_| AppError::new("validation", "GIF 프레임 시트 셀 높이가 올바르지 않습니다."))?;
    validate_import_dimensions(frame_cell_width, frame_cell_height)?;
    if !(1..=MAX_GIF_FRAMES).contains(&settings.columns) {
        return Err(AppError::new(
            "validation",
            format!("GIF 프레임 시트 열 수는 1 이상 {MAX_GIF_FRAMES} 이하여야 합니다."),
        ));
    }
    if settings.gap_x < 0
        || settings.gap_y < 0
        || settings.border_x < 0
        || settings.border_y < 0
        || settings.gap_x > i64::from(MAX_IMPORT_DIMENSION)
        || settings.gap_y > i64::from(MAX_IMPORT_DIMENSION)
        || settings.border_x > i64::from(MAX_IMPORT_DIMENSION)
        || settings.border_y > i64::from(MAX_IMPORT_DIMENSION)
    {
        return Err(AppError::new(
            "validation",
            "GIF 프레임 시트 간격과 테두리는 0~12,000px 범위여야 합니다.",
        ));
    }
    if !(1..=i64::from(MAX_IMPORT_DIMENSION)).contains(&settings.max_sheet_width)
        || !(1..=i64::from(MAX_IMPORT_DIMENSION)).contains(&settings.max_sheet_height)
    {
        return Err(AppError::new(
            "validation",
            "GIF 프레임 시트의 최대 가로·세로 크기는 1~12,000px 범위여야 합니다.",
        ));
    }
    if let Some(frames_per_page) = settings.frames_per_page {
        if !(1..=MAX_GIF_FRAMES).contains(&frames_per_page) {
            return Err(AppError::new(
                "validation",
                format!("페이지당 프레임 수는 1 이상 {MAX_GIF_FRAMES} 이하여야 합니다."),
            ));
        }
    }
    if !settings.include_clean_sheet && !settings.include_guide_sheet && !settings.include_manifest
    {
        return Err(AppError::new(
            "validation",
            "내보낼 GIF 프레임 시트 산출물을 하나 이상 선택해야 합니다.",
        ));
    }
    Ok(())
}

fn validate_page_plan_workload(plan: &super::grid::PageSplitPlan) -> AppResult<()> {
    let mut total_page_pixels = 0_u64;
    for page in &plan.pages {
        let width = u32::try_from(page.width)
            .map_err(|_| AppError::new("validation", "프레임 시트 너비가 올바르지 않습니다."))?;
        let height = u32::try_from(page.height)
            .map_err(|_| AppError::new("validation", "프레임 시트 높이가 올바르지 않습니다."))?;
        validate_import_dimensions(width, height)?;
        let page_pixels = u64::from(width).saturating_mul(u64::from(height));
        total_page_pixels = total_page_pixels.checked_add(page_pixels).ok_or_else(|| {
            AppError::new(
                "validation",
                "프레임 시트 페이지의 전체 픽셀 수가 너무 큽니다.",
            )
        })?;
        if total_page_pixels > MAX_GIF_TOTAL_FRAME_PIXELS {
            return Err(AppError::new(
                "validation",
                "프레임 시트 페이지의 전체 픽셀 수가 지원 범위를 벗어났습니다.",
            ));
        }
    }
    Ok(())
}

fn render_frame_sheet_page(
    frames: &[DecodedFrame],
    placements: &[&PageCellPlacement],
    width: i64,
    height: i64,
    background: &str,
    guide: bool,
) -> AppResult<RgbaImage> {
    let width = u32::try_from(width)
        .map_err(|_| AppError::new("validation", "프레임 시트 너비가 올바르지 않습니다."))?;
    let height = u32::try_from(height)
        .map_err(|_| AppError::new("validation", "프레임 시트 높이가 올바르지 않습니다."))?;
    validate_import_dimensions(width, height)?;
    let mut sheet = background_image(width, height, background, guide);
    for placement in placements {
        let frame = &frames[placement.item_index];
        imageops::overlay(&mut sheet, &frame.image, placement.x, placement.y);
    }

    if guide {
        for placement in placements {
            draw_grid_rect(&mut sheet, placement);
            draw_number_label(
                &mut sheet,
                placement.x + 4,
                placement.y + 4,
                placement.item_index + 1,
            );
            let duration = frames[placement.item_index].duration_ms;
            draw_small_label(
                &mut sheet,
                placement.x + 4,
                placement.y + placement.h - 12,
                &format!("{duration}ms"),
            );
        }
    }

    Ok(sheet)
}

fn validate_reimport_inputs(
    manifest_path: String,
    manifest_file: Option<ImportImageFilePayload>,
    edited_frame_sheet_paths: Vec<String>,
    edited_frame_sheet_files: Vec<ImportImageFilePayload>,
) -> AppResult<ReimportValidationInternal> {
    let manifest_path = PathBuf::from(manifest_path.trim());
    let manifest_file_supplied = manifest_file.is_some();
    let input_path_count = edited_frame_sheet_paths.len();
    let input_file_count = edited_frame_sheet_files.len();
    let (manifest, manifest_payload_bytes) = match manifest_file.as_ref() {
        Some(file) => (read_gif_manifest_bytes(&file.bytes)?, file.bytes.len()),
        None => {
            let bytes = read_import_file_bytes(&manifest_path)?;
            let byte_size = bytes.len();
            (read_gif_manifest_bytes(&bytes)?, byte_size)
        }
    };
    validate_gif_manifest(&manifest)?;

    let mut total_encoded_bytes = manifest_payload_bytes;
    for file in &edited_frame_sheet_files {
        validate_import_file_size(file.bytes.len())?;
        total_encoded_bytes = total_encoded_bytes
            .checked_add(file.bytes.len())
            .ok_or_else(|| {
                AppError::new(
                    "manifest_workload",
                    "선택한 프레임 시트 PNG의 전체 파일 크기가 너무 큽니다.",
                )
            })?;
    }
    if total_encoded_bytes > MAX_REIMPORT_TOTAL_ENCODED_BYTES {
        return Err(AppError::new(
            "manifest_workload",
            "선택한 프레임 시트 PNG는 합계 64MB까지 처리할 수 있습니다.",
        ));
    }

    let allow_sibling_lookup = !manifest_file_supplied && edited_frame_sheet_files.is_empty();
    let page_sources = resolve_page_sources(
        &manifest,
        &manifest_path,
        &edited_frame_sheet_paths,
        edited_frame_sheet_files,
        allow_sibling_lookup,
    );
    let mut warnings = Vec::new();
    let mut errors = Vec::new();
    let mut missing_pages = Vec::new();
    let mut wrong_dimension_pages = Vec::new();
    let mut detected_frame_indexes = HashSet::new();
    let mut page_images = HashMap::with_capacity(manifest.pages.len());
    let mut actual_decoded_page_pixels = 0_u64;

    if input_path_count + input_file_count > manifest.pages.len() {
        warnings.push(
            "선택한 프레임 시트 파일 수가 매니페스트 페이지 수보다 많습니다. 매니페스트에 있는 페이지만 사용합니다."
                .to_string(),
        );
    }

    for page in &manifest.pages {
        let Some(source) = page_sources.get(&page.page_index) else {
            missing_pages.push(page.page_index);
            errors.push(format!(
                "{} 페이지의 수정된 프레임 시트 PNG를 찾을 수 없습니다.",
                page.page_index + 1
            ));
            continue;
        };

        let image = match load_page_image_source(source, &mut total_encoded_bytes) {
            Ok(image) => image,
            Err(error) if error.code == "manifest_workload" => return Err(error),
            Err(error) => {
                errors.push(format!(
                    "{} 페이지를 읽을 수 없습니다: {}",
                    page.page_index + 1,
                    error.message
                ));
                continue;
            }
        };

        let page_pixels = u64::from(image.width()).saturating_mul(u64::from(image.height()));
        let next_total_page_pixels = actual_decoded_page_pixels
            .checked_add(page_pixels)
            .ok_or_else(|| {
                AppError::new(
                    "manifest_workload",
                    "디코드한 프레임 시트 페이지의 전체 픽셀 수가 너무 큽니다.",
                )
            })?;
        if next_total_page_pixels > MAX_GIF_TOTAL_FRAME_PIXELS {
            return Err(AppError::new(
                "manifest_workload",
                "디코드한 프레임 시트 페이지의 전체 픽셀 수가 지원 범위를 벗어났습니다.",
            ));
        }
        actual_decoded_page_pixels = next_total_page_pixels;
        if i64::from(image.width()) != page.width || i64::from(image.height()) != page.height {
            wrong_dimension_pages.push(page.page_index);
            errors.push(format!(
                "{} 페이지 크기가 매니페스트와 다릅니다. 예상 {}x{}, 실제 {}x{}.",
                page.page_index + 1,
                page.width,
                page.height,
                image.width(),
                image.height()
            ));
            continue;
        }
        if !image.pixels().any(|pixel| pixel.0[3] < 255) {
            warnings.push(format!(
                "{} 페이지에서 투명 픽셀이 감지되지 않았습니다. 외부 편집 중 alpha가 사라졌는지 확인하세요.",
                page.page_index + 1
            ));
        }
        for frame in manifest
            .frames
            .iter()
            .filter(|frame| frame.page_index == page.page_index)
        {
            if frame_fits_image(frame, &image) {
                detected_frame_indexes.insert(frame.frame_index);
            } else {
                errors.push(format!(
                    "frame {} 셀 영역이 {} 페이지 이미지 밖으로 벗어났습니다.",
                    frame.frame_index,
                    page.page_index + 1
                ));
            }
        }
        page_images.insert(page.page_index, image);
    }

    if detected_frame_indexes.len() != manifest.frames.len() {
        errors.push(format!(
            "감지된 프레임 수가 매니페스트와 다릅니다. 예상 {}, 감지 {}.",
            manifest.frames.len(),
            detected_frame_indexes.len()
        ));
    }

    Ok(ReimportValidationInternal {
        public: GifFrameSheetReimportValidation {
            frame_count: manifest.frame_count,
            detected_frame_count: detected_frame_indexes.len() as i64,
            page_count: manifest.pages.len() as i64,
            missing_pages,
            wrong_dimension_pages,
            loop_mode: manifest.loop_mode.clone(),
            loop_count: manifest.loop_count,
            duration_ms: manifest.duration_ms,
            warnings,
            errors,
        },
        manifest,
        page_images,
    })
}

fn resolve_page_sources(
    manifest: &GifFrameSheetManifest,
    manifest_path: &Path,
    explicit_paths: &[String],
    explicit_files: Vec<ImportImageFilePayload>,
    allow_sibling_lookup: bool,
) -> HashMap<i64, PageImageSource> {
    let paths = explicit_paths
        .iter()
        .map(|path| PathBuf::from(path.trim()))
        .filter(|path| !path.as_os_str().is_empty())
        .collect::<Vec<_>>();
    let mut output = HashMap::new();

    if paths.len() == manifest.pages.len() {
        let mut pages = manifest.pages.iter().collect::<Vec<_>>();
        pages.sort_by_key(|page| page.page_index);
        for (page, path) in pages.into_iter().zip(paths) {
            output.insert(page.page_index, PageImageSource::Path(path));
        }
    } else {
        let by_name = paths
            .iter()
            .filter_map(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| (name.to_string(), path.clone()))
            })
            .collect::<HashMap<_, _>>();
        let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
        for page in &manifest.pages {
            if let Some(path) = by_name.get(&page.clean_sheet_file) {
                output.insert(page.page_index, PageImageSource::Path(path.clone()));
                continue;
            }
            if allow_sibling_lookup {
                if let Some(same_dir) =
                    contained_manifest_sibling(manifest_dir, &page.clean_sheet_file)
                {
                    output.insert(page.page_index, PageImageSource::Path(same_dir));
                }
            }
        }
    }

    if explicit_files.len() == manifest.pages.len() {
        let all_names_match = manifest.pages.iter().all(|page| {
            explicit_files
                .iter()
                .any(|file| file.original_filename == page.clean_sheet_file)
        });
        if all_names_match {
            let mut files = explicit_files
                .into_iter()
                .map(|file| (file.original_filename, file.bytes))
                .collect::<HashMap<_, _>>();
            for page in &manifest.pages {
                if let Some(bytes) = files.remove(&page.clean_sheet_file) {
                    output.insert(page.page_index, PageImageSource::Bytes(bytes));
                }
            }
        } else {
            let mut pages = manifest.pages.iter().collect::<Vec<_>>();
            pages.sort_by_key(|page| page.page_index);
            for (page, file) in pages.into_iter().zip(explicit_files) {
                output.insert(page.page_index, PageImageSource::Bytes(file.bytes));
            }
        }
    } else {
        let mut files = explicit_files
            .into_iter()
            .map(|file| (file.original_filename, file.bytes))
            .collect::<HashMap<_, _>>();
        for page in &manifest.pages {
            if let Some(bytes) = files.remove(&page.clean_sheet_file) {
                output.insert(page.page_index, PageImageSource::Bytes(bytes));
            }
        }
    }

    output
}

fn contained_manifest_sibling(manifest_dir: &Path, file_name: &str) -> Option<PathBuf> {
    let canonical_root = fs::canonicalize(manifest_dir).ok()?;
    let candidate = fs::canonicalize(manifest_dir.join(file_name)).ok()?;
    candidate.starts_with(&canonical_root).then_some(candidate)
}

fn frame_fits_image(frame: &GifFrameManifestItem, image: &RgbaImage) -> bool {
    let Ok(x) = u32::try_from(frame.x) else {
        return false;
    };
    let Ok(y) = u32::try_from(frame.y) else {
        return false;
    };
    let Ok(width) = u32::try_from(frame.w) else {
        return false;
    };
    let Ok(height) = u32::try_from(frame.h) else {
        return false;
    };
    let Some(right) = x.checked_add(width) else {
        return false;
    };
    let Some(bottom) = y.checked_add(height) else {
        return false;
    };
    width > 0 && height > 0 && right <= image.width() && bottom <= image.height()
}

fn crop_reimport_frames(
    manifest: &GifFrameSheetManifest,
    page_images: &HashMap<i64, RgbaImage>,
) -> AppResult<Vec<Frame>> {
    let mut frame_items = manifest.frames.iter().collect::<Vec<_>>();
    frame_items.sort_by_key(|frame| frame.frame_index);
    let mut frames = Vec::with_capacity(frame_items.len());

    for item in frame_items {
        let page = page_images.get(&item.page_index).ok_or_else(|| {
            AppError::new(
                "validation",
                format!(
                    "frame {}의 페이지 이미지를 읽을 수 없습니다.",
                    item.frame_index
                ),
            )
        })?;
        if !frame_fits_image(item, page) {
            return Err(AppError::new(
                "validation",
                format!(
                    "frame {}의 셀 영역이 이미지 밖으로 나갑니다.",
                    item.frame_index
                ),
            ));
        }
        let x = u32::try_from(item.x)
            .map_err(|_| AppError::new("validation", "프레임 x 좌표가 올바르지 않습니다."))?;
        let y = u32::try_from(item.y)
            .map_err(|_| AppError::new("validation", "프레임 y 좌표가 올바르지 않습니다."))?;
        let width = u32::try_from(item.w)
            .map_err(|_| AppError::new("validation", "프레임 너비가 올바르지 않습니다."))?;
        let height = u32::try_from(item.h)
            .map_err(|_| AppError::new("validation", "프레임 높이가 올바르지 않습니다."))?;
        let duration_ms = u32::try_from(item.duration_ms).map_err(|_| {
            AppError::new("validation", "프레임 재생시간이 지원 범위를 벗어났습니다.")
        })?;
        let cropped = imageops::crop_imm(page, x, y, width, height).to_image();
        frames.push(Frame::from_parts(
            cropped,
            0,
            0,
            Delay::from_numer_denom_ms(duration_ms, 1),
        ));
    }

    Ok(frames)
}

fn load_page_image_source(
    source: &PageImageSource,
    total_encoded_bytes: &mut usize,
) -> AppResult<RgbaImage> {
    match source {
        PageImageSource::Path(path) => {
            let bytes = read_import_file_bytes(path)?;
            let next_total_encoded_bytes = total_encoded_bytes
                .checked_add(bytes.len())
                .ok_or_else(|| {
                    AppError::new(
                        "manifest_workload",
                        "선택한 프레임 시트 PNG의 전체 파일 크기가 너무 큽니다.",
                    )
                })?;
            if next_total_encoded_bytes > MAX_REIMPORT_TOTAL_ENCODED_BYTES {
                return Err(AppError::new(
                    "manifest_workload",
                    "선택한 프레임 시트 PNG는 합계 64MB까지 처리할 수 있습니다.",
                ));
            }
            *total_encoded_bytes = next_total_encoded_bytes;
            Ok(decode_import_image(&bytes, ImageFormat::Png)?.to_rgba8())
        }
        PageImageSource::Bytes(bytes) => {
            Ok(decode_import_image(bytes, ImageFormat::Png)?.to_rgba8())
        }
    }
}
fn repeat_from_manifest(manifest: &GifFrameSheetManifest) -> AppResult<GifOutputRepeat> {
    output_repeat_for_settings(
        &manifest.loop_mode,
        manifest.loop_count,
        &manifest.loop_mode,
        manifest.loop_count,
    )
}

fn gif_output_directory(
    paths: &AppPaths,
    settings: &GifFrameSheetSettings,
    icon: &GifIconRecord,
) -> AppResult<PathBuf> {
    let run_name = format!(
        "{}-{}",
        sanitize_name(&format!("{}-{}", icon.display_name, icon.id)),
        timestamp_suffix()
    );
    let output_root = settings
        .output_directory
        .as_ref()
        .map(|path| PathBuf::from(path.trim()).join(&run_name))
        .unwrap_or_else(|| {
            paths
                .root
                .join("sheet_exports")
                .join("gif_frames")
                .join(&run_name)
        });
    fs::create_dir_all(&output_root)?;
    Ok(output_root)
}

fn save_png_atomic(path: &Path, image: &RgbaImage) -> AppResult<()> {
    let temp_path = path.with_extension("png.tmp");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    image.save_with_format(&temp_path, ImageFormat::Png)?;
    move_temp_file(&temp_path, path)?;
    Ok(())
}

fn write_gif_atomic(
    temp_path: &Path,
    final_path: &Path,
    frames: Vec<Frame>,
    repeat: GifOutputRepeat,
) -> AppResult<()> {
    if let Some(parent) = temp_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = File::create(temp_path)?;
    let mut encoder = GifEncoder::new(BufWriter::new(file));
    match repeat {
        GifOutputRepeat::Infinite => encoder.set_repeat(ImageGifRepeat::Infinite)?,
        GifOutputRepeat::Finite(count) => encoder.set_repeat(ImageGifRepeat::Finite(count))?,
        GifOutputRepeat::Once => {}
    }
    encoder.encode_frames(frames.into_iter())?;
    drop(encoder);
    move_temp_file(temp_path, final_path)?;
    Ok(())
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
        .map_err(|_| AppError::new("validation", "GIF 프레임 너비가 올바르지 않습니다."))?;
    let height = u32::try_from(height)
        .map_err(|_| AppError::new("validation", "GIF 프레임 높이가 올바르지 않습니다."))?;
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
    draw_small_label(sheet, x, y, &number.to_string());
}

fn draw_small_label(sheet: &mut RgbaImage, x: i64, y: i64, label: &str) {
    let mut cursor_x = x.max(0) as u32;
    let y = y.max(0) as u32;
    for character in label.chars() {
        if character.is_ascii_digit() {
            draw_digit(sheet, cursor_x, y, character);
            cursor_x += 5;
        } else {
            cursor_x += 4;
        }
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

fn png_bytes_from_rgba(image: &RgbaImage) -> AppResult<Vec<u8>> {
    let mut cursor = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image.clone())
        .write_to(&mut cursor, ImageFormat::Png)
        .map_err(AppError::from)?;
    Ok(cursor.into_inner())
}

fn delay_ms(delay: Delay) -> i64 {
    let (numerator, denominator) = delay.numer_denom_ms();
    let denominator = denominator.max(1);
    i64::from((numerator / denominator).max(1))
}

fn effective_loop_mode(icon: &GifIconRecord) -> String {
    if icon.source_extension != "gif" && icon.gif_loop_mode == "preserve" {
        return "infinite".to_string();
    }
    if icon.gif_loop_mode == "preserve" {
        icon.source_gif_loop_mode.clone()
    } else {
        icon.gif_loop_mode.clone()
    }
}

fn effective_loop_count(icon: &GifIconRecord) -> Option<i64> {
    if icon.source_extension != "gif" && icon.gif_loop_mode == "preserve" {
        return None;
    }
    if icon.gif_loop_mode == "preserve" {
        icon.source_gif_loop_count
    } else {
        icon.gif_loop_count
    }
}

fn analysis_warnings(icon: &GifIconRecord, settings: &GifFrameSheetSettings) -> Vec<String> {
    let mut warnings = Vec::new();
    if icon.shape != "single" {
        warnings.push(
            "multi-piece GIF 아이콘은 프레임 시트에서 전체 애니메이션을 하나의 셀로 렌더링합니다."
                .to_string(),
        );
    }
    if normalized_background(&settings.background) != "transparent" {
        warnings.push(
            "transparent가 아닌 배경을 선택하면 clean frame sheet의 투명 픽셀이 배경색으로 보일 수 있습니다."
                .to_string(),
        );
    }
    warnings
}

fn normalized_background(value: &str) -> String {
    match value {
        "checker" | "white" | "black" => value.to_string(),
        _ => "transparent".to_string(),
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

fn sheet_extent(count: i64, cell: i64, gap: i64, border: i64) -> i64 {
    border * 2 + count * cell + (count - 1).max(0) * gap
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
        "gif_frame_sheet".to_string()
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

fn default_background() -> String {
    "transparent".to_string()
}

fn default_max_sheet_size() -> i64 {
    2048
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    use image::{AnimationDecoder, DynamicImage, ImageBuffer, ImageFormat, Rgba};
    use rusqlite::Connection;

    use crate::db::migrations;
    use crate::db::repositories::collections::create_collection;
    use crate::db::repositories::effects::upsert_effect_recipe;
    use crate::db::repositories::imports::import_image_files;
    use crate::db::repositories::motion::upsert_motion_recipe;
    use crate::imaging::effects::{EffectRecipe, EffectStep, ToneMode, EFFECT_RECIPE_VERSION};
    use crate::imaging::motion::{static_motion_schedule, MotionRecipe, SpatialMotion};
    use crate::models::ImportImageFilePayload;
    use crate::paths::AppPaths;

    use super::{
        analyze_gif_frame_sheet_export, crop_reimport_frames, decode_rendered_frames,
        export_gif_frame_sheet, load_gif_icon, load_page_image_source, reimport_gif_frame_sheet,
        shared_static_source_frames, validate_gif_frame_sheet_reimport, validate_reimport_inputs,
        AnalyzeGifFrameSheetExportRequest, GifFrameSheetExportRequest,
        GifFrameSheetReimportRequest, GifFrameSheetSettings, PageImageSource,
        ValidateGifFrameSheetReimportRequest, MAX_REIMPORT_TOTAL_ENCODED_BYTES,
    };

    fn connection() -> Connection {
        let mut connection = Connection::open_in_memory().unwrap();
        migrations::run(&mut connection).unwrap();
        connection
    }

    fn temp_paths(prefix: &str) -> AppPaths {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        AppPaths::prepare(std::env::temp_dir().join(format!("{prefix}-{suffix}"))).unwrap()
    }

    fn settings() -> GifFrameSheetSettings {
        GifFrameSheetSettings {
            frame_cell_width: 24,
            frame_cell_height: 24,
            columns: 2,
            frames_per_page: Some(2),
            gap_x: 4,
            gap_y: 4,
            border_x: 6,
            border_y: 6,
            max_sheet_width: 256,
            max_sheet_height: 256,
            background: "transparent".to_string(),
            include_clean_sheet: true,
            include_guide_sheet: true,
            include_manifest: true,
            output_directory: None,
            open_output_folder: false,
        }
    }

    fn animated_gif_bytes() -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut encoder = gif::Encoder::new(&mut bytes, 24, 24, &[]).unwrap();
            encoder.set_repeat(gif::Repeat::Infinite).unwrap();
            for frame_index in 0..4_u8 {
                let mut pixels = Vec::with_capacity(24 * 24 * 4);
                for y in 0..24_u8 {
                    for x in 0..24_u8 {
                        let alpha = if x < 3 && y < 3 { 0 } else { 255 };
                        pixels.extend_from_slice(&[
                            x.wrapping_mul(9).wrapping_add(frame_index * 13),
                            y.wrapping_mul(7).wrapping_add(frame_index * 11),
                            frame_index.wrapping_mul(31),
                            alpha,
                        ]);
                    }
                }
                let mut frame = gif::Frame::from_rgba_speed(24, 24, &mut pixels, 10);
                frame.delay = 5 + u16::from(frame_index);
                encoder.write_frame(&frame).unwrap();
            }
        }
        bytes
    }

    fn seed_gif_icon(connection: &mut Connection, paths: &AppPaths) -> (String, Vec<u8>) {
        let collection =
            create_collection(connection, Some("gif frame sheet".to_string())).unwrap();
        let bytes = animated_gif_bytes();
        let imported = import_image_files(
            connection,
            paths,
            &collection.id,
            vec![ImportImageFilePayload {
                original_filename: "source.gif".to_string(),
                bytes: bytes.clone(),
            }],
        )
        .unwrap();
        (imported.imported_icons[0].id.clone(), bytes)
    }

    #[test]
    fn static_motion_timeline_shares_one_source_allocation() {
        let source = ImageBuffer::from_pixel(16, 16, Rgba([255, 0, 0, 255]));
        let schedule = static_motion_schedule(&MotionRecipe {
            duration_ms: 10_000,
            fps: 50,
            ..MotionRecipe::default()
        })
        .unwrap();
        let frames = shared_static_source_frames(source, schedule);

        assert_eq!(frames.len(), 500);
        assert!(frames[1..]
            .iter()
            .all(|frame| Arc::ptr_eq(&frames[0].image, &frame.image)));
        assert_eq!(Arc::strong_count(&frames[0].image), frames.len());
    }

    #[test]
    fn gif_frame_export_writes_clean_guide_and_manifest_with_page_split() {
        let mut connection = connection();
        let paths = temp_paths("pmtconcon-gif-frame-export");
        let (icon_id, source_bytes) = seed_gif_icon(&mut connection, &paths);

        let analysis = analyze_gif_frame_sheet_export(
            &connection,
            AnalyzeGifFrameSheetExportRequest {
                icon_id: icon_id.clone(),
                settings: settings(),
            },
        )
        .unwrap();
        assert_eq!(analysis.frame_count, 4);
        assert_eq!(analysis.page_count, 2);

        let result = export_gif_frame_sheet(
            &connection,
            &paths,
            GifFrameSheetExportRequest {
                icon_id,
                settings: settings(),
            },
        )
        .unwrap();
        assert_eq!(result.frame_count, 4);
        assert_eq!(result.page_count, 2);
        assert_eq!(result.frame_sheet_paths.len(), 2);
        assert_eq!(result.guide_sheet_paths.len(), 2);
        assert!(std::path::Path::new(result.manifest_path.as_ref().unwrap()).is_file());

        let manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(result.manifest_path.unwrap()).unwrap()).unwrap();
        assert_eq!(manifest["schema"], "pmtcon-gif-frame-sheet-v2");
        assert!(manifest["visual_source"].is_object());
        assert_eq!(manifest["frame_count"], 4);
        assert_eq!(manifest["duration_ms"], 260);
        assert_eq!(manifest["loop_mode"], "infinite");
        assert_eq!(std::fs::read(connection.query_row::<String, _, _>(
            "SELECT s.original_path_in_library FROM source_files s JOIN icons i ON i.source_file_id = s.id WHERE i.id = ?1",
            [&manifest["icon_id"].as_str().unwrap()],
            |row| row.get(0),
        ).unwrap()).unwrap(), source_bytes);

        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn gif_frame_sheet_loads_and_applies_persisted_effect_recipe() {
        let mut connection = connection();
        let paths = temp_paths("pmtconcon-gif-frame-effects");
        let (icon_id, _) = seed_gif_icon(&mut connection, &paths);
        let before =
            decode_rendered_frames(&load_gif_icon(&connection, &icon_id).unwrap(), &settings())
                .unwrap();
        let before_pixel = *before[0].image.get_pixel(12, 12);
        assert_ne!(before_pixel[0], before_pixel[1]);

        let collection_id: String = connection
            .query_row(
                "SELECT collection_id FROM icons WHERE id = ?1",
                [&icon_id],
                |row| row.get(0),
            )
            .unwrap();
        let recipe = EffectRecipe {
            version: EFFECT_RECIPE_VERSION,
            effects: vec![EffectStep::Tone {
                id: "grayscale".to_string(),
                enabled: true,
                mode: ToneMode::Grayscale,
                amount: 100,
            }],
        };
        let transaction = connection.transaction().unwrap();
        upsert_effect_recipe(&transaction, &collection_id, &icon_id, 0, &recipe).unwrap();
        transaction.commit().unwrap();

        let stored = load_gif_icon(&connection, &icon_id).unwrap();
        assert_eq!(stored.effects, recipe);
        let after = decode_rendered_frames(&stored, &settings()).unwrap();
        let after_pixel = *after[0].image.get_pixel(12, 12);
        assert_ne!(after_pixel, before_pixel);
        assert_eq!(after_pixel[0], after_pixel[1]);
        assert_eq!(after_pixel[1], after_pixel[2]);
        assert_eq!(after[0].duration_ms, before[0].duration_ms);

        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn gif_frame_reimport_creates_variant_and_preserves_timing_count() {
        let mut connection = connection();
        let paths = temp_paths("pmtconcon-gif-frame-reimport");
        let (icon_id, source_bytes) = seed_gif_icon(&mut connection, &paths);
        let export = export_gif_frame_sheet(
            &connection,
            &paths,
            GifFrameSheetExportRequest {
                icon_id: icon_id.clone(),
                settings: settings(),
            },
        )
        .unwrap();
        let manifest_path = export.manifest_path.clone().unwrap();

        let mut edited = image::open(&export.frame_sheet_paths[0])
            .unwrap()
            .to_rgba8();
        for y in 6..30 {
            for x in 6..30 {
                edited.put_pixel(x, y, Rgba([255, 0, 0, 128]));
            }
        }
        let mut cursor = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(edited)
            .write_to(&mut cursor, ImageFormat::Png)
            .unwrap();
        let edited_first = cursor.into_inner();
        let second_page_bytes = std::fs::read(&export.frame_sheet_paths[1]).unwrap();

        let validation = validate_gif_frame_sheet_reimport(ValidateGifFrameSheetReimportRequest {
            manifest_path: manifest_path.clone(),
            manifest_file: None,
            edited_frame_sheet_paths: Vec::new(),
            edited_frame_sheet_files: vec![
                ImportImageFilePayload {
                    original_filename: "frames_sheet_001.png".to_string(),
                    bytes: edited_first.clone(),
                },
                ImportImageFilePayload {
                    original_filename: "frames_sheet_002.png".to_string(),
                    bytes: second_page_bytes,
                },
            ],
        })
        .unwrap();
        assert!(validation.errors.is_empty());
        assert_eq!(validation.detected_frame_count, 4);

        let result = reimport_gif_frame_sheet(
            &connection,
            &paths,
            GifFrameSheetReimportRequest {
                manifest_path,
                manifest_file: None,
                edited_frame_sheet_paths: Vec::new(),
                edited_frame_sheet_files: vec![
                    ImportImageFilePayload {
                        original_filename: "frames_sheet_001.png".to_string(),
                        bytes: edited_first,
                    },
                    ImportImageFilePayload {
                        original_filename: "frames_sheet_002.png".to_string(),
                        bytes: std::fs::read(&export.frame_sheet_paths[1]).unwrap(),
                    },
                ],
                target_icon_id: icon_id.clone(),
                create_variant: true,
                set_active_variant: false,
                target_profile_id: None,
            },
        )
        .unwrap();
        assert!(result.errors.is_empty());
        assert!(result.variant_id.is_some());
        let output_path = result.output_path.unwrap();
        assert!(std::path::Path::new(&output_path).is_file());

        let file = std::fs::File::open(output_path).unwrap();
        let decoder = image::codecs::gif::GifDecoder::new(std::io::BufReader::new(file)).unwrap();
        let frames = decoder.into_frames().collect_frames().unwrap();
        assert_eq!(frames.len(), 4);
        let total_ms = frames
            .iter()
            .map(|frame| {
                let (numerator, denominator) = frame.delay().numer_denom_ms();
                numerator / denominator.max(1)
            })
            .sum::<u32>();
        assert_eq!(total_ms, 260);

        let original_path: String = connection
            .query_row(
                "SELECT s.original_path_in_library FROM source_files s JOIN icons i ON i.source_file_id = s.id WHERE i.id = ?1",
                [&icon_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(std::fs::read(original_path).unwrap(), source_bytes);

        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn gif_frame_reimport_only_activates_when_source_and_render_recipe_still_match() {
        let mut connection = connection();
        let paths = temp_paths("pmtconcon-gif-frame-recipe");
        let (icon_id, _) = seed_gif_icon(&mut connection, &paths);
        connection
            .execute(
                "UPDATE icons
                 SET cell_width_override = 24,
                     cell_height_override = 24
                 WHERE id = ?1",
                [&icon_id],
            )
            .unwrap();
        let profile_id: String = connection
            .query_row(
                "SELECT p.id
                 FROM export_profiles p
                 JOIN icons i ON i.collection_id = p.collection_id
                 WHERE i.id = ?1
                 ORDER BY p.created_at ASC
                 LIMIT 1",
                [&icon_id],
                |row| row.get(0),
            )
            .unwrap();
        let export = export_gif_frame_sheet(
            &connection,
            &paths,
            GifFrameSheetExportRequest {
                icon_id: icon_id.clone(),
                settings: settings(),
            },
        )
        .unwrap();
        let manifest_path = export.manifest_path.clone().unwrap();
        let manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();
        assert!(manifest["render_recipe_hash"].as_str().is_some());

        let matching = reimport_gif_frame_sheet(
            &connection,
            &paths,
            GifFrameSheetReimportRequest {
                manifest_path: manifest_path.clone(),
                manifest_file: None,
                edited_frame_sheet_paths: export.frame_sheet_paths.clone(),
                edited_frame_sheet_files: Vec::new(),
                target_icon_id: icon_id.clone(),
                create_variant: true,
                set_active_variant: true,
                target_profile_id: Some(profile_id.clone()),
            },
        )
        .unwrap();
        assert!(matching.active_variant_set);

        let collection_id: String = connection
            .query_row(
                "SELECT collection_id FROM icons WHERE id = ?1",
                [&icon_id],
                |row| row.get(0),
            )
            .unwrap();
        let transaction = connection.transaction().unwrap();
        upsert_effect_recipe(
            &transaction,
            &collection_id,
            &icon_id,
            0,
            &EffectRecipe {
                version: EFFECT_RECIPE_VERSION,
                effects: vec![EffectStep::Pixelate {
                    id: "stale-pixelate".to_string(),
                    enabled: true,
                    block_size: 4,
                }],
            },
        )
        .unwrap();
        transaction.commit().unwrap();
        let stale = reimport_gif_frame_sheet(
            &connection,
            &paths,
            GifFrameSheetReimportRequest {
                manifest_path,
                manifest_file: None,
                edited_frame_sheet_paths: export.frame_sheet_paths,
                edited_frame_sheet_files: Vec::new(),
                target_icon_id: icon_id,
                create_variant: true,
                set_active_variant: true,
                target_profile_id: Some(profile_id),
            },
        )
        .unwrap();

        assert!(!stale.active_variant_set);
        assert!(stale
            .warnings
            .iter()
            .any(|warning| warning.contains("recipe가 바뀌어")));
        let stale_profile_id: Option<String> = connection
            .query_row(
                "SELECT profile_id
                 FROM processed_asset_variants
                 WHERE id = ?1",
                [stale.variant_id.as_ref().unwrap()],
                |row| row.get(0),
            )
            .unwrap();
        assert!(stale_profile_id.is_none());

        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn gif_frame_reimport_validation_detects_missing_and_wrong_size_pages() {
        let mut connection = connection();
        let paths = temp_paths("pmtconcon-gif-frame-validate");
        let (icon_id, _) = seed_gif_icon(&mut connection, &paths);
        let export = export_gif_frame_sheet(
            &connection,
            &paths,
            GifFrameSheetExportRequest {
                icon_id,
                settings: settings(),
            },
        )
        .unwrap();
        let small = ImageBuffer::from_pixel(10, 10, Rgba([0, 0, 0, 0]));
        let mut cursor = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(small)
            .write_to(&mut cursor, ImageFormat::Png)
            .unwrap();

        let manifest_bytes = std::fs::read(export.manifest_path.unwrap()).unwrap();
        let validation = validate_gif_frame_sheet_reimport(ValidateGifFrameSheetReimportRequest {
            manifest_path: String::new(),
            manifest_file: Some(ImportImageFilePayload {
                original_filename: "frames_manifest.json".to_string(),
                bytes: manifest_bytes,
            }),
            edited_frame_sheet_paths: Vec::new(),
            edited_frame_sheet_files: vec![ImportImageFilePayload {
                original_filename: "frames_sheet_001.png".to_string(),
                bytes: cursor.into_inner(),
            }],
        })
        .unwrap();
        assert!(!validation.errors.is_empty());
        assert_eq!(validation.missing_pages, vec![1]);
        assert_eq!(validation.wrong_dimension_pages, vec![0]);

        std::fs::remove_dir_all(paths.root).unwrap();
    }
    #[test]
    fn gif_frame_sheet_pingpong_reflects_final_motion_frames() {
        let mut connection = connection();
        let paths = temp_paths("pmtconcon-gif-frame-pingpong-motion");
        let (icon_id, _) = seed_gif_icon(&mut connection, &paths);
        let collection_id: String = connection
            .query_row(
                "SELECT collection_id FROM icons WHERE id = ?1",
                [&icon_id],
                |row| row.get(0),
            )
            .unwrap();
        let transaction = connection.transaction().unwrap();
        upsert_motion_recipe(
            &transaction,
            &collection_id,
            &icon_id,
            0,
            &MotionRecipe {
                duration_ms: 260,
                fps: 10,
                seed: 91,
                spatial: Some(SpatialMotion::Spin {
                    enabled: true,
                    cycles_per_loop: 1,
                    clockwise: true,
                }),
                ..MotionRecipe::default()
            },
        )
        .unwrap();
        transaction.commit().unwrap();

        let mut normal_icon = load_gif_icon(&connection, &icon_id).unwrap();
        normal_icon.gif_loop_mode = "infinite".to_string();
        let normal = decode_rendered_frames(&normal_icon, &settings()).unwrap();
        let mut pingpong_icon = load_gif_icon(&connection, &icon_id).unwrap();
        pingpong_icon.gif_loop_mode = "pingpong".to_string();
        let actual = decode_rendered_frames(&pingpong_icon, &settings()).unwrap();

        assert_eq!(normal.len(), 4);
        assert_eq!(actual.len(), 6);
        assert_eq!(
            actual
                .iter()
                .map(|frame| frame.duration_ms)
                .collect::<Vec<_>>(),
            vec![50, 60, 70, 80, 70, 60]
        );
        for index in 0..4 {
            assert_eq!(actual[index].image, normal[index].image);
            assert_eq!(
                actual[index].source_frame_hash,
                normal[index].source_frame_hash
            );
        }
        assert_eq!(actual[4].image, normal[2].image);
        assert_eq!(actual[4].source_frame_hash, normal[2].source_frame_hash);
        assert_eq!(actual[5].image, normal[1].image);
        assert_eq!(actual[5].source_frame_hash, normal[1].source_frame_hash);

        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn gif_frame_reimport_rejects_target_mismatch_before_writing() {
        let mut connection = connection();
        let paths = temp_paths("pmtconcon-gif-frame-target-mismatch");
        let (manifest_icon_id, _) = seed_gif_icon(&mut connection, &paths);
        let (other_icon_id, _) = seed_gif_icon(&mut connection, &paths);
        let export = export_gif_frame_sheet(
            &connection,
            &paths,
            GifFrameSheetExportRequest {
                icon_id: manifest_icon_id,
                settings: settings(),
            },
        )
        .unwrap();
        let reimport_root = paths.processed_variants_dir.join("gif_frame_reimports");
        let result = reimport_gif_frame_sheet(
            &connection,
            &paths,
            GifFrameSheetReimportRequest {
                manifest_path: export.manifest_path.unwrap(),
                manifest_file: None,
                edited_frame_sheet_paths: export.frame_sheet_paths,
                edited_frame_sheet_files: Vec::new(),
                target_icon_id: other_icon_id,
                create_variant: true,
                set_active_variant: false,
                target_profile_id: None,
            },
        )
        .unwrap();

        assert!(result.variant_id.is_none());
        assert!(result.output_path.is_none());
        assert!(!result.errors.is_empty());
        assert!(!reimport_root.exists());
        let variant_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM processed_asset_variants", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(variant_count, 0);

        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn gif_frame_reimport_preflights_missing_manifest_icon_without_orphan() {
        let mut connection = connection();
        let paths = temp_paths("pmtconcon-gif-frame-missing-target");
        let (icon_id, _) = seed_gif_icon(&mut connection, &paths);
        let export = export_gif_frame_sheet(
            &connection,
            &paths,
            GifFrameSheetExportRequest {
                icon_id,
                settings: settings(),
            },
        )
        .unwrap();
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(export.manifest_path.unwrap()).unwrap()).unwrap();
        manifest["icon_id"] = serde_json::json!("icon_missing");
        let edited_frame_sheet_files = export
            .frame_sheet_paths
            .iter()
            .enumerate()
            .map(|(index, path)| ImportImageFilePayload {
                original_filename: format!("frames_sheet_{:03}.png", index + 1),
                bytes: std::fs::read(path).unwrap(),
            })
            .collect();
        let error = reimport_gif_frame_sheet(
            &connection,
            &paths,
            GifFrameSheetReimportRequest {
                manifest_path: String::new(),
                manifest_file: Some(ImportImageFilePayload {
                    original_filename: "frames_manifest.json".to_string(),
                    bytes: serde_json::to_vec(&manifest).unwrap(),
                }),
                edited_frame_sheet_paths: Vec::new(),
                edited_frame_sheet_files,
                target_icon_id: "icon_missing".to_string(),
                create_variant: true,
                set_active_variant: false,
                target_profile_id: None,
            },
        )
        .unwrap_err();

        assert_eq!(error.code, "not_found");
        assert!(!paths
            .processed_variants_dir
            .join("gif_frame_reimports")
            .exists());
        let variant_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM processed_asset_variants", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(variant_count, 0);

        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn gif_frame_reimport_crops_validated_image_snapshot_after_paths_are_removed() {
        let mut connection = connection();
        let paths = temp_paths("pmtconcon-gif-frame-snapshot");
        let (icon_id, _) = seed_gif_icon(&mut connection, &paths);
        let export = export_gif_frame_sheet(
            &connection,
            &paths,
            GifFrameSheetExportRequest {
                icon_id,
                settings: settings(),
            },
        )
        .unwrap();
        let validation = validate_reimport_inputs(
            export.manifest_path.unwrap(),
            None,
            export.frame_sheet_paths.clone(),
            Vec::new(),
        )
        .unwrap();
        assert!(validation.public.errors.is_empty());
        for path in export.frame_sheet_paths {
            std::fs::remove_file(path).unwrap();
        }

        let frames = crop_reimport_frames(&validation.manifest, &validation.page_images).unwrap();
        assert_eq!(frames.len(), 4);

        std::fs::remove_dir_all(paths.root).unwrap();
    }
    #[test]
    fn gif_path_aggregate_limit_is_checked_before_png_decode() {
        let paths = temp_paths("pmtconcon-gif-frame-path-budget");
        let invalid_png_path = paths.root.join("invalid.png");
        std::fs::write(&invalid_png_path, b"not a png").unwrap();
        let source = PageImageSource::Path(invalid_png_path);
        let mut total_encoded_bytes = MAX_REIMPORT_TOTAL_ENCODED_BYTES;

        let error = load_page_image_source(&source, &mut total_encoded_bytes).unwrap_err();

        assert_eq!(error.code, "manifest_workload");
        assert_eq!(total_encoded_bytes, MAX_REIMPORT_TOTAL_ENCODED_BYTES);
        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn gif_frame_export_rejects_oversized_sheet_settings_before_output() {
        let mut connection = connection();
        let paths = temp_paths("pmtconcon-gif-frame-export-workload");
        let (icon_id, _) = seed_gif_icon(&mut connection, &paths);
        let rejected_output = paths.root.join("rejected-output");
        let mut huge = settings();
        huge.columns = 1_000_000_000;
        huge.max_sheet_width = 1_000_000_000;
        huge.output_directory = Some(rejected_output.to_string_lossy().to_string());
        let error = export_gif_frame_sheet(
            &connection,
            &paths,
            GifFrameSheetExportRequest {
                icon_id: icon_id.clone(),
                settings: huge,
            },
        )
        .unwrap_err();
        assert_eq!(error.code, "validation");
        assert!(!rejected_output.exists());

        let mut oversized_cell = settings();
        oversized_cell.frame_cell_width = i64::MAX;
        let error = analyze_gif_frame_sheet_export(
            &connection,
            AnalyzeGifFrameSheetExportRequest {
                icon_id: icon_id.clone(),
                settings: oversized_cell,
            },
        )
        .unwrap_err();
        assert_eq!(error.code, "validation");

        let mut overflowing = settings();
        overflowing.gap_x = i64::MAX;
        overflowing.border_y = i64::MAX;
        overflowing.max_sheet_height = i64::MAX;
        assert!(analyze_gif_frame_sheet_export(
            &connection,
            AnalyzeGifFrameSheetExportRequest {
                icon_id,
                settings: overflowing,
            },
        )
        .is_err());

        std::fs::remove_dir_all(paths.root).unwrap();
    }
}
