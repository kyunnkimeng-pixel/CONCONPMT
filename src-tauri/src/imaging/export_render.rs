use std::borrow::Cow;
use std::fs::{self, File};
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use image::codecs::gif::{GifDecoder, GifEncoder, Repeat as ImageGifRepeat};
use image::imageops::{self, FilterType};
use image::{AnimationDecoder, DynamicImage, Frame, ImageFormat, Rgba, RgbaImage};

use crate::error::{AppError, AppResult};
use crate::imaging::effects::{apply_effect_recipe, validate_effect_recipe, EffectRecipe};
use crate::imaging::geometry::viewport_size as validated_viewport_size;
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

#[derive(Debug, Clone, Copy)]
pub struct ExportCropRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone)]
pub struct ExportRenderPiece {
    pub piece_index: usize,
    pub file_name: String,
}

#[derive(Debug, Clone)]
pub struct ExportRenderRequest<'a> {
    pub source_path: &'a Path,
    pub source_extension: &'a str,
    pub shape: &'a str,
    pub crop: ExportCropRect,
    pub cell_width: i64,
    pub cell_height: i64,
    pub transform: ImageTransform,
    pub output_format: &'a str,
    pub resize_filter: &'a str,
    pub gif_loop_mode: &'a str,
    pub gif_loop_count: Option<i64>,
    pub source_gif_loop_mode: &'a str,
    pub source_gif_loop_count: Option<i64>,
    pub text_overlay: Option<TextOverlayRenderSpec>,
    pub effects: EffectRecipe,
    pub motion: MotionRecipe,
    pub output_dir: &'a Path,
    pub pieces: &'a [ExportRenderPiece],
}

pub fn render_icon_export(request: ExportRenderRequest<'_>) -> AppResult<Vec<PathBuf>> {
    validate_crop_rect(
        request.crop.x,
        request.crop.y,
        request.crop.width,
        request.crop.height,
    )?;
    let _ = viewport_size(request.shape, request.cell_width, request.cell_height)?;
    validate_effect_recipe(&request.effects)?;
    validate_motion_recipe(&request.motion)?;
    if request.motion.has_enabled_motion() && request.output_format != "gif" {
        return Err(AppError::new(
            "validation",
            "모션 효과가 켜진 아이콘은 GIF 형식으로만 내보낼 수 있습니다.",
        ));
    }
    fs::create_dir_all(request.output_dir)?;

    if request.source_extension == "gif" || request.motion.has_enabled_motion() {
        render_gif_export(request)
    } else {
        render_static_export(request)
    }
}

fn render_static_export(request: ExportRenderRequest<'_>) -> AppResult<Vec<PathBuf>> {
    let image = image::open(request.source_path)?.to_rgba8();
    let image = image_with_text_overlay(&image, request.text_overlay.as_ref())?;
    let (viewport_width, viewport_height) =
        viewport_size(request.shape, request.cell_width, request.cell_height)?;
    let source_geometry = source_viewport_geometry(
        request.shape,
        request.cell_width,
        request.cell_height,
        request.transform,
    )?;
    let viewport = crop_and_resize(
        image.as_ref(),
        request.crop,
        source_geometry.viewport.width,
        source_geometry.viewport.height,
        request.resize_filter,
    )?;
    let viewport = apply_image_transform(viewport, request.transform)?;
    let mut viewport = viewport;
    apply_effect_recipe(&mut viewport, &request.effects)?;
    validate_transformed_viewport(&viewport, viewport_width, viewport_height)?;
    let pieces = split_viewport(
        &viewport,
        request.shape,
        request.cell_width,
        request.cell_height,
    )?;

    write_static_pieces(
        request.output_dir,
        request.output_format,
        request.pieces,
        pieces,
    )
}

fn render_gif_export(request: ExportRenderRequest<'_>) -> AppResult<Vec<PathBuf>> {
    if can_copy_original_gif_without_reencode(&request)? {
        let mut paths = Vec::with_capacity(request.pieces.len());
        for render_piece in request.pieces {
            let output_path = request.output_dir.join(&render_piece.file_name);
            fs::copy(request.source_path, &output_path)?;
            paths.push(output_path);
        }
        return Ok(paths);
    }

    let repeat = output_repeat_for_settings(
        request.gif_loop_mode,
        request.gif_loop_count,
        request.source_gif_loop_mode,
        request.source_gif_loop_count,
    )?;
    let frames = load_source_motion_frames(&request)?;
    let is_pingpong = is_pingpong_loop_mode(request.gif_loop_mode);
    let output_frame_count = if is_pingpong {
        pingpong_sequence_len(frames.len())
    } else {
        frames.len()
    };

    let (viewport_width, viewport_height) =
        viewport_size(request.shape, request.cell_width, request.cell_height)?;
    let source_geometry = source_viewport_geometry(
        request.shape,
        request.cell_width,
        request.cell_height,
        request.transform,
    )?;
    let output_width = u32::try_from(viewport_width)
        .map_err(|_| AppError::new("validation", "출력 너비가 올바르지 않습니다."))?;
    let output_height = u32::try_from(viewport_height)
        .map_err(|_| AppError::new("validation", "출력 높이가 올바르지 않습니다."))?;
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
    let mut elapsed_ms = 0_u64;
    let final_frame_index = frames.len().saturating_sub(1);
    let mut piece_frames: Vec<Vec<Frame>> = request
        .pieces
        .iter()
        .map(|_| Vec::with_capacity(output_frame_count))
        .collect();
    for (frame_index, frame) in frames.into_iter().enumerate() {
        let delay = frame.delay();
        let frame_duration_ms = frame.duration_ms;
        let source_frame =
            image_with_text_overlay(frame.image.as_ref(), request.text_overlay.as_ref())?;
        let viewport = crop_and_resize(
            source_frame.as_ref(),
            request.crop,
            source_geometry.viewport.width,
            source_geometry.viewport.height,
            request.resize_filter,
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
        let viewport = motion_result.image;
        validate_transformed_viewport(&viewport, viewport_width, viewport_height)?;
        let split_pieces = split_viewport(
            &viewport,
            request.shape,
            request.cell_width,
            request.cell_height,
        )?;

        for (target_index, render_piece) in request.pieces.iter().enumerate() {
            let piece = split_pieces
                .get(render_piece.piece_index)
                .ok_or_else(|| AppError::new("validation", "내보낼 조각 순서가 잘못되었습니다."))?;
            piece_frames[target_index].push(Frame::from_parts(piece.clone(), 0, 0, delay));
        }
        elapsed_ms = elapsed_ms.saturating_add(frame_duration_ms);
    }

    if is_pingpong {
        for frames in &mut piece_frames {
            pingpong_sequence(frames);
        }
    }

    let mut paths = Vec::with_capacity(request.pieces.len());
    for (target_index, render_piece) in request.pieces.iter().enumerate() {
        let output_path = request.output_dir.join(&render_piece.file_name);
        write_gif(
            &output_path,
            std::mem::take(&mut piece_frames[target_index]),
            repeat,
        )?;
        paths.push(output_path);
    }

    Ok(paths)
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
    request: &ExportRenderRequest<'_>,
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

fn can_copy_original_gif_without_reencode(request: &ExportRenderRequest<'_>) -> AppResult<bool> {
    if request.source_extension != "gif"
        || request.output_format != "gif"
        || request.shape != "single"
        || request.gif_loop_mode != "preserve"
        || request.text_overlay.is_some()
        || request.effects.has_enabled_effects()
        || request.motion.has_enabled_motion()
        || !request.transform.is_identity()
        || request.pieces.len() != 1
        || request.pieces[0].piece_index != 0
    {
        return Ok(false);
    }

    let (source_width, source_height) = image::image_dimensions(request.source_path)?;
    let crop_x = request.crop.x.round() as i64;
    let crop_y = request.crop.y.round() as i64;
    let crop_width = request.crop.width.round() as i64;
    let crop_height = request.crop.height.round() as i64;

    Ok(crop_x == 0
        && crop_y == 0
        && crop_width == i64::from(source_width)
        && crop_height == i64::from(source_height)
        && request.cell_width == i64::from(source_width)
        && request.cell_height == i64::from(source_height))
}

fn crop_and_resize(
    image: &RgbaImage,
    crop: ExportCropRect,
    viewport_width: i64,
    viewport_height: i64,
    resize_filter: &str,
) -> AppResult<RgbaImage> {
    let crop = validate_crop_rect(crop.x, crop.y, crop.width, crop.height)?;
    let cropped = crop_with_padding(image, crop);
    let width = u32::try_from(viewport_width)
        .map_err(|_| AppError::new("validation", "출력 너비가 올바르지 않습니다."))?;
    let height = u32::try_from(viewport_height)
        .map_err(|_| AppError::new("validation", "출력 높이가 올바르지 않습니다."))?;
    validate_import_dimensions(width, height)?;

    Ok(imageops::resize(
        &cropped,
        width,
        height,
        resize_filter_type(resize_filter),
    ))
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
        "회전 후 출력 크기가 아이콘 모양과 일치하지 않습니다.",
    ))
}

fn resize_filter_type(value: &str) -> FilterType {
    match value.trim().to_ascii_lowercase().as_str() {
        "nearest" => FilterType::Nearest,
        "triangle" | "bilinear" => FilterType::Triangle,
        "catmull_rom" | "bicubic" => FilterType::CatmullRom,
        "gaussian" => FilterType::Gaussian,
        "lanczos" | "lanczos3" => FilterType::Lanczos3,
        _ => FilterType::Lanczos3,
    }
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

fn split_viewport(
    viewport: &RgbaImage,
    shape: &str,
    cell_width: i64,
    cell_height: i64,
) -> AppResult<Vec<RgbaImage>> {
    let width = u32::try_from(cell_width)
        .map_err(|_| AppError::new("validation", "출력 조각 너비가 올바르지 않습니다."))?;
    let height = u32::try_from(cell_height)
        .map_err(|_| AppError::new("validation", "출력 조각 높이가 올바르지 않습니다."))?;
    validate_import_dimensions(width, height)?;

    match shape {
        "single" => Ok(vec![viewport.clone()]),
        "horizontal_double" => Ok(vec![
            imageops::crop_imm(viewport, 0, 0, width, height).to_image(),
            imageops::crop_imm(viewport, width, 0, width, height).to_image(),
        ]),
        "vertical_double" => Ok(vec![
            imageops::crop_imm(viewport, 0, 0, width, height).to_image(),
            imageops::crop_imm(viewport, 0, height, width, height).to_image(),
        ]),
        _ => Err(AppError::new(
            "validation",
            "지원하지 않는 아이콘 모양입니다.",
        )),
    }
}

fn viewport_size(shape: &str, cell_width: i64, cell_height: i64) -> AppResult<(i64, i64)> {
    let viewport = validated_viewport_size(shape, cell_width, cell_height)?;
    Ok((viewport.width, viewport.height))
}

fn write_static_pieces(
    output_dir: &Path,
    output_format: &str,
    render_pieces: &[ExportRenderPiece],
    pieces: Vec<RgbaImage>,
) -> AppResult<Vec<PathBuf>> {
    let mut paths = Vec::with_capacity(render_pieces.len());

    for render_piece in render_pieces {
        let piece = pieces
            .get(render_piece.piece_index)
            .ok_or_else(|| AppError::new("validation", "내보낼 조각 순서가 잘못되었습니다."))?;
        let output_path = output_dir.join(&render_piece.file_name);
        save_static_piece(piece, &output_path, output_format)?;
        paths.push(output_path);
    }

    Ok(paths)
}

fn save_static_piece(piece: &RgbaImage, output_path: &Path, output_format: &str) -> AppResult<()> {
    match output_format {
        "jpg" => {
            let rgb = DynamicImage::ImageRgba8(piece.clone()).to_rgb8();
            DynamicImage::ImageRgb8(rgb).save_with_format(output_path, ImageFormat::Jpeg)?;
        }
        "gif" => {
            DynamicImage::ImageRgba8(piece.clone())
                .save_with_format(output_path, ImageFormat::Gif)?;
        }
        _ => {
            piece.save_with_format(output_path, ImageFormat::Png)?;
        }
    }

    Ok(())
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
    use std::fs::File;
    use std::io::BufReader;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    use image::codecs::gif::GifDecoder;
    use image::{AnimationDecoder, RgbaImage};
    use image::{DynamicImage, ImageBuffer, Rgba};

    use crate::imaging::effects::{EffectRecipe, EffectStep, EFFECT_RECIPE_VERSION};
    use crate::imaging::gif_pipeline::inspect_gif_bytes;
    use crate::imaging::motion::{
        apply_motion_recipe, static_motion_schedule, MotionEdgeMode, MotionFrameContext,
        MotionInterpolation,
    };
    use crate::imaging::motion::{DisplacementMotion, MotionAxis, MotionRecipe};
    use crate::imaging::preview::{
        generate_icon_preview_in_directory, CropRect, GeneratePreviewRequest,
    };
    use crate::imaging::transform::ImageTransform;

    use super::{
        crop_and_resize, render_icon_export, shared_static_source_frames, ExportCropRect,
        ExportRenderPiece, ExportRenderRequest,
    };

    fn temp_dir(label: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("pmtcon-effects-{label}-{suffix}"));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn recipe(effects: Vec<EffectStep>) -> EffectRecipe {
        EffectRecipe {
            version: EFFECT_RECIPE_VERSION,
            effects,
        }
    }

    fn decode_gif_frames(path: &PathBuf) -> Vec<RgbaImage> {
        decode_gif_timeline(path)
            .into_iter()
            .map(|(image, _)| image)
            .collect()
    }

    fn decode_gif_timeline(path: &PathBuf) -> Vec<(RgbaImage, u32)> {
        let decoder = GifDecoder::new(BufReader::new(File::open(path).unwrap())).unwrap();
        decoder
            .into_frames()
            .collect_frames()
            .unwrap()
            .into_iter()
            .map(|frame| {
                let delay = frame.delay();
                let (numerator, denominator) = delay.numer_denom_ms();
                (frame.into_buffer(), numerator / denominator.max(1))
            })
            .collect()
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
    fn crop_with_padding_keeps_export_output_size() {
        let source = ImageBuffer::from_pixel(10, 10, Rgba([255, 0, 0, 255]));
        let output = crop_and_resize(
            &source,
            ExportCropRect {
                x: -5.0,
                y: -5.0,
                width: 20.0,
                height: 20.0,
            },
            40,
            40,
            "lanczos3",
        )
        .unwrap();

        assert_eq!(output.width(), 40);
        assert_eq!(output.height(), 40);
    }

    #[test]
    fn native_effect_preview_matches_default_png_export_pixels() {
        let root = temp_dir("preview-export");
        let source_path = root.join("source.png");
        let source = ImageBuffer::from_fn(9, 7, |x, y| {
            Rgba([
                (x * 23) as u8,
                (y * 31) as u8,
                ((x + y) * 13) as u8,
                if x == 0 || y == 0 { 90 } else { 255 },
            ])
        });
        DynamicImage::ImageRgba8(source).save(&source_path).unwrap();
        let effects = recipe(vec![
            EffectStep::Pixelate {
                id: "pixel".to_string(),
                enabled: true,
                block_size: 2,
            },
            EffectStep::ColorAdjust {
                id: "color".to_string(),
                enabled: true,
                brightness: 10,
                contrast: -5,
                saturation: 20,
                hue: 30,
            },
        ]);
        let transform = ImageTransform::new(0, false, false).unwrap();
        let preview = generate_icon_preview_in_directory(
            &root.join("preview"),
            GeneratePreviewRequest {
                collection_id: "collection",
                icon_id: "icon",
                source_path: &source_path,
                source_extension: "png",
                shape: "single",
                crop: CropRect {
                    x: 0.0,
                    y: 0.0,
                    width: 9.0,
                    height: 7.0,
                },
                cell_width: 6,
                cell_height: 5,
                transform,
                gif_loop_mode: "preserve",
                gif_loop_count: None,
                source_gif_loop_mode: None,
                source_gif_loop_count: None,
                text_overlay: None,
                effects: effects.clone(),
                motion: MotionRecipe::default(),
            },
        )
        .unwrap();
        let pieces = [ExportRenderPiece {
            piece_index: 0,
            file_name: "001.png".to_string(),
        }];
        let output_paths = render_icon_export(ExportRenderRequest {
            source_path: &source_path,
            source_extension: "png",
            shape: "single",
            crop: ExportCropRect {
                x: 0.0,
                y: 0.0,
                width: 9.0,
                height: 7.0,
            },
            cell_width: 6,
            cell_height: 5,
            transform,
            output_format: "png",
            resize_filter: "lanczos3",
            gif_loop_mode: "preserve",
            gif_loop_count: None,
            source_gif_loop_mode: "preserve",
            source_gif_loop_count: None,
            text_overlay: None,
            effects,
            motion: MotionRecipe::default(),
            output_dir: &root.join("export"),
            pieces: &pieces,
        })
        .unwrap();

        assert_eq!(
            image::open(preview.current_preview_path)
                .unwrap()
                .to_rgba8(),
            image::open(&output_paths[0]).unwrap().to_rgba8()
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn multi_piece_effect_is_rendered_before_the_viewport_is_split() {
        let root = temp_dir("multi-piece");
        let source_path = root.join("source.png");
        let mut source = ImageBuffer::from_pixel(4, 3, Rgba([0, 0, 0, 0]));
        source.put_pixel(1, 1, Rgba([255, 0, 0, 255]));
        DynamicImage::ImageRgba8(source).save(&source_path).unwrap();
        let pieces = [
            ExportRenderPiece {
                piece_index: 0,
                file_name: "left.png".to_string(),
            },
            ExportRenderPiece {
                piece_index: 1,
                file_name: "right.png".to_string(),
            },
        ];
        let output_paths = render_icon_export(ExportRenderRequest {
            source_path: &source_path,
            source_extension: "png",
            shape: "horizontal_double",
            crop: ExportCropRect {
                x: 0.0,
                y: 0.0,
                width: 4.0,
                height: 3.0,
            },
            cell_width: 2,
            cell_height: 3,
            transform: ImageTransform::new(0, false, false).unwrap(),
            output_format: "png",
            resize_filter: "lanczos3",
            gif_loop_mode: "preserve",
            gif_loop_count: None,
            source_gif_loop_mode: "preserve",
            source_gif_loop_count: None,
            text_overlay: None,
            effects: recipe(vec![EffectStep::Blur {
                id: "blur".to_string(),
                enabled: true,
                radius: 1,
            }]),
            motion: MotionRecipe::default(),
            output_dir: &root.join("export"),
            pieces: &pieces,
        })
        .unwrap();

        let right = image::open(&output_paths[1]).unwrap().to_rgba8();
        assert!(
            right.get_pixel(0, 1)[3] > 0,
            "결합 viewport 경계를 가로지르는 blur가 오른쪽 조각에도 남아야 합니다."
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    fn assert_combined_motion_split(shape: &str, axis: MotionAxis, label: &str) {
        let cell_width = 3_u32;
        let cell_height = 2_u32;
        let (viewport_width, viewport_height) = match shape {
            "horizontal_double" => (cell_width * 2, cell_height),
            "vertical_double" => (cell_width, cell_height * 2),
            _ => panic!("unsupported fixture shape"),
        };
        let root = temp_dir(label);
        let source_path = root.join("source.png");
        let source = ImageBuffer::from_fn(viewport_width, viewport_height, |x, y| {
            Rgba([
                (x * 31 + y * 7) as u8,
                (y * 53 + x * 5) as u8,
                (x * 11 + y * 17) as u8,
                255,
            ])
        });
        DynamicImage::ImageRgba8(source.clone())
            .save(&source_path)
            .unwrap();
        let motion = MotionRecipe {
            duration_ms: 200,
            fps: 10,
            interpolation: MotionInterpolation::Nearest,
            edge_mode: MotionEdgeMode::Transparent,
            displacement: Some(DisplacementMotion::Wave {
                enabled: true,
                cycles_per_loop: 1,
                axis,
                amplitude_px: 1,
                wavelength_px: 4,
            }),
            ..MotionRecipe::default()
        };
        let pieces = [
            ExportRenderPiece {
                piece_index: 0,
                file_name: "first.gif".to_string(),
            },
            ExportRenderPiece {
                piece_index: 1,
                file_name: "second.gif".to_string(),
            },
        ];
        let output_dir = root.join("export");
        let output_paths = render_icon_export(ExportRenderRequest {
            source_path: &source_path,
            source_extension: "png",
            shape,
            crop: ExportCropRect {
                x: 0.0,
                y: 0.0,
                width: f64::from(viewport_width),
                height: f64::from(viewport_height),
            },
            cell_width: i64::from(cell_width),
            cell_height: i64::from(cell_height),
            transform: ImageTransform::new(0, false, false).unwrap(),
            output_format: "gif",
            resize_filter: "lanczos3",
            gif_loop_mode: "infinite",
            gif_loop_count: None,
            source_gif_loop_mode: "preserve",
            source_gif_loop_count: None,
            text_overlay: None,
            effects: EffectRecipe::default(),
            motion: motion.clone(),
            output_dir: &output_dir,
            pieces: &pieces,
        })
        .unwrap();

        let first_frames = decode_gif_frames(&output_paths[0]);
        let second_frames = decode_gif_frames(&output_paths[1]);
        assert_eq!(first_frames.len(), 2);
        assert_eq!(second_frames.len(), 2);
        assert_eq!(first_frames[1].dimensions(), (cell_width, cell_height));
        assert_eq!(second_frames[1].dimensions(), (cell_width, cell_height));

        let offsets = match shape {
            "horizontal_double" => [(0_i64, 0_i64), (i64::from(cell_width), 0)],
            "vertical_double" => [(0_i64, 0_i64), (0, i64::from(cell_height))],
            _ => unreachable!(),
        };
        let mut joined = RgbaImage::new(viewport_width, viewport_height);
        image::imageops::replace(&mut joined, &first_frames[1], offsets[0].0, offsets[0].1);
        image::imageops::replace(&mut joined, &second_frames[1], offsets[1].0, offsets[1].1);

        let base = crop_and_resize(
            &source,
            ExportCropRect {
                x: 0.0,
                y: 0.0,
                width: f64::from(viewport_width),
                height: f64::from(viewport_height),
            },
            i64::from(viewport_width),
            i64::from(viewport_height),
            "lanczos3",
        )
        .unwrap();
        let context = MotionFrameContext {
            elapsed_ms: 100,
            total_duration_ms: 200,
        };
        let expected = apply_motion_recipe(&base, &motion, context).unwrap().image;

        let mut split_before_motion = RgbaImage::new(viewport_width, viewport_height);
        for (piece_index, (offset_x, offset_y)) in offsets.into_iter().enumerate() {
            let piece = image::imageops::crop_imm(
                &base,
                u32::try_from(offset_x).unwrap(),
                u32::try_from(offset_y).unwrap(),
                cell_width,
                cell_height,
            )
            .to_image();
            let moved = apply_motion_recipe(&piece, &motion, context).unwrap().image;
            image::imageops::replace(&mut split_before_motion, &moved, offset_x, offset_y);
            assert_eq!(pieces[piece_index].piece_index, piece_index);
        }

        let seam_probe = match shape {
            "horizontal_double" => (cell_width - 1, 1),
            "vertical_double" => (1, cell_height - 1),
            _ => unreachable!(),
        };
        assert_ne!(
            expected.get_pixel(seam_probe.0, seam_probe.1),
            split_before_motion.get_pixel(seam_probe.0, seam_probe.1),
            "fixture must detect a split-before-motion seam"
        );
        assert_eq!(
            joined, expected,
            "exported pieces must rejoin to the combined-viewport motion result"
        );

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn motion_is_applied_to_non_square_combined_viewport_before_double_split() {
        assert_combined_motion_split(
            "horizontal_double",
            MotionAxis::Horizontal,
            "motion-horizontal-double",
        );
        assert_combined_motion_split(
            "vertical_double",
            MotionAxis::Vertical,
            "motion-vertical-double",
        );
    }

    #[test]
    fn static_motion_preview_and_final_export_are_the_same_measured_gif() {
        let root = temp_dir("static-motion-parity");
        let source_path = root.join("source.png");
        let source = ImageBuffer::from_fn(8, 8, |x, y| {
            Rgba([
                (x * 29) as u8,
                (y * 23) as u8,
                ((x + y) * 13) as u8,
                if x == 0 || y == 0 { 128 } else { 255 },
            ])
        });
        DynamicImage::ImageRgba8(source).save(&source_path).unwrap();
        let motion = MotionRecipe {
            duration_ms: 200,
            fps: 10,
            seed: 77,
            displacement: Some(DisplacementMotion::Wave {
                enabled: true,
                cycles_per_loop: 1,
                axis: MotionAxis::Horizontal,
                amplitude_px: 1,
                wavelength_px: 4,
            }),
            ..MotionRecipe::default()
        };
        let transform = ImageTransform::new(0, false, false).unwrap();
        let preview = generate_icon_preview_in_directory(
            &root.join("preview"),
            GeneratePreviewRequest {
                collection_id: "collection",
                icon_id: "motion-icon",
                source_path: &source_path,
                source_extension: "png",
                shape: "single",
                crop: CropRect {
                    x: 0.0,
                    y: 0.0,
                    width: 8.0,
                    height: 8.0,
                },
                cell_width: 8,
                cell_height: 8,
                transform,
                gif_loop_mode: "infinite",
                gif_loop_count: None,
                source_gif_loop_mode: None,
                source_gif_loop_count: None,
                text_overlay: None,
                effects: EffectRecipe::default(),
                motion: motion.clone(),
            },
        )
        .unwrap();
        let output_paths = render_icon_export(ExportRenderRequest {
            source_path: &source_path,
            source_extension: "png",
            shape: "single",
            crop: ExportCropRect {
                x: 0.0,
                y: 0.0,
                width: 8.0,
                height: 8.0,
            },
            cell_width: 8,
            cell_height: 8,
            transform,
            output_format: "gif",
            resize_filter: "lanczos3",
            gif_loop_mode: "infinite",
            gif_loop_count: None,
            source_gif_loop_mode: "preserve",
            source_gif_loop_count: None,
            text_overlay: None,
            effects: EffectRecipe::default(),
            motion: motion.clone(),
            output_dir: &root.join("export"),
            pieces: &[ExportRenderPiece {
                piece_index: 0,
                file_name: "001.gif".to_string(),
            }],
        })
        .unwrap();
        let repeated_output_paths = render_icon_export(ExportRenderRequest {
            source_path: &source_path,
            source_extension: "png",
            shape: "single",
            crop: ExportCropRect {
                x: 0.0,
                y: 0.0,
                width: 8.0,
                height: 8.0,
            },
            cell_width: 8,
            cell_height: 8,
            transform,
            output_format: "gif",
            resize_filter: "lanczos3",
            gif_loop_mode: "infinite",
            gif_loop_count: None,
            source_gif_loop_mode: "preserve",
            source_gif_loop_count: None,
            text_overlay: None,
            effects: EffectRecipe::default(),
            motion,
            output_dir: &root.join("export-repeat"),
            pieces: &[ExportRenderPiece {
                piece_index: 0,
                file_name: "001.gif".to_string(),
            }],
        })
        .unwrap();

        assert_eq!(preview.frame_count, 2);
        assert_eq!(preview.duration_ms, 200);
        assert!(preview.poster_path.ends_with("poster.png"));
        assert_eq!(
            preview.encoded_byte_size,
            std::fs::metadata(&preview.current_preview_path)
                .unwrap()
                .len()
        );
        assert_eq!(
            std::fs::read(&preview.current_preview_path).unwrap(),
            std::fs::read(&output_paths[0]).unwrap()
        );
        assert_eq!(
            std::fs::read(&output_paths[0]).unwrap(),
            std::fs::read(&repeated_output_paths[0]).unwrap()
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn pingpong_reflects_final_motion_timeline_in_preview_and_export() {
        let root = temp_dir("motion-pingpong-final-timeline");
        let source_path = root.join("source.png");
        let source = ImageBuffer::from_fn(8, 8, |x, y| {
            Rgba([
                (x * 29) as u8,
                (y * 23) as u8,
                ((x + y) * 13) as u8,
                if x == 0 || y == 0 { 128 } else { 255 },
            ])
        });
        DynamicImage::ImageRgba8(source).save(&source_path).unwrap();
        let motion = MotionRecipe {
            duration_ms: 400,
            fps: 10,
            seed: 77,
            displacement: Some(DisplacementMotion::Wave {
                enabled: true,
                cycles_per_loop: 1,
                axis: MotionAxis::Horizontal,
                amplitude_px: 2,
                wavelength_px: 4,
            }),
            ..MotionRecipe::default()
        };
        let transform = ImageTransform::new(0, false, false).unwrap();
        let normal_preview = generate_icon_preview_in_directory(
            &root.join("preview-normal"),
            GeneratePreviewRequest {
                collection_id: "collection",
                icon_id: "motion-icon",
                source_path: &source_path,
                source_extension: "png",
                shape: "single",
                crop: CropRect {
                    x: 0.0,
                    y: 0.0,
                    width: 8.0,
                    height: 8.0,
                },
                cell_width: 8,
                cell_height: 8,
                transform,
                gif_loop_mode: "infinite",
                gif_loop_count: None,
                source_gif_loop_mode: None,
                source_gif_loop_count: None,
                text_overlay: None,
                effects: EffectRecipe::default(),
                motion: motion.clone(),
            },
        )
        .unwrap();
        let pingpong_preview = generate_icon_preview_in_directory(
            &root.join("preview-pingpong"),
            GeneratePreviewRequest {
                collection_id: "collection",
                icon_id: "motion-icon",
                source_path: &source_path,
                source_extension: "png",
                shape: "single",
                crop: CropRect {
                    x: 0.0,
                    y: 0.0,
                    width: 8.0,
                    height: 8.0,
                },
                cell_width: 8,
                cell_height: 8,
                transform,
                gif_loop_mode: "pingpong",
                gif_loop_count: None,
                source_gif_loop_mode: None,
                source_gif_loop_count: None,
                text_overlay: None,
                effects: EffectRecipe::default(),
                motion: motion.clone(),
            },
        )
        .unwrap();
        let output_paths = render_icon_export(ExportRenderRequest {
            source_path: &source_path,
            source_extension: "png",
            shape: "single",
            crop: ExportCropRect {
                x: 0.0,
                y: 0.0,
                width: 8.0,
                height: 8.0,
            },
            cell_width: 8,
            cell_height: 8,
            transform,
            output_format: "gif",
            resize_filter: "lanczos3",
            gif_loop_mode: "pingpong",
            gif_loop_count: None,
            source_gif_loop_mode: "preserve",
            source_gif_loop_count: None,
            text_overlay: None,
            effects: EffectRecipe::default(),
            motion,
            output_dir: &root.join("export-pingpong"),
            pieces: &[ExportRenderPiece {
                piece_index: 0,
                file_name: "001.gif".to_string(),
            }],
        })
        .unwrap();

        let normal = decode_gif_timeline(&normal_preview.current_preview_path);
        let actual = decode_gif_timeline(&pingpong_preview.current_preview_path);
        let exported = decode_gif_timeline(&output_paths[0]);
        assert_eq!(normal.len(), 4);
        assert_eq!(actual.len(), 6);
        assert_eq!(actual, exported);
        assert_eq!(&actual[..4], normal.as_slice());
        assert_eq!(actual[4], normal[2]);
        assert_eq!(actual[5], normal[1]);
        assert_eq!(pingpong_preview.frame_count, 6);
        assert_eq!(pingpong_preview.duration_ms, 600);
        let inspection =
            inspect_gif_bytes(&std::fs::read(&pingpong_preview.current_preview_path).unwrap())
                .unwrap();
        assert_eq!(inspection.frame_count, 6);
        assert_eq!(inspection.loop_mode, "infinite");
        assert_eq!(
            std::fs::read(&pingpong_preview.current_preview_path).unwrap(),
            std::fs::read(&output_paths[0]).unwrap()
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
