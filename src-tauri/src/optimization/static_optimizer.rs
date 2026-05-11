use std::fs;
use std::path::{Path, PathBuf};

use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, ImageFormat};
use rusqlite::Connection;

use crate::db::repositories::optimization::{insert_variant, NewProcessedAssetVariant};
use crate::error::{AppError, AppResult};
use crate::ids::create_id;
use crate::models::OptimizationAdvancedSettingsPayload;
use crate::models::OptimizationCandidateDto;
use crate::optimization::analyzer::{analyze_file, move_temp_file, variant_path, RenderedBaseline};
use crate::optimization::cache::hash_text;
use crate::optimization::settings::{static_settings_for_format, OptimizationPreset};
use crate::paths::AppPaths;

pub fn generate_candidates(
    connection: &Connection,
    paths: &AppPaths,
    baseline: &RenderedBaseline,
    advanced_settings: Option<&OptimizationAdvancedSettingsPayload>,
) -> AppResult<Vec<OptimizationCandidateDto>> {
    let mut candidates = Vec::new();

    for preset in [
        OptimizationPreset::Quality,
        OptimizationPreset::Balanced,
        OptimizationPreset::Smallest,
    ] {
        match encode_candidate(connection, paths, baseline, preset, advanced_settings) {
            Ok(candidate) => candidates.push(candidate),
            Err(_error) => continue,
        }
    }

    if candidates.is_empty() {
        return Err(AppError::new(
            "optimization",
            "정적 이미지 최적화 후보를 생성하지 못했습니다.",
        ));
    }

    Ok(candidates)
}

fn encode_candidate(
    connection: &Connection,
    paths: &AppPaths,
    baseline: &RenderedBaseline,
    preset: OptimizationPreset,
    advanced_settings: Option<&OptimizationAdvancedSettingsPayload>,
) -> AppResult<OptimizationCandidateDto> {
    let format = baseline.target.output_format.as_str();
    let mut settings = static_settings_for_format(preset, format);
    if format == "jpg" {
        if let Some(quality) = advanced_settings.and_then(|settings| settings.jpeg_quality) {
            settings.quality = Some(quality.clamp(1, 100));
        }
    }
    let settings_json = serde_json::to_string(&settings)
        .map_err(|error| AppError::new("json", error.to_string()))?;
    let settings_hash = hash_text(&[settings_json.clone()]);
    let variant_id = create_id("variant");
    let final_path = variant_path(paths, &baseline.target, &variant_id, format)?;
    let temp_path = temp_path_for(&final_path);

    match format {
        "jpg" => write_jpg_candidate(&baseline.path, &temp_path, settings.quality.unwrap_or(82))?,
        "png" => write_png_candidate(&baseline.path, &temp_path)?,
        _ => {
            return Err(AppError::new(
                "optimization",
                "지원하지 않는 정적 이미지 최적화 형식입니다.",
            ));
        }
    }

    move_temp_file(&temp_path, &final_path)?;
    let byte_size = file_size(&final_path)?;
    let file_analysis = analyze_file(&final_path, format)?;
    let kind = if format == "jpg" {
        "optimized_jpg"
    } else {
        "optimized_png"
    };

    let variant = insert_variant(
        connection,
        &NewProcessedAssetVariant {
            id: variant_id,
            icon_id: baseline.target.icon_id.clone(),
            piece_id: Some(baseline.target.piece_id.clone()),
            profile_id: Some(baseline.target.profile.id.clone()),
            source_file_id: Some(baseline.target.source_file_id.clone()),
            kind: kind.to_string(),
            preset: Some(preset.as_str().to_string()),
            path: path_string(&final_path),
            format: format.to_string(),
            width: file_analysis.width,
            height: file_analysis.height,
            byte_size,
            frame_count: None,
            duration_ms: None,
            loop_mode: None,
            settings_json,
            source_hash: baseline.target.source_hash.clone(),
            crop_hash: baseline.target.crop_hash.clone(),
            profile_hash: baseline.target.profile_hash.clone(),
            settings_hash,
        },
    )?;

    Ok(crate::db::repositories::optimization::to_candidate_dto(
        &variant,
        baseline.target.profile.max_bytes,
        baseline.frame_count,
        baseline.duration_ms,
    ))
}

fn write_jpg_candidate(source: &Path, output: &Path, quality: i64) -> AppResult<()> {
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    let rgb = image::open(source)?.to_rgb8();
    let file = fs::File::create(output)?;
    let mut encoder = JpegEncoder::new_with_quality(file, quality.clamp(1, 100) as u8);
    encoder
        .encode_image(&DynamicImage::ImageRgb8(rgb))
        .map_err(AppError::from)
}

fn write_png_candidate(source: &Path, output: &Path) -> AppResult<()> {
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    let image = image::open(source)?.to_rgba8();
    image.save_with_format(output, ImageFormat::Png)?;
    Ok(())
}

fn temp_path_for(final_path: &Path) -> PathBuf {
    final_path.with_extension("tmp")
}

fn file_size(path: &Path) -> AppResult<i64> {
    Ok(i64::try_from(fs::metadata(path)?.len()).unwrap_or(i64::MAX))
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}
