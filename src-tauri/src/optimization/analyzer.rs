use std::fs;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use image::AnimationDecoder;
use rusqlite::{params, Connection, OptionalExtension};

use crate::db::repositories::optimization::{insert_variant, NewProcessedAssetVariant};
use crate::error::{AppError, AppResult};
use crate::ids::create_id;
use crate::imaging::export_render::{
    render_icon_export, ExportCropRect, ExportRenderPiece, ExportRenderRequest,
};
use crate::imaging::text_overlay::{text_overlay_from_fields, TextOverlayRenderSpec};
use crate::models::{ExportAssetAnalysisDto, ExportProfileDto};
use crate::optimization::cache::hash_text;
use crate::paths::AppPaths;

#[derive(Debug, Clone)]
pub struct OptimizationTarget {
    pub icon_id: String,
    pub piece_id: String,
    pub piece_index: usize,
    pub profile: ExportProfileDto,
    pub source_file_id: String,
    pub source_path: PathBuf,
    pub source_extension: String,
    pub source_hash: String,
    pub source_gif_loop_mode: String,
    pub source_gif_loop_count: Option<i64>,
    pub shape: String,
    pub crop: ExportCropRect,
    pub cell_width: i64,
    pub cell_height: i64,
    pub output_format: String,
    pub gif_loop_mode: String,
    pub gif_loop_count: Option<i64>,
    pub text_overlay: Option<TextOverlayRenderSpec>,
    pub crop_hash: String,
    pub profile_hash: String,
}

#[derive(Debug, Clone)]
pub struct RenderedBaseline {
    pub target: OptimizationTarget,
    pub analysis: ExportAssetAnalysisDto,
    pub path: PathBuf,
    pub frame_count: Option<i64>,
    pub duration_ms: Option<i64>,
}

#[derive(Debug, Clone)]
struct RawProfile {
    profile: ExportProfileDto,
    collection_default_width: i64,
    collection_default_height: i64,
}

pub fn load_target(
    connection: &Connection,
    icon_id: &str,
    profile_id: &str,
    piece_id: Option<&str>,
) -> AppResult<OptimizationTarget> {
    let raw_profile = load_profile(connection, profile_id)?;
    let icon = connection
        .query_row(
            "SELECT
               i.id,
               i.source_file_id,
               i.display_name,
               i.shape,
               i.cell_width_override,
               i.cell_height_override,
               CASE WHEN i.gif_pingpong = 1 THEN 'pingpong' ELSE i.gif_loop_mode END AS gif_loop_mode,
               i.gif_loop_count,
               s.original_path_in_library,
               s.original_extension,
               s.sha256,
               s.is_animated,
               COALESCE(s.original_loop_mode, 'preserve') AS source_loop_mode,
               s.original_loop_count,
               cs.crop_x,
               cs.crop_y,
               cs.crop_w,
               cs.crop_h,
               i.text_overlay_enabled,
               i.text_overlay_text,
               i.text_overlay_font_path,
               i.text_overlay_font_size,
               i.text_overlay_x,
               i.text_overlay_y,
               i.text_overlay_color,
               i.text_overlay_stroke_color,
               i.text_overlay_stroke_width
             FROM icons i
             JOIN source_files s ON s.id = i.source_file_id
             JOIN crop_settings cs ON cs.icon_id = i.id
             WHERE i.id = ?1
               AND i.collection_id = ?2
               AND i.deleted_at IS NULL",
            params![icon_id, raw_profile.profile.collection_id],
            |row| {
                Ok((
                    row.get::<_, String>("id")?,
                    row.get::<_, String>("source_file_id")?,
                    row.get::<_, String>("display_name")?,
                    row.get::<_, String>("shape")?,
                    row.get::<_, Option<i64>>("cell_width_override")?,
                    row.get::<_, Option<i64>>("cell_height_override")?,
                    row.get::<_, String>("gif_loop_mode")?,
                    row.get::<_, Option<i64>>("gif_loop_count")?,
                    row.get::<_, String>("original_path_in_library")?,
                    row.get::<_, String>("original_extension")?,
                    row.get::<_, String>("sha256")?,
                    row.get::<_, i64>("is_animated")? != 0,
                    row.get::<_, String>("source_loop_mode")?,
                    row.get::<_, Option<i64>>("original_loop_count")?,
                    row.get::<_, f64>("crop_x")?,
                    row.get::<_, f64>("crop_y")?,
                    row.get::<_, f64>("crop_w")?,
                    row.get::<_, f64>("crop_h")?,
                    row.get::<_, i64>("text_overlay_enabled")? != 0,
                    row.get::<_, String>("text_overlay_text")?,
                    row.get::<_, Option<String>>("text_overlay_font_path")?,
                    row.get::<_, f64>("text_overlay_font_size")?,
                    row.get::<_, f64>("text_overlay_x")?,
                    row.get::<_, f64>("text_overlay_y")?,
                    row.get::<_, String>("text_overlay_color")?,
                    row.get::<_, String>("text_overlay_stroke_color")?,
                    row.get::<_, f64>("text_overlay_stroke_width")?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("최적화할 아이콘을 찾을 수 없습니다."))?;

    let piece = load_piece(connection, icon_id, piece_id)?;
    let source_extension = normalize_format(&icon.9);
    let output_format =
        output_format_for_icon(&raw_profile.profile.target_format, &source_extension);
    let cell_width = icon
        .4
        .unwrap_or(raw_profile.collection_default_width)
        .max(1);
    let cell_height = icon
        .5
        .unwrap_or(raw_profile.collection_default_height)
        .max(1);
    let crop = ExportCropRect {
        x: icon.14,
        y: icon.15,
        width: icon.16,
        height: icon.17,
    };
    let text_overlay = text_overlay_from_fields(
        icon.18,
        Some(icon.19.clone()),
        icon.20.clone(),
        Some(icon.21),
        Some(icon.22),
        Some(icon.23),
        Some(icon.24.clone()),
        Some(icon.25.clone()),
        Some(icon.26),
    )?;
    let crop_hash = crop_hash(
        &icon.3,
        &crop,
        cell_width,
        cell_height,
        piece.1,
        &icon.6,
        icon.7,
        text_overlay.as_ref(),
    );
    let profile_hash = profile_hash(
        &raw_profile.profile,
        &output_format,
        cell_width,
        cell_height,
        "lanczos3",
    );

    Ok(OptimizationTarget {
        icon_id: icon.0,
        piece_id: piece.0,
        piece_index: piece.1,
        profile: raw_profile.profile,
        source_file_id: icon.1,
        source_path: PathBuf::from(icon.8),
        source_extension,
        source_hash: icon.10,
        source_gif_loop_mode: icon.12,
        source_gif_loop_count: icon.13,
        shape: icon.3,
        crop,
        cell_width,
        cell_height,
        output_format,
        gif_loop_mode: icon.6,
        gif_loop_count: icon.7,
        text_overlay,
        crop_hash,
        profile_hash,
    })
}

pub fn render_baseline(
    connection: &Connection,
    paths: &AppPaths,
    icon_id: &str,
    profile_id: &str,
    piece_id: Option<&str>,
) -> AppResult<RenderedBaseline> {
    let target = load_target(connection, icon_id, profile_id, piece_id)?;
    if !target.source_path.is_file() {
        return Err(AppError::not_found("원본 파일을 찾을 수 없습니다."));
    }

    let variant_id = create_id("variant");
    let final_path = variant_path(paths, &target, &variant_id, &target.output_format)?;
    let temp_dir = paths.temp_export_dir.join("optimization").join(&variant_id);
    fs::create_dir_all(&temp_dir)?;

    let file_name = format!("baseline.{}", target.output_format);
    let rendered = render_icon_export(ExportRenderRequest {
        source_path: &target.source_path,
        source_extension: &target.source_extension,
        shape: &target.shape,
        crop: target.crop,
        cell_width: target.cell_width,
        cell_height: target.cell_height,
        output_format: &target.output_format,
        resize_filter: "lanczos3",
        gif_loop_mode: &target.gif_loop_mode,
        gif_loop_count: target.gif_loop_count,
        source_gif_loop_mode: &target.source_gif_loop_mode,
        source_gif_loop_count: target.source_gif_loop_count,
        text_overlay: target.text_overlay.clone(),
        output_dir: &temp_dir,
        pieces: &[ExportRenderPiece {
            piece_index: target.piece_index,
            file_name,
        }],
    });

    let rendered_path = match rendered {
        Ok(mut paths) => paths.pop().ok_or_else(|| {
            AppError::new("optimization", "baseline 후보 파일이 생성되지 않았습니다.")
        })?,
        Err(error) => {
            let _ = fs::remove_dir_all(&temp_dir);
            return Err(error);
        }
    };

    move_temp_file(&rendered_path, &final_path)?;
    let _ = fs::remove_dir_all(&temp_dir);

    let metadata = fs::metadata(&final_path)?;
    let byte_size = i64::try_from(metadata.len()).unwrap_or(i64::MAX);
    let file_analysis = analyze_file(&final_path, &target.output_format)?;
    let settings_json = serde_json::json!({
        "preset": "baseline",
        "source": "current_export_pipeline"
    })
    .to_string();

    insert_variant(
        connection,
        &NewProcessedAssetVariant {
            id: variant_id.clone(),
            icon_id: target.icon_id.clone(),
            piece_id: Some(target.piece_id.clone()),
            profile_id: Some(target.profile.id.clone()),
            source_file_id: Some(target.source_file_id.clone()),
            kind: "baseline_export".to_string(),
            preset: Some("baseline".to_string()),
            path: path_string(&final_path),
            format: target.output_format.clone(),
            width: file_analysis.width,
            height: file_analysis.height,
            byte_size,
            frame_count: file_analysis.frame_count,
            duration_ms: file_analysis.duration_ms,
            loop_mode: file_analysis.loop_mode.clone(),
            settings_json: settings_json.clone(),
            source_hash: target.source_hash.clone(),
            crop_hash: target.crop_hash.clone(),
            profile_hash: target.profile_hash.clone(),
            settings_hash: hash_text(&[settings_json]),
        },
    )?;

    let analysis = build_analysis(&target, &variant_id, byte_size, &file_analysis);

    Ok(RenderedBaseline {
        target,
        analysis,
        path: final_path,
        frame_count: file_analysis.frame_count,
        duration_ms: file_analysis.duration_ms,
    })
}

pub fn variant_path(
    paths: &AppPaths,
    target: &OptimizationTarget,
    variant_id: &str,
    format: &str,
) -> AppResult<PathBuf> {
    let directory = paths
        .processed_variants_dir
        .join(&target.icon_id)
        .join(&target.profile.id)
        .join(&target.piece_id);
    fs::create_dir_all(&directory)?;
    Ok(directory.join(format!("{variant_id}.{format}")))
}

pub fn move_temp_file(temp_path: &Path, final_path: &Path) -> AppResult<()> {
    if let Some(parent) = final_path.parent() {
        fs::create_dir_all(parent)?;
    }
    if final_path.exists() {
        fs::remove_file(final_path)?;
    }
    fs::rename(temp_path, final_path).or_else(|_| {
        fs::copy(temp_path, final_path)?;
        fs::remove_file(temp_path)
    })?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct FileAnalysis {
    pub width: i64,
    pub height: i64,
    pub frame_count: Option<i64>,
    pub duration_ms: Option<i64>,
    pub average_fps: Option<f64>,
    pub loop_mode: Option<String>,
    pub has_transparency: Option<bool>,
}

pub fn analyze_file(path: &Path, format: &str) -> AppResult<FileAnalysis> {
    if format == "gif" {
        analyze_gif_file_streaming(path)
    } else {
        analyze_static_file(path)
    }
}

fn analyze_static_file(path: &Path) -> AppResult<FileAnalysis> {
    let image = image::open(path)?;
    let rgba = image.to_rgba8();
    let has_transparency = rgba.pixels().any(|pixel| pixel.0[3] < 255);
    Ok(FileAnalysis {
        width: i64::from(rgba.width()),
        height: i64::from(rgba.height()),
        frame_count: None,
        duration_ms: None,
        average_fps: None,
        loop_mode: None,
        has_transparency: Some(has_transparency),
    })
}

fn analyze_gif_file_streaming(path: &Path) -> AppResult<FileAnalysis> {
    let mut options = gif::DecodeOptions::new();
    options.set_color_output(gif::ColorOutput::RGBA);
    let file = fs::File::open(path)?;
    let mut reader = options.read_info(BufReader::new(file))?;
    let width = i64::from(reader.width());
    let height = i64::from(reader.height());
    let loop_mode = match reader.repeat() {
        gif::Repeat::Infinite | gif::Repeat::Finite(0) => "infinite".to_string(),
        gif::Repeat::Finite(1) => "once".to_string(),
        gif::Repeat::Finite(count) => format!("count:{count}"),
    };
    let mut frame_count = 0_i64;
    let mut duration_ms = 0_i64;
    let mut has_transparency = false;

    while let Some(frame) = reader.read_next_frame()? {
        frame_count += 1;
        duration_ms += i64::from(frame.delay.max(1)) * 10;
        if frame.buffer.chunks_exact(4).any(|pixel| pixel[3] < 255) {
            has_transparency = true;
        }
    }

    if frame_count == 0 {
        return Err(AppError::new("gif", "GIF 프레임을 찾을 수 없습니다."));
    }

    let average_fps = if duration_ms > 0 {
        Some(frame_count as f64 / (duration_ms as f64 / 1000.0))
    } else {
        None
    };

    Ok(FileAnalysis {
        width,
        height,
        frame_count: Some(frame_count),
        duration_ms: Some(duration_ms),
        average_fps,
        loop_mode: Some(loop_mode),
        has_transparency: Some(has_transparency),
    })
}

#[allow(dead_code)]
fn analyze_gif_file(path: &Path) -> AppResult<FileAnalysis> {
    let file = fs::File::open(path)?;
    let decoder = image::codecs::gif::GifDecoder::new(BufReader::new(file))?;
    let frames = decoder.into_frames().collect_frames()?;
    if frames.is_empty() {
        return Err(AppError::new("gif", "GIF 프레임을 찾을 수 없습니다."));
    }

    let width = i64::from(frames[0].buffer().width());
    let height = i64::from(frames[0].buffer().height());
    let mut duration_ms = 0_i64;
    let mut has_transparency = false;
    for frame in &frames {
        let (numerator, denominator) = frame.delay().numer_denom_ms();
        let frame_ms = if denominator == 0 {
            0
        } else {
            i64::from(numerator / denominator)
        };
        duration_ms += frame_ms;
        if frame.buffer().pixels().any(|pixel| pixel.0[3] < 255) {
            has_transparency = true;
        }
    }
    let frame_count = i64::try_from(frames.len()).unwrap_or(i64::MAX);
    let average_fps = if duration_ms > 0 {
        Some(frame_count as f64 / (duration_ms as f64 / 1000.0))
    } else {
        None
    };

    Ok(FileAnalysis {
        width,
        height,
        frame_count: Some(frame_count),
        duration_ms: Some(duration_ms),
        average_fps,
        loop_mode: Some(loop_mode_for_gif(path)?),
        has_transparency: Some(has_transparency),
    })
}

#[allow(dead_code)]
fn loop_mode_for_gif(path: &Path) -> AppResult<String> {
    let mut options = gif::DecodeOptions::new();
    options.set_color_output(gif::ColorOutput::RGBA);
    let file = fs::File::open(path)?;
    let reader = options.read_info(BufReader::new(file))?;
    Ok(match reader.repeat() {
        gif::Repeat::Infinite | gif::Repeat::Finite(0) => "infinite".to_string(),
        gif::Repeat::Finite(1) => "once".to_string(),
        gif::Repeat::Finite(count) => format!("count:{count}"),
    })
}

fn build_analysis(
    target: &OptimizationTarget,
    baseline_variant_id: &str,
    baseline_bytes: i64,
    file_analysis: &FileAnalysis,
) -> ExportAssetAnalysisDto {
    let target_max_bytes = target.profile.max_bytes.max(1);
    let over_by_bytes = baseline_bytes - target_max_bytes;
    let over_ratio = baseline_bytes as f64 / target_max_bytes as f64;
    let status = if baseline_bytes <= target_max_bytes {
        "already_passes"
    } else {
        "oversized"
    }
    .to_string();
    let explanation_for_user = if baseline_bytes <= target_max_bytes {
        format!(
            "현재 파일은 {} / 제한 {}로 통과합니다. 추가 최적화가 필요 없습니다.",
            format_bytes(baseline_bytes),
            format_bytes(target_max_bytes)
        )
    } else {
        format!(
            "이 파일은 {}로 현재 프로필 제한 {}를 초과합니다. 자동 최적화 후보를 생성할 수 있습니다.",
            format_bytes(baseline_bytes),
            format_bytes(target_max_bytes)
        )
    };

    ExportAssetAnalysisDto {
        icon_id: target.icon_id.clone(),
        profile_id: target.profile.id.clone(),
        piece_id: target.piece_id.clone(),
        baseline_variant_id: baseline_variant_id.to_string(),
        baseline_bytes,
        target_max_bytes,
        over_by_bytes,
        over_ratio,
        format: target.output_format.clone(),
        width: file_analysis.width,
        height: file_analysis.height,
        frame_count: file_analysis.frame_count,
        duration_ms: file_analysis.duration_ms,
        average_fps: file_analysis.average_fps,
        loop_mode: file_analysis.loop_mode.clone(),
        has_transparency: file_analysis.has_transparency,
        status,
        explanation_for_user,
    }
}

fn load_profile(connection: &Connection, profile_id: &str) -> AppResult<RawProfile> {
    connection
        .query_row(
            "SELECT
               ep.id,
               ep.collection_id,
               ep.name,
               ep.profile_type,
               ep.target_format,
               ep.target_cell_width,
               ep.target_cell_height,
               ep.preview_width,
               ep.preview_height,
               ep.max_bytes,
               ep.allowed_formats_json,
               ep.filename_mode,
               ep.include_alt_txt,
               ep.strict_warnings,
               ep.created_at,
               ep.updated_at,
               c.default_cell_width,
               c.default_cell_height
             FROM export_profiles ep
             JOIN collections c ON c.id = ep.collection_id
             WHERE ep.id = ?1
               AND c.deleted_at IS NULL",
            params![profile_id],
            |row| {
                let allowed_formats_json: String = row.get("allowed_formats_json")?;
                let include_alt_txt: i64 = row.get("include_alt_txt")?;
                let strict_warnings: i64 = row.get("strict_warnings")?;
                Ok(RawProfile {
                    profile: ExportProfileDto {
                        id: row.get("id")?,
                        collection_id: row.get("collection_id")?,
                        name: row.get("name")?,
                        profile_type: row.get("profile_type")?,
                        target_format: normalize_format(&row.get::<_, String>("target_format")?),
                        target_cell_width: row.get("target_cell_width")?,
                        target_cell_height: row.get("target_cell_height")?,
                        preview_width: row.get("preview_width")?,
                        preview_height: row.get("preview_height")?,
                        max_bytes: row.get("max_bytes")?,
                        allowed_formats: allowed_formats_from_json(&allowed_formats_json),
                        filename_mode: row.get("filename_mode")?,
                        include_alt_txt: include_alt_txt != 0,
                        strict_warnings: strict_warnings != 0,
                        created_at: row.get("created_at")?,
                        updated_at: row.get("updated_at")?,
                    },
                    collection_default_width: row.get("default_cell_width")?,
                    collection_default_height: row.get("default_cell_height")?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("export 프로필을 찾을 수 없습니다."))
}

fn load_piece(
    connection: &Connection,
    icon_id: &str,
    piece_id: Option<&str>,
) -> AppResult<(String, usize)> {
    let mut statement = connection.prepare(
        "SELECT id, piece_index
         FROM icon_pieces
         WHERE icon_id = ?1
           AND (?2 IS NULL OR id = ?2)
         ORDER BY piece_index ASC
         LIMIT 1",
    )?;
    statement
        .query_row(params![icon_id, piece_id], |row| {
            let index: i64 = row.get("piece_index")?;
            Ok((
                row.get::<_, String>("id")?,
                usize::try_from(index.max(0)).unwrap_or(0),
            ))
        })
        .optional()?
        .ok_or_else(|| AppError::not_found("최적화할 export 조각을 찾을 수 없습니다."))
}

fn crop_hash(
    shape: &str,
    crop: &ExportCropRect,
    cell_width: i64,
    cell_height: i64,
    piece_index: usize,
    gif_loop_mode: &str,
    gif_loop_count: Option<i64>,
    text_overlay: Option<&TextOverlayRenderSpec>,
) -> String {
    let mut parts = vec![
        shape.to_string(),
        format!("{:.3}", crop.x),
        format!("{:.3}", crop.y),
        format!("{:.3}", crop.width),
        format!("{:.3}", crop.height),
        cell_width.to_string(),
        cell_height.to_string(),
        piece_index.to_string(),
        gif_loop_mode.to_string(),
        gif_loop_count.unwrap_or_default().to_string(),
    ];
    if let Some(text_overlay) = text_overlay {
        parts.extend(text_overlay.normalized_hash_parts());
    }
    hash_text(&parts)
}

fn profile_hash(
    profile: &ExportProfileDto,
    output_format: &str,
    cell_width: i64,
    cell_height: i64,
    resize_filter: &str,
) -> String {
    hash_text(&[
        profile.id.clone(),
        output_format.to_string(),
        profile.max_bytes.to_string(),
        cell_width.to_string(),
        cell_height.to_string(),
        resize_filter.to_string(),
    ])
}

fn allowed_formats_from_json(value: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(value)
        .unwrap_or_else(|_| vec!["jpg".to_string(), "png".to_string(), "gif".to_string()])
        .into_iter()
        .map(|format| normalize_format(&format))
        .collect()
}

fn output_format_for_icon(profile_format: &str, source_extension: &str) -> String {
    let source_format = normalize_format(source_extension);
    if source_format == "gif" {
        return "gif".to_string();
    }

    match normalize_format(profile_format).as_str() {
        "source" => source_format,
        "jpg" => "jpg".to_string(),
        "gif" => "gif".to_string(),
        _ => "png".to_string(),
    }
}

fn normalize_format(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "jpeg" | "jpg" => "jpg".to_string(),
        "gif" => "gif".to_string(),
        "source" => "source".to_string(),
        _ => "png".to_string(),
    }
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
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
