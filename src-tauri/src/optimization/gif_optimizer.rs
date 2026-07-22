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
    if is_playback_fps_only_request(advanced_settings) {
        let playback_fps = advanced_settings
            .and_then(|settings| settings.playback_fps)
            .unwrap_or(24);
        return Ok(vec![encode_playback_fps_only_candidate(
            connection,
            paths,
            baseline,
            playback_fps,
        )?]);
    }

    let decoded = decode_gif(&baseline.path)?;
    let mut candidates = Vec::new();
    let presets = vec![
        OptimizationPreset::Quality,
        OptimizationPreset::Balanced,
        OptimizationPreset::Smallest,
    ];

    for preset in presets {
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

fn encode_playback_fps_only_candidate(
    connection: &Connection,
    paths: &AppPaths,
    baseline: &RenderedBaseline,
    playback_fps: i64,
) -> AppResult<OptimizationCandidateDto> {
    let frame_count = baseline.frame_count.unwrap_or(1).max(1) as usize;
    let mut settings = gif_settings_for_preset(OptimizationPreset::Quality, frame_count);
    settings.playback_fps = Some(playback_fps.clamp(1, 60));
    settings.frame_step = 1;
    settings.fps_limit = None;
    settings.color_limit = None;

    let settings_json = serde_json::to_string(&settings)
        .map_err(|error| AppError::new("json", error.to_string()))?;
    let settings_hash = hash_text(&[settings_json.clone()]);
    let variant_id = create_id("variant");
    let final_path = variant_path(paths, &baseline.target, &variant_id, "gif")?;
    let temp_path = temp_path_for(&final_path);

    write_gif_playback_fps_candidate_streaming(
        &temp_path,
        &baseline.path,
        settings.encoder_speed,
        settings.playback_fps,
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
            preset: Some(OptimizationPreset::Quality.as_str().to_string()),
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

fn is_playback_fps_only_request(
    advanced_settings: Option<&OptimizationAdvancedSettingsPayload>,
) -> bool {
    advanced_settings.is_some_and(|settings| {
        settings.playback_fps.is_some()
            && settings.fps_limit.is_none()
            && settings.frame_step.is_none()
            && settings.color_limit.is_none()
    })
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
        if let Some(color_limit) = advanced_settings.color_limit {
            settings.color_limit = Some(color_limit.clamp(2, 256));
        }
        if let Some(playback_fps) = advanced_settings.playback_fps {
            settings.playback_fps = Some(playback_fps.clamp(1, 60));
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
        settings.color_limit,
        settings.playback_fps,
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
    color_limit: Option<i64>,
    playback_fps: Option<i64>,
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
        quantize_rgba_in_place(&mut rgba, color_limit);
        let mut output_frame =
            Frame::from_rgba_speed(decoded.width, decoded.height, &mut rgba, encoder_speed);
        output_frame.delay = playback_delay_cs(playback_fps)
            .unwrap_or_else(|| pending_delay.min(u32::from(u16::MAX)) as u16);
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

pub(crate) fn write_gif_playback_fps_candidate_streaming(
    path: &Path,
    source_path: &Path,
    encoder_speed: i32,
    playback_fps: Option<i64>,
) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut options = DecodeOptions::new();
    options.set_color_output(ColorOutput::RGBA);
    let input = fs::File::open(source_path)?;
    let mut reader = options.read_info(BufReader::new(input))?;
    let width = reader.width();
    let height = reader.height();
    let repeat = reader.repeat();
    let output = fs::File::create(path)?;
    let mut encoder = Encoder::new(BufWriter::new(output), width, height, &[])
        .map_err(|error| AppError::new("gif", error.to_string()))?;
    encoder
        .set_repeat(repeat)
        .map_err(|error| AppError::new("gif", error.to_string()))?;
    let delay = playback_delay_cs(playback_fps).unwrap_or(1);
    let mut wrote_any = false;

    while let Some(frame) = reader.read_next_frame()? {
        let mut rgba = frame.buffer.to_vec();
        let mut output_frame = Frame::from_rgba_speed(width, height, &mut rgba, encoder_speed);
        output_frame.delay = delay;
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

fn playback_delay_cs(playback_fps: Option<i64>) -> Option<u16> {
    playback_fps.map(|fps| {
        let fps = fps.clamp(1, 60) as f64;
        ((100.0 / fps).round() as u16).max(1)
    })
}

fn quantize_rgba_in_place(rgba: &mut [u8], color_limit: Option<i64>) {
    let Some(color_limit) = color_limit else {
        return;
    };
    if color_limit >= 256 {
        return;
    }

    let levels = levels_for_color_limit(color_limit);
    if levels >= 256 {
        return;
    }

    for pixel in rgba.chunks_exact_mut(4) {
        if pixel[3] == 0 {
            pixel[0] = 0;
            pixel[1] = 0;
            pixel[2] = 0;
            continue;
        }
        pixel[0] = quantize_channel(pixel[0], levels);
        pixel[1] = quantize_channel(pixel[1], levels);
        pixel[2] = quantize_channel(pixel[2], levels);
    }
}

fn levels_for_color_limit(color_limit: i64) -> u16 {
    match color_limit {
        0..=32 => 3,
        33..=64 => 4,
        65..=128 => 5,
        129..=192 => 6,
        193..=224 => 7,
        _ => 8,
    }
}

fn quantize_channel(value: u8, levels: u16) -> u8 {
    if levels <= 1 {
        return 0;
    }
    let max_index = levels - 1;
    let index = ((u16::from(value) * max_index) + 127) / 255;
    ((index * 255) / max_index) as u8
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

#[cfg(test)]
mod tests {
    use super::{levels_for_color_limit, playback_delay_cs, quantize_rgba_in_place};

    #[test]
    fn color_limit_quantization_reduces_channel_variants_and_preserves_alpha() {
        let mut rgba = vec![12, 34, 56, 255, 200, 210, 220, 0];

        quantize_rgba_in_place(&mut rgba, Some(64));

        assert_eq!(levels_for_color_limit(64), 4);
        assert_eq!(rgba[3], 255);
        assert_eq!(&rgba[4..8], &[0, 0, 0, 0]);
        assert!(rgba[0] % 85 == 0);
        assert!(rgba[1] % 85 == 0);
        assert!(rgba[2] % 85 == 0);
    }

    #[test]
    fn playback_fps_maps_to_centisecond_delay() {
        assert_eq!(playback_delay_cs(Some(25)), Some(4));
        assert_eq!(playback_delay_cs(Some(10)), Some(10));
        assert_eq!(playback_delay_cs(None), None);
    }
}
