use std::fs::{self, File};
use std::io::BufReader;
use std::path::{Path, PathBuf};

use image::codecs::gif::{GifDecoder, GifEncoder, Repeat as ImageGifRepeat};
use image::imageops::{self, FilterType};
use image::{AnimationDecoder, DynamicImage, Frame, ImageFormat, Rgba, RgbaImage};

use crate::error::{AppError, AppResult};
use crate::imaging::gif_pipeline::{
    is_pingpong_loop_mode, output_repeat_for_settings, pingpong_sequence, GifOutputRepeat,
};
use crate::imaging::text_overlay::{apply_text_overlay, TextOverlayRenderSpec};

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
    pub output_format: &'a str,
    pub gif_loop_mode: &'a str,
    pub gif_loop_count: Option<i64>,
    pub source_gif_loop_mode: &'a str,
    pub source_gif_loop_count: Option<i64>,
    pub text_overlay: Option<TextOverlayRenderSpec>,
    pub output_dir: &'a Path,
    pub pieces: &'a [ExportRenderPiece],
}

pub fn render_icon_export(request: ExportRenderRequest<'_>) -> AppResult<Vec<PathBuf>> {
    fs::create_dir_all(request.output_dir)?;

    if request.source_extension == "gif" {
        render_gif_export(request)
    } else {
        render_static_export(request)
    }
}

fn render_static_export(request: ExportRenderRequest<'_>) -> AppResult<Vec<PathBuf>> {
    let image = image::open(request.source_path)?;
    let (viewport_width, viewport_height) =
        viewport_size(request.shape, request.cell_width, request.cell_height)?;
    let viewport = crop_and_resize(&image, request.crop, viewport_width, viewport_height)?;
    let mut viewport = viewport;
    apply_text_overlay(&mut viewport, request.text_overlay.as_ref())?;
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

    let file = File::open(request.source_path)?;
    let decoder = GifDecoder::new(BufReader::new(file))?;
    let frames = decoder.into_frames().collect_frames()?;
    if frames.is_empty() {
        return Err(AppError::new("gif", "GIF 프레임을 찾을 수 없습니다."));
    }

    let (viewport_width, viewport_height) =
        viewport_size(request.shape, request.cell_width, request.cell_height)?;
    let mut piece_frames: Vec<Vec<Frame>> = request
        .pieces
        .iter()
        .map(|_| Vec::with_capacity(frames.len()))
        .collect();
    let repeat = output_repeat_for_settings(
        request.gif_loop_mode,
        request.gif_loop_count,
        request.source_gif_loop_mode,
        request.source_gif_loop_count,
    )?;

    for frame in frames {
        let delay = frame.delay();
        let source_frame = DynamicImage::ImageRgba8(frame.into_buffer());
        let viewport =
            crop_and_resize(&source_frame, request.crop, viewport_width, viewport_height)?;
        let mut viewport = viewport;
        apply_text_overlay(&mut viewport, request.text_overlay.as_ref())?;
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
    }

    let mut paths = Vec::with_capacity(request.pieces.len());
    for (target_index, render_piece) in request.pieces.iter().enumerate() {
        let output_path = request.output_dir.join(&render_piece.file_name);
        if is_pingpong_loop_mode(request.gif_loop_mode) {
            pingpong_sequence(&mut piece_frames[target_index]);
        }
        write_gif(
            &output_path,
            std::mem::take(&mut piece_frames[target_index]),
            repeat,
        )?;
        paths.push(output_path);
    }

    Ok(paths)
}

fn can_copy_original_gif_without_reencode(request: &ExportRenderRequest<'_>) -> AppResult<bool> {
    if request.source_extension != "gif"
        || request.output_format != "gif"
        || request.shape != "single"
        || request.gif_loop_mode != "preserve"
        || request.text_overlay.is_some()
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
    image: &DynamicImage,
    crop: ExportCropRect,
    viewport_width: i64,
    viewport_height: i64,
) -> AppResult<RgbaImage> {
    if crop.width <= 0.0 || crop.height <= 0.0 {
        return Err(AppError::new(
            "validation",
            "크롭 영역은 1px 이상이어야 합니다.",
        ));
    }

    let cropped = crop_with_padding(image, crop);
    let width = u32::try_from(viewport_width.max(1)).unwrap_or(u32::MAX);
    let height = u32::try_from(viewport_height.max(1)).unwrap_or(u32::MAX);

    Ok(imageops::resize(
        &cropped,
        width,
        height,
        FilterType::Lanczos3,
    ))
}

fn crop_with_padding(image: &DynamicImage, crop: ExportCropRect) -> RgbaImage {
    let source = image.to_rgba8();
    let source_width = i64::from(source.width());
    let source_height = i64::from(source.height());
    let crop_x = crop.x.round() as i64;
    let crop_y = crop.y.round() as i64;
    let crop_width = crop.width.round().max(1.0) as u32;
    let crop_height = crop.height.round().max(1.0) as u32;
    let mut output = RgbaImage::from_pixel(crop_width, crop_height, Rgba([0, 0, 0, 0]));

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
    let width = u32::try_from(cell_width.max(1)).unwrap_or(u32::MAX);
    let height = u32::try_from(cell_height.max(1)).unwrap_or(u32::MAX);

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
    match shape {
        "single" => Ok((cell_width, cell_height)),
        "horizontal_double" => Ok((cell_width * 2, cell_height)),
        "vertical_double" => Ok((cell_width, cell_height * 2)),
        _ => Err(AppError::new(
            "validation",
            "지원하지 않는 아이콘 모양입니다.",
        )),
    }
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
    use image::{DynamicImage, ImageBuffer, Rgba};

    use super::{crop_and_resize, ExportCropRect};

    #[test]
    fn crop_with_padding_keeps_export_output_size() {
        let source = ImageBuffer::from_pixel(10, 10, Rgba([255, 0, 0, 255]));
        let image = DynamicImage::ImageRgba8(source);
        let output = crop_and_resize(
            &image,
            ExportCropRect {
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
