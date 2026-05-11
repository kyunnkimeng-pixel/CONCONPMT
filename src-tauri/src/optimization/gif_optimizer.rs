use std::fs;
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};

use gif::{ColorOutput, DecodeOptions, Encoder, Frame, Repeat};
use rusqlite::Connection;

use crate::db::repositories::optimization::{insert_variant, NewProcessedAssetVariant};
use crate::error::{AppError, AppResult};
use crate::ids::create_id;
use crate::models::OptimizationAdvancedSettingsPayload;
use crate::models::OptimizationCandidateDto;
use crate::optimization::analyzer::{analyze_file, move_temp_file, variant_path, RenderedBaseline};
use crate::optimization::cache::hash_text;
use crate::optimization::settings::{gif_settings_for_preset, OptimizationPreset};
use crate::paths::AppPaths;

#[derive(Debug, Clone)]
struct DecodedGifFrame {
    rgba: Vec<u8>,
    delay: u16,
}

#[derive(Debug, Clone)]
struct DecodedGif {
    width: u16,
    height: u16,
    repeat: Repeat,
    frames: Vec<DecodedGifFrame>,
}

pub fn generate_candidates(
    connection: &Connection,
    paths: &AppPaths,
    baseline: &RenderedBaseline,
    advanced_settings: Option<&OptimizationAdvancedSettingsPayload>,
) -> AppResult<Vec<OptimizationCandidateDto>> {
    let decoded = decode_gif(&baseline.path)?;
    let mut candidates = Vec::new();

    for preset in [
        OptimizationPreset::Quality,
        OptimizationPreset::Balanced,
        OptimizationPreset::Smallest,
    ] {
        match encode_candidate(
            connection,
            paths,
            baseline,
            &decoded,
            preset,
            advanced_settings,
        ) {
            Ok(candidate) => candidates.push(candidate),
            Err(_error) => {
                continue;
            }
        }
    }

    if candidates.is_empty() {
        return Err(AppError::new(
            "optimization",
            "GIF 최적화 후보를 생성하지 못했습니다.",
        ));
    }

    Ok(candidates)
}

fn encode_candidate(
    connection: &Connection,
    paths: &AppPaths,
    baseline: &RenderedBaseline,
    decoded: &DecodedGif,
    preset: OptimizationPreset,
    advanced_settings: Option<&OptimizationAdvancedSettingsPayload>,
) -> AppResult<OptimizationCandidateDto> {
    let mut settings = gif_settings_for_preset(preset, decoded.frames.len());
    if let Some(advanced_settings) = advanced_settings {
        if let Some(frame_step) = advanced_settings.frame_step {
            settings.frame_step = usize::try_from(frame_step.max(1)).unwrap_or(1);
        } else if let Some(fps_limit) = advanced_settings.fps_limit {
            settings.frame_step = frame_step_for_fps(decoded, fps_limit);
            settings.fps_limit = Some(fps_limit.max(1));
        }
    }
    let settings_json = serde_json::to_string(&settings)
        .map_err(|error| AppError::new("json", error.to_string()))?;
    let settings_hash = hash_text(&[settings_json.clone()]);
    let variant_id = create_id("variant");
    let final_path = variant_path(paths, &baseline.target, &variant_id, "gif")?;
    let temp_path = temp_path_for(&final_path);

    write_gif_candidate(
        &temp_path,
        decoded,
        settings.frame_step,
        settings.encoder_speed,
    )?;
    move_temp_file(&temp_path, &final_path)?;
    let byte_size = file_size(&final_path)?;
    let file_analysis = analyze_file(&final_path, "gif")?;

    let variant = insert_variant(
        connection,
        &NewProcessedAssetVariant {
            id: variant_id,
            icon_id: baseline.target.icon_id.clone(),
            piece_id: Some(baseline.target.piece_id.clone()),
            profile_id: Some(baseline.target.profile.id.clone()),
            source_file_id: Some(baseline.target.source_file_id.clone()),
            kind: "optimized_gif".to_string(),
            preset: Some(preset.as_str().to_string()),
            path: path_string(&final_path),
            format: "gif".to_string(),
            width: file_analysis.width,
            height: file_analysis.height,
            byte_size,
            frame_count: file_analysis.frame_count,
            duration_ms: file_analysis.duration_ms,
            loop_mode: file_analysis.loop_mode,
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

fn frame_step_for_fps(decoded: &DecodedGif, fps_limit: i64) -> usize {
    let fps_limit = fps_limit.max(1) as f64;
    let duration_cs = decoded
        .frames
        .iter()
        .map(|frame| u64::from(frame.delay.max(1)))
        .sum::<u64>()
        .max(1);
    let duration_seconds = duration_cs as f64 / 100.0;
    let average_fps = decoded.frames.len() as f64 / duration_seconds;
    (average_fps / fps_limit).ceil().max(1.0) as usize
}

fn decode_gif(path: &Path) -> AppResult<DecodedGif> {
    let mut options = DecodeOptions::new();
    options.set_color_output(ColorOutput::RGBA);
    let file = fs::File::open(path)?;
    let mut reader = options.read_info(BufReader::new(file))?;
    let width = reader.width();
    let height = reader.height();
    let repeat = reader.repeat();
    let mut frames = Vec::new();

    while let Some(frame) = reader.read_next_frame()? {
        frames.push(DecodedGifFrame {
            rgba: frame.buffer.to_vec(),
            delay: frame.delay.max(1),
        });
    }

    if frames.is_empty() {
        return Err(AppError::new("gif", "GIF 프레임을 찾을 수 없습니다."));
    }

    Ok(DecodedGif {
        width,
        height,
        repeat,
        frames,
    })
}

fn write_gif_candidate(
    path: &Path,
    decoded: &DecodedGif,
    frame_step: usize,
    encoder_speed: i32,
) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = fs::File::create(path)?;
    let mut encoder = Encoder::new(BufWriter::new(file), decoded.width, decoded.height, &[])
        .map_err(|error| AppError::new("gif", error.to_string()))?;
    encoder
        .set_repeat(decoded.repeat)
        .map_err(|error| AppError::new("gif", error.to_string()))?;

    let step = frame_step.max(1);
    let mut pending_delay = 0_u32;
    let mut wrote_any = false;

    for (index, frame) in decoded.frames.iter().enumerate() {
        pending_delay = pending_delay.saturating_add(u32::from(frame.delay.max(1)));
        let should_write = index % step == 0 || index + 1 == decoded.frames.len();
        if !should_write {
            continue;
        }

        let mut rgba = frame.rgba.clone();
        let mut output_frame =
            Frame::from_rgba_speed(decoded.width, decoded.height, &mut rgba, encoder_speed);
        output_frame.delay = pending_delay.min(u32::from(u16::MAX)) as u16;
        pending_delay = 0;
        encoder
            .write_frame(&output_frame)
            .map_err(|error| AppError::new("gif", error.to_string()))?;
        wrote_any = true;
    }

    if !wrote_any {
        return Err(AppError::new("gif", "GIF 후보 프레임을 만들 수 없습니다."));
    }

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
