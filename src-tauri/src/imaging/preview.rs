use std::borrow::Cow;
use std::fs::{self, File};
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use image::codecs::gif::{GifDecoder, GifEncoder, Repeat as ImageGifRepeat};
use image::imageops::{self, FilterType};
use image::{AnimationDecoder, Frame, ImageFormat, Rgba, RgbaImage};

use crate::error::{AppError, AppResult};
use crate::imaging::effects::{apply_effect_recipe, validate_effect_recipe, EffectRecipe};
use crate::imaging::geometry::{piece_roles, viewport_size};
use crate::imaging::gif_pipeline::{
    is_pingpong_loop_mode, output_repeat_for_settings, pingpong_sequence, pingpong_sequence_len,
    GifOutputRepeat,
};
use crate::imaging::import_limits::{
    validate_crop_rect, validate_gif_workload, validate_import_dimensions, ValidatedCropRect,
};
use crate::imaging::motion::{
    apply_motion_recipe, static_motion_schedule, validate_motion_recipe, MotionFrameContext,
    MotionFrameTiming, MotionRecipe,
};
use crate::imaging::text_overlay::{apply_text_overlay, TextOverlayRenderSpec};
use crate::imaging::transform::{apply_image_transform, source_viewport_geometry, ImageTransform};
use crate::paths::AppPaths;

#[derive(Debug, Clone, Copy)]
pub struct CropRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug)]
pub struct GeneratePreviewRequest<'a> {
    pub collection_id: &'a str,
    pub icon_id: &'a str,
    pub source_path: &'a Path,
    pub source_extension: &'a str,
    pub shape: &'a str,
    pub crop: CropRect,
    pub cell_width: i64,
    pub cell_height: i64,
    pub transform: ImageTransform,
    pub gif_loop_mode: &'a str,
    pub gif_loop_count: Option<i64>,
    pub source_gif_loop_mode: Option<&'a str>,
    pub source_gif_loop_count: Option<i64>,
    pub text_overlay: Option<TextOverlayRenderSpec>,
    pub effects: EffectRecipe,
    pub motion: MotionRecipe,
}

#[derive(Debug)]
pub struct GeneratedPreview {
    pub current_preview_path: PathBuf,
    pub piece_paths: Vec<PathBuf>,
    pub poster_path: PathBuf,
    pub frame_count: usize,
    pub duration_ms: u64,
    pub effective_fps: f64,
    pub clipped_frame_count: usize,
    pub clipped_pixel_count: u64,
    pub encoded_byte_size: u64,
}

pub fn generate_icon_preview(
    paths: &AppPaths,
    request: GeneratePreviewRequest<'_>,
) -> AppResult<GeneratedPreview> {
    let preview_dir = paths
        .collection_previews_dir
        .join(request.collection_id)
        .join(request.icon_id);
    generate_icon_preview_in_directory(&preview_dir, request)
}

pub fn generate_icon_preview_in_directory(
    preview_dir: &Path,
    request: GeneratePreviewRequest<'_>,
) -> AppResult<GeneratedPreview> {
    validate_crop_rect(
        request.crop.x,
        request.crop.y,
        request.crop.width,
        request.crop.height,
    )?;
    validate_effect_recipe(&request.effects)?;
    validate_motion_recipe(&request.motion)?;
    let viewport = viewport_size(request.shape, request.cell_width, request.cell_height)?;
    let source_geometry = source_viewport_geometry(
        request.shape,
        request.cell_width,
        request.cell_height,
        request.transform,
    )?;
    let roles = piece_roles(request.shape)?;
    fs::create_dir_all(preview_dir)?;

    if request.source_extension == "gif" || request.motion.has_enabled_motion() {
        generate_gif_preview(
            &preview_dir,
            &request,
            source_geometry.viewport.width,
            source_geometry.viewport.height,
            viewport.width,
            viewport.height,
            roles.len(),
        )
    } else {
        generate_static_preview(
            &preview_dir,
            &request,
            source_geometry.viewport.width,
            source_geometry.viewport.height,
            viewport.width,
            viewport.height,
            roles.len(),
        )
    }
}

fn generate_static_preview(
    preview_dir: &Path,
    request: &GeneratePreviewRequest<'_>,
    source_viewport_width: i64,
    source_viewport_height: i64,
    output_viewport_width: i64,
    output_viewport_height: i64,
    piece_count: usize,
) -> AppResult<GeneratedPreview> {
    let image = image::open(request.source_path)?.to_rgba8();
    let image = image_with_text_overlay(&image, request.text_overlay.as_ref())?;
    let viewport = crop_and_resize(
        image.as_ref(),
        request.crop,
        source_viewport_width,
        source_viewport_height,
    )?;
    let viewport = apply_image_transform(viewport, request.transform)?;
    let mut viewport = viewport;
    apply_effect_recipe(&mut viewport, &request.effects)?;
    validate_transformed_viewport(&viewport, output_viewport_width, output_viewport_height)?;
    let current_preview_path = preview_dir.join("preview.png");
    viewport.save_with_format(&current_preview_path, ImageFormat::Png)?;

    let piece_paths = write_static_pieces(
        preview_dir,
        &viewport,
        request.shape,
        request.cell_width,
        request.cell_height,
        piece_count,
    )?;
    let encoded_byte_size = fs::metadata(&current_preview_path)?.len();

    Ok(GeneratedPreview {
        poster_path: current_preview_path.clone(),
        current_preview_path,
        piece_paths,
        frame_count: 1,
        duration_ms: 0,
        effective_fps: 0.0,
        clipped_frame_count: 0,
        clipped_pixel_count: 0,
        encoded_byte_size,
    })
}

fn generate_gif_preview(
    preview_dir: &Path,
    request: &GeneratePreviewRequest<'_>,
    source_viewport_width: i64,
    source_viewport_height: i64,
    output_viewport_width: i64,
    output_viewport_height: i64,
    piece_count: usize,
) -> AppResult<GeneratedPreview> {
    let frames = load_source_motion_frames(request)?;
    let is_pingpong = is_pingpong_loop_mode(request.gif_loop_mode);
    let output_frame_count = if is_pingpong {
        pingpong_sequence_len(frames.len())
    } else {
        frames.len()
    };
    let repeat = output_repeat_for_settings(
        request.gif_loop_mode,
        request.gif_loop_count,
        request.source_gif_loop_mode.unwrap_or("preserve"),
        request.source_gif_loop_count,
    )?;
    let output_width = u32::try_from(output_viewport_width)
        .map_err(|_| AppError::new("validation", "미리보기 너비가 올바르지 않습니다."))?;
    let output_height = u32::try_from(output_viewport_height)
        .map_err(|_| AppError::new("validation", "미리보기 높이가 올바르지 않습니다."))?;
    validate_import_dimensions(output_width, output_height)?;
    validate_gif_workload(
        output_width,
        output_height,
        i64::try_from(output_frame_count).unwrap_or(i64::MAX),
    )
    .map_err(|message| AppError::new("validation", message))?;

    let total_duration_ms = frames
        .iter()
        .map(|frame| frame.duration_ms)
        .sum::<u64>()
        .max(1);
    let reflected_duration_ms = if is_pingpong && frames.len() > 2 {
        frames[1..frames.len() - 1]
            .iter()
            .map(|frame| frame.duration_ms)
            .fold(0_u64, u64::saturating_add)
    } else {
        0
    };
    let output_duration_ms = total_duration_ms.saturating_add(reflected_duration_ms);
    let mut viewport_frames = Vec::with_capacity(output_frame_count);
    let mut piece_frames: Vec<Vec<Frame>> = (0..piece_count)
        .map(|_| Vec::with_capacity(output_frame_count))
        .collect();
    let mut clipping_stats = Vec::with_capacity(output_frame_count);
    let mut elapsed_ms = 0_u64;
    let final_frame_index = frames.len().saturating_sub(1);

    for (frame_index, frame) in frames.into_iter().enumerate() {
        let delay = frame.delay();
        let frame_duration_ms = frame.duration_ms;
        let source_frame =
            image_with_text_overlay(frame.image.as_ref(), request.text_overlay.as_ref())?;
        let viewport = crop_and_resize(
            source_frame.as_ref(),
            request.crop,
            source_viewport_width,
            source_viewport_height,
        )?;
        let viewport = apply_image_transform(viewport, request.transform)?;
        let mut viewport = viewport;
        apply_effect_recipe(&mut viewport, &request.effects)?;
        let context_elapsed_ms = if repeat == GifOutputRepeat::Once
            && frame_index == final_frame_index
            && request.motion.has_enabled_motion()
        {
            total_duration_ms
        } else {
            elapsed_ms
        };
        let motion_result = apply_motion_recipe(
            &viewport,
            &request.motion,
            MotionFrameContext {
                elapsed_ms: context_elapsed_ms,
                total_duration_ms,
            },
        )?;
        clipping_stats.push((
            motion_result.clipped_pixel_count > 0,
            motion_result.clipped_pixel_count,
        ));
        let viewport = motion_result.image;
        validate_transformed_viewport(&viewport, output_viewport_width, output_viewport_height)?;
        viewport_frames.push(Frame::from_parts(viewport.clone(), 0, 0, delay));

        for (piece_index, piece) in split_viewport(
            &viewport,
            request.shape,
            request.cell_width,
            request.cell_height,
            piece_count,
        )?
        .into_iter()
        .enumerate()
        {
            piece_frames[piece_index].push(Frame::from_parts(piece, 0, 0, delay));
        }
        elapsed_ms = elapsed_ms.saturating_add(frame_duration_ms);
    }

    if is_pingpong {
        pingpong_sequence(&mut viewport_frames);
        for frames in &mut piece_frames {
            pingpong_sequence(frames);
        }
        pingpong_sequence(&mut clipping_stats);
    }

    let frame_count = viewport_frames.len();
    let duration_ms = output_duration_ms.max(1);
    let effective_fps = frame_count as f64 * 1_000.0 / duration_ms as f64;
    let clipped_frame_count = clipping_stats
        .iter()
        .filter(|(is_clipped, _)| *is_clipped)
        .count();
    let clipped_pixel_count = clipping_stats
        .iter()
        .map(|(_, count)| *count)
        .fold(0_u64, u64::saturating_add);
    let poster_path = preview_dir.join("poster.png");
    viewport_frames
        .first()
        .ok_or_else(|| AppError::new("gif", "GIF 포스터 프레임을 찾을 수 없습니다."))?
        .buffer()
        .save_with_format(&poster_path, ImageFormat::Png)?;
    let current_preview_path = preview_dir.join("preview.gif");
    write_gif(&current_preview_path, viewport_frames, repeat)?;
    let encoded_byte_size = fs::metadata(&current_preview_path)?.len();

    let mut piece_paths = Vec::with_capacity(piece_count);
    for (piece_index, frames) in piece_frames.into_iter().enumerate() {
        let piece_path = preview_dir.join(format!("piece-{piece_index:02}.gif"));
        write_gif(&piece_path, frames, repeat)?;
        piece_paths.push(piece_path);
    }

    Ok(GeneratedPreview {
        current_preview_path,
        piece_paths,
        poster_path,
        frame_count,
        duration_ms,
        effective_fps,
        clipped_frame_count,
        clipped_pixel_count,
        encoded_byte_size,
    })
}
#[derive(Debug, Clone)]
struct SourceMotionFrame {
    image: Arc<RgbaImage>,
    delay: image::Delay,
    duration_ms: u64,
}

impl SourceMotionFrame {
    fn delay(&self) -> image::Delay {
        self.delay
    }
}

fn load_source_motion_frames(
    request: &GeneratePreviewRequest<'_>,
) -> AppResult<Vec<SourceMotionFrame>> {
    if request.source_extension == "gif" {
        let file = File::open(request.source_path)?;
        let decoder = GifDecoder::new(BufReader::new(file))?;
        let frames = decoder.into_frames().collect_frames()?;
        if frames.is_empty() {
            return Err(AppError::new("gif", "GIF 프레임을 찾을 수 없습니다."));
        }
        return Ok(frames
            .into_iter()
            .map(|frame| {
                let delay = frame.delay();
                SourceMotionFrame {
                    image: Arc::new(frame.into_buffer()),
                    delay,
                    duration_ms: delay_ms(delay),
                }
            })
            .collect());
    }

    let source = image::open(request.source_path)?.to_rgba8();
    let schedule = static_motion_schedule(&request.motion)?;
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
) -> Vec<SourceMotionFrame> {
    let source = Arc::new(source);
    schedule
        .into_iter()
        .map(|timing| SourceMotionFrame {
            image: Arc::clone(&source),
            delay: image::Delay::from_numer_denom_ms(timing.duration_ms, 1),
            duration_ms: u64::from(timing.duration_ms),
        })
        .collect()
}

fn delay_ms(delay: image::Delay) -> u64 {
    let (numerator, denominator) = delay.numer_denom_ms();
    if denominator == 0 {
        return 0;
    }

    u64::from(numerator).div_ceil(u64::from(denominator))
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

fn validate_transformed_viewport(
    viewport: &RgbaImage,
    expected_width: i64,
    expected_height: i64,
) -> AppResult<()> {
    if i64::from(viewport.width()) == expected_width
        && i64::from(viewport.height()) == expected_height
    {
        return Ok(());
    }

    Err(AppError::new(
        "validation",
        "회전 후 미리보기 크기가 출력 모양과 일치하지 않습니다.",
    ))
}

fn crop_and_resize(
    image: &RgbaImage,
    crop: CropRect,
    viewport_width: i64,
    viewport_height: i64,
) -> AppResult<RgbaImage> {
    let crop = validate_crop_rect(crop.x, crop.y, crop.width, crop.height)?;
    let cropped = crop_with_padding(image, crop);
    let width = u32::try_from(viewport_width)
        .map_err(|_| AppError::new("validation", "미리보기 너비가 올바르지 않습니다."))?;
    let height = u32::try_from(viewport_height)
        .map_err(|_| AppError::new("validation", "미리보기 높이가 올바르지 않습니다."))?;
    validate_import_dimensions(width, height)?;

    Ok(imageops::resize(
        &cropped,
        width,
        height,
        FilterType::Lanczos3,
    ))
}

fn crop_with_padding(source: &RgbaImage, crop: ValidatedCropRect) -> RgbaImage {
    let source_width = i64::from(source.width());
    let source_height = i64::from(source.height());
    let crop_x = crop.x;
    let crop_y = crop.y;
    let crop_width = crop.width;
    let crop_height = crop.height;
    let mut output = RgbaImage::from_pixel(crop_width, crop_height, Rgba([0, 0, 0, 0]));

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
            let pixel = source.get_pixel((src_x as u32) + x, (src_y as u32) + y);
            output.put_pixel((dst_x as u32) + x, (dst_y as u32) + y, *pixel);
        }
    }

    output
}

fn write_static_pieces(
    preview_dir: &Path,
    viewport: &RgbaImage,
    shape: &str,
    cell_width: i64,
    cell_height: i64,
    piece_count: usize,
) -> AppResult<Vec<PathBuf>> {
    let pieces = split_viewport(viewport, shape, cell_width, cell_height, piece_count)?;
    let mut piece_paths = Vec::with_capacity(piece_count);

    for (piece_index, piece) in pieces.into_iter().enumerate() {
        let piece_path = preview_dir.join(format!("piece-{piece_index:02}.png"));
        piece.save_with_format(&piece_path, ImageFormat::Png)?;
        piece_paths.push(piece_path);
    }

    Ok(piece_paths)
}

fn split_viewport(
    viewport: &RgbaImage,
    shape: &str,
    cell_width: i64,
    cell_height: i64,
    piece_count: usize,
) -> AppResult<Vec<RgbaImage>> {
    let width = u32::try_from(cell_width)
        .map_err(|_| AppError::new("validation", "미리보기 조각 너비가 올바르지 않습니다."))?;
    let height = u32::try_from(cell_height)
        .map_err(|_| AppError::new("validation", "미리보기 조각 높이가 올바르지 않습니다."))?;
    validate_import_dimensions(width, height)?;

    let pieces = match shape {
        "single" => vec![viewport.clone()],
        "horizontal_double" => vec![
            imageops::crop_imm(viewport, 0, 0, width, height).to_image(),
            imageops::crop_imm(viewport, width, 0, width, height).to_image(),
        ],
        "vertical_double" => vec![
            imageops::crop_imm(viewport, 0, 0, width, height).to_image(),
            imageops::crop_imm(viewport, 0, height, width, height).to_image(),
        ],
        _ => {
            return Err(AppError::new(
                "validation",
                "지원하지 않는 아이콘 모양입니다.",
            ));
        }
    };

    if pieces.len() != piece_count {
        return Err(AppError::new(
            "validation",
            "아이콘 조각 수가 모양 설정과 일치하지 않습니다.",
        ));
    }

    Ok(pieces)
}

fn write_gif(path: &Path, frames: Vec<Frame>, repeat: GifOutputRepeat) -> AppResult<()> {
    let file = File::create(path)?;
    let mut encoder = GifEncoder::new(file);

    match repeat {
        GifOutputRepeat::Infinite => encoder.set_repeat(ImageGifRepeat::Infinite)?,
        GifOutputRepeat::Finite(count) => encoder.set_repeat(ImageGifRepeat::Finite(count))?,
        GifOutputRepeat::Once => {}
    }

    encoder.encode_frames(frames.into_iter())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use image::{ImageBuffer, Rgba};

    use crate::imaging::motion::{static_motion_schedule, MotionRecipe};

    use super::{crop_and_resize, shared_static_source_frames, CropRect};

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
    fn crop_with_padding_keeps_requested_output_size() {
        let source = ImageBuffer::from_pixel(10, 10, Rgba([255, 0, 0, 255]));
        let output = crop_and_resize(
            &source,
            CropRect {
                x: -5.0,
                y: -5.0,
                width: 20.0,
                height: 20.0,
            },
            40,
            40,
        )
        .unwrap();

        assert_eq!(output.width(), 40);
        assert_eq!(output.height(), 40);
    }
}
