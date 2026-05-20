use std::collections::{HashMap, HashSet};
use std::fmt::Write as FmtWrite;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Cursor};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use image::codecs::gif::{GifDecoder, GifEncoder, Repeat as ImageGifRepeat};
use image::imageops::{self, FilterType};
use image::{AnimationDecoder, Delay, DynamicImage, Frame, ImageFormat, Rgba, RgbaImage};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::db::repositories::optimization::{insert_variant, NewProcessedAssetVariant};
use crate::error::{AppError, AppResult};
use crate::ids::create_id;
use crate::imaging::gif_pipeline::{
    is_pingpong_loop_mode, output_repeat_for_settings, GifOutputRepeat,
};
use crate::models::ImportImageFilePayload;
use crate::optimization::analyzer::{analyze_file, load_target, move_temp_file};
use crate::optimization::cache::hash_text;
use crate::paths::AppPaths;

use super::grid::{split_pages, PageCellPlacement, PageSplitSettings};
use super::manifest::{
    read_gif_manifest, read_gif_manifest_bytes, validate_gif_manifest, write_gif_manifest,
    GifFrameManifestItem, GifFrameSheetManifest, GifFrameSheetPage, APP_NAME,
    GIF_FRAME_SHEET_SCHEMA,
};
use super::path_string;

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
}

#[derive(Debug, Clone)]
struct DecodedFrame {
    image: RgbaImage,
    duration_ms: i64,
    source_frame_hash: String,
}

#[derive(Debug)]
struct ReimportValidationInternal {
    manifest: GifFrameSheetManifest,
    page_sources: HashMap<i64, PageImageSource>,
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
        warnings.push(
            "선택한 대상 GIF 아이콘과 매니페스트의 icon_id가 다릅니다. 매니페스트 매핑을 기준으로 처리했습니다."
                .to_string(),
        );
    }

    let frames = crop_reimport_frames(&validation.manifest, &validation.page_sources)?;
    let variant_id = create_id("variant");
    let output_dir = paths
        .processed_variants_dir
        .join("gif_frame_reimports")
        .join(&validation.manifest.icon_id);
    fs::create_dir_all(&output_dir)?;
    let output_path = output_dir.join(format!("{variant_id}.gif"));
    let temp_path = output_path.with_extension("gif.tmp");
    write_gif_atomic(
        &temp_path,
        &output_path,
        frames,
        repeat_from_manifest(&validation.manifest)?,
    )?;

    let file_analysis = analyze_file(&output_path, "gif")?;
    let metadata = fs::metadata(&output_path)?;
    let settings_json = serde_json::json!({
        "source": "gif_frame_sheet_reimport",
        "schema": validation.manifest.schema,
        "frameCellWidth": validation.manifest.frame_cell_width,
        "frameCellHeight": validation.manifest.frame_cell_height,
        "frameCount": validation.manifest.frame_count,
    })
    .to_string();

    let mut profile_id = None;
    let mut piece_id = None;
    let mut crop_hash = hash_text(&[
        "gif_frame_sheet_reimport".to_string(),
        validation.manifest.icon_id.clone(),
        validation.manifest.frame_count.to_string(),
    ]);
    let mut profile_hash = "gif_frame_sheet_reimport".to_string();
    let mut source_hash = validation
        .manifest
        .source_hash
        .clone()
        .unwrap_or_else(|| sha256_hex(settings_json.as_bytes()));
    let mut source_file_id = validation.manifest.source_file_id.clone();
    let mut active_variant_set = false;

    if request.set_active_variant {
        if let Some(target_profile_id) = request.target_profile_id.as_deref() {
            match load_target(
                connection,
                &validation.manifest.icon_id,
                target_profile_id,
                None,
            ) {
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
                    } else {
                        profile_id = Some(target.profile.id.clone());
                        piece_id = Some(target.piece_id.clone());
                        crop_hash = target.crop_hash;
                        profile_hash = target.profile_hash;
                        source_hash = target.source_hash;
                        source_file_id = Some(target.source_file_id);
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

    let variant = insert_variant(
        connection,
        &NewProcessedAssetVariant {
            id: variant_id.clone(),
            icon_id: validation.manifest.icon_id.clone(),
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
    )?;

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

fn load_gif_icon(connection: &Connection, icon_id: &str) -> AppResult<GifIconRecord> {
    connection
        .query_row(
            "SELECT
               i.id,
               i.source_file_id,
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
               i.gif_loop_count
             FROM icons i
             JOIN source_files s ON s.id = i.source_file_id
             JOIN crop_settings cs ON cs.icon_id = i.id
             WHERE i.id = ?1
               AND i.deleted_at IS NULL
               AND i.icon_kind = 'image'",
            params![icon_id],
            |row| {
                Ok(GifIconRecord {
                    id: row.get("id")?,
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
                })
            },
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("GIF 프레임 시트로 내보낼 아이콘을 찾을 수 없습니다."))
        .and_then(|icon| {
            if icon.source_extension != "gif" {
                return Err(AppError::new(
                    "validation",
                    "GIF 프레임 시트는 GIF 아이콘에서만 내보낼 수 있습니다.",
                ));
            }
            if !icon.source_is_animated {
                return Err(AppError::new(
                    "validation",
                    "프레임 시트로 내보낼 애니메이션 프레임이 없는 GIF입니다.",
                ));
            }
            Ok(icon)
        })
}

fn decode_rendered_frames(
    icon: &GifIconRecord,
    settings: &GifFrameSheetSettings,
) -> AppResult<Vec<DecodedFrame>> {
    let file = File::open(&icon.source_path)?;
    let decoder = GifDecoder::new(BufReader::new(file))?;
    let frames = decoder.into_frames().collect_frames()?;
    if frames.is_empty() {
        return Err(AppError::new("gif", "GIF 프레임을 찾을 수 없습니다."));
    }

    let viewport_width = viewport_width(&icon.shape, settings.frame_cell_width.max(1));
    let viewport_height = viewport_height(&icon.shape, settings.frame_cell_height.max(1));
    let mut decoded = Vec::with_capacity(frames.len());

    for frame in frames {
        let delay = frame.delay();
        let duration_ms = delay_ms(delay);
        let source_frame = DynamicImage::ImageRgba8(frame.into_buffer());
        let viewport = crop_and_resize(
            &source_frame,
            icon.crop_x,
            icon.crop_y,
            icon.crop_w,
            icon.crop_h,
            viewport_width,
            viewport_height,
        )?;
        let rendered_frame = match icon.shape.as_str() {
            "single" => viewport,
            "horizontal_double" => imageops::resize(
                &viewport,
                settings.frame_cell_width.max(1) as u32,
                settings.frame_cell_height.max(1) as u32,
                FilterType::Lanczos3,
            ),
            "vertical_double" => imageops::resize(
                &viewport,
                settings.frame_cell_width.max(1) as u32,
                settings.frame_cell_height.max(1) as u32,
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
    }

    if is_pingpong_loop_mode(&icon.gif_loop_mode) && decoded.len() > 2 {
        let reflected = decoded[1..decoded.len() - 1]
            .iter()
            .rev()
            .cloned()
            .collect::<Vec<_>>();
        decoded.extend(reflected);
    }

    Ok(decoded)
}

fn page_split_for_settings(
    item_count: usize,
    settings: &GifFrameSheetSettings,
) -> AppResult<super::grid::PageSplitPlan> {
    validate_export_settings(settings)?;
    let effective_max_height = if let Some(frames_per_page) = settings.frames_per_page {
        let rows = ((frames_per_page.max(1) + settings.columns.max(1) - 1)
            / settings.columns.max(1))
        .max(1);
        settings.max_sheet_height.min(sheet_extent(
            rows,
            settings.frame_cell_height.max(1),
            settings.gap_y.max(0),
            settings.border_y.max(0),
        ))
    } else {
        settings.max_sheet_height
    };

    split_pages(
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
    )
}

fn validate_export_settings(settings: &GifFrameSheetSettings) -> AppResult<()> {
    if settings.frame_cell_width <= 0 || settings.frame_cell_height <= 0 || settings.columns <= 0 {
        return Err(AppError::new(
            "validation",
            "GIF 프레임 시트 셀 크기와 열 수는 1 이상이어야 합니다.",
        ));
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

fn render_frame_sheet_page(
    frames: &[DecodedFrame],
    placements: &[&PageCellPlacement],
    width: i64,
    height: i64,
    background: &str,
    guide: bool,
) -> AppResult<RgbaImage> {
    let mut sheet = background_image(width.max(1) as u32, height.max(1) as u32, background, guide);
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
    let manifest = match manifest_file {
        Some(file) => read_gif_manifest_bytes(&file.bytes)?,
        None => read_gif_manifest(&manifest_path)?,
    };
    validate_gif_manifest(&manifest)?;

    let page_sources = resolve_page_sources(
        &manifest,
        &manifest_path,
        &edited_frame_sheet_paths,
        &edited_frame_sheet_files,
    );
    let mut warnings = Vec::new();
    let mut errors = Vec::new();
    let mut missing_pages = Vec::new();
    let mut wrong_dimension_pages = Vec::new();
    let mut detected_frame_indexes = HashSet::new();

    if edited_frame_sheet_paths.len() + edited_frame_sheet_files.len() > manifest.pages.len() {
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

        let image = match load_page_image_source(source) {
            Ok(image) => image,
            Err(error) => {
                errors.push(format!(
                    "{} 페이지를 읽을 수 없습니다: {}",
                    page.page_index + 1,
                    error.message
                ));
                continue;
            }
        };

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
            if frame.x < 0
                || frame.y < 0
                || frame.w <= 0
                || frame.h <= 0
                || frame.x + frame.w > i64::from(image.width())
                || frame.y + frame.h > i64::from(image.height())
            {
                errors.push(format!(
                    "frame {} 셀 영역이 {} 페이지 이미지 밖으로 벗어났습니다.",
                    frame.frame_index,
                    page.page_index + 1
                ));
            } else {
                detected_frame_indexes.insert(frame.frame_index);
            }
        }
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
        page_sources,
    })
}

fn resolve_page_sources(
    manifest: &GifFrameSheetManifest,
    manifest_path: &Path,
    explicit_paths: &[String],
    explicit_files: &[ImportImageFilePayload],
) -> HashMap<i64, PageImageSource> {
    let paths = explicit_paths
        .iter()
        .map(|path| PathBuf::from(path.trim()))
        .filter(|path| !path.as_os_str().is_empty())
        .collect::<Vec<_>>();
    let files = explicit_files
        .iter()
        .map(|file| (file.original_filename.clone(), file.bytes.clone()))
        .collect::<HashMap<_, _>>();
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
            let same_dir = manifest_dir.join(&page.clean_sheet_file);
            if same_dir.exists() {
                output.insert(page.page_index, PageImageSource::Path(same_dir));
                continue;
            }
        }
    }

    for page in &manifest.pages {
        if let Some(bytes) = files.get(&page.clean_sheet_file) {
            output.insert(page.page_index, PageImageSource::Bytes(bytes.clone()));
            continue;
        }
        if explicit_files.len() == manifest.pages.len() {
            let mut pages = manifest.pages.iter().collect::<Vec<_>>();
            pages.sort_by_key(|page| page.page_index);
            if let Some(position) = pages
                .iter()
                .position(|candidate| candidate.page_index == page.page_index)
            {
                if let Some(file) = explicit_files.get(position) {
                    output.insert(page.page_index, PageImageSource::Bytes(file.bytes.clone()));
                }
            }
        }
    }

    output
}

fn crop_reimport_frames(
    manifest: &GifFrameSheetManifest,
    page_sources: &HashMap<i64, PageImageSource>,
) -> AppResult<Vec<Frame>> {
    let mut page_cache = HashMap::new();
    for page in &manifest.pages {
        if let Some(source) = page_sources.get(&page.page_index) {
            page_cache.insert(page.page_index, load_page_image_source(source)?);
        }
    }

    let mut frame_items = manifest.frames.clone();
    frame_items.sort_by_key(|frame| frame.frame_index);
    let mut frames = Vec::with_capacity(frame_items.len());

    for item in frame_items {
        let page = page_cache.get(&item.page_index).ok_or_else(|| {
            AppError::new(
                "validation",
                format!(
                    "frame {}의 페이지 이미지를 읽을 수 없습니다.",
                    item.frame_index
                ),
            )
        })?;
        let cropped = imageops::crop_imm(
            page,
            item.x as u32,
            item.y as u32,
            item.w as u32,
            item.h as u32,
        )
        .to_image();
        frames.push(Frame::from_parts(
            cropped,
            0,
            0,
            Delay::from_numer_denom_ms(item.duration_ms.max(1) as u32, 1),
        ));
    }

    Ok(frames)
}

fn load_page_image_source(source: &PageImageSource) -> AppResult<RgbaImage> {
    match source {
        PageImageSource::Path(path) => Ok(image::open(path)?.to_rgba8()),
        PageImageSource::Bytes(bytes) => {
            Ok(image::load_from_memory_with_format(bytes, ImageFormat::Png)?.to_rgba8())
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
    image: &DynamicImage,
    crop_x: f64,
    crop_y: f64,
    crop_w: f64,
    crop_h: f64,
    width: i64,
    height: i64,
) -> AppResult<RgbaImage> {
    if crop_w <= 0.0 || crop_h <= 0.0 {
        return Err(AppError::new(
            "validation",
            "올바르지 않은 crop 영역입니다.",
        ));
    }
    let cropped = crop_with_padding(image, crop_x, crop_y, crop_w, crop_h);
    Ok(imageops::resize(
        &cropped,
        width.max(1) as u32,
        height.max(1) as u32,
        FilterType::Lanczos3,
    ))
}

fn crop_with_padding(
    image: &DynamicImage,
    crop_x: f64,
    crop_y: f64,
    crop_w: f64,
    crop_h: f64,
) -> RgbaImage {
    let source = image.to_rgba8();
    let crop_x = crop_x.round() as i64;
    let crop_y = crop_y.round() as i64;
    let crop_width = crop_w.round().max(1.0) as u32;
    let crop_height = crop_h.round().max(1.0) as u32;
    let mut output = RgbaImage::from_pixel(crop_width, crop_height, Rgba([0, 0, 0, 0]));
    let source_width = i64::from(source.width());
    let source_height = i64::from(source.height());
    let src_x = crop_x.max(0);
    let src_y = crop_y.max(0);
    let dst_x = (-crop_x).max(0);
    let dst_y = (-crop_y).max(0);
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
    if icon.gif_loop_mode == "preserve" {
        icon.source_gif_loop_mode.clone()
    } else {
        icon.gif_loop_mode.clone()
    }
}

fn effective_loop_count(icon: &GifIconRecord) -> Option<i64> {
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
    use std::time::{SystemTime, UNIX_EPOCH};

    use image::{AnimationDecoder, DynamicImage, ImageBuffer, ImageFormat, Rgba};
    use rusqlite::Connection;

    use crate::db::migrations;
    use crate::db::repositories::collections::create_collection;
    use crate::db::repositories::imports::import_image_files;
    use crate::models::ImportImageFilePayload;
    use crate::paths::AppPaths;

    use super::{
        analyze_gif_frame_sheet_export, export_gif_frame_sheet, reimport_gif_frame_sheet,
        validate_gif_frame_sheet_reimport, AnalyzeGifFrameSheetExportRequest,
        GifFrameSheetExportRequest, GifFrameSheetReimportRequest, GifFrameSheetSettings,
        ValidateGifFrameSheetReimportRequest,
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
        assert_eq!(manifest["schema"], "pmtcon-gif-frame-sheet-v1");
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
}
