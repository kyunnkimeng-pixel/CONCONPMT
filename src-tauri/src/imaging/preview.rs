use std::fs::{self, File};
use std::io::BufReader;
use std::path::{Path, PathBuf};

use image::codecs::gif::{GifDecoder, GifEncoder, Repeat as ImageGifRepeat};
use image::imageops::{self, FilterType};
use image::{AnimationDecoder, DynamicImage, Frame, ImageFormat, Rgba, RgbaImage};

use crate::error::{AppError, AppResult};
use crate::imaging::geometry::{piece_roles, viewport_size};
use crate::imaging::gif_pipeline::{output_repeat_for_settings, GifOutputRepeat};
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
    pub gif_loop_mode: &'a str,
    pub gif_loop_count: Option<i64>,
    pub source_gif_loop_mode: Option<&'a str>,
    pub source_gif_loop_count: Option<i64>,
}

#[derive(Debug)]
pub struct GeneratedPreview {
    pub current_preview_path: PathBuf,
    pub piece_paths: Vec<PathBuf>,
}

pub fn generate_icon_preview(
    paths: &AppPaths,
    request: GeneratePreviewRequest<'_>,
) -> AppResult<GeneratedPreview> {
    let viewport = viewport_size(request.shape, request.cell_width, request.cell_height)?;
    let roles = piece_roles(request.shape)?;
    let preview_dir = paths
        .collection_previews_dir
        .join(request.collection_id)
        .join(request.icon_id);
    fs::create_dir_all(&preview_dir)?;

    if request.source_extension == "gif" {
        generate_gif_preview(
            &preview_dir,
            &request,
            viewport.width,
            viewport.height,
            roles.len(),
        )
    } else {
        generate_static_preview(
            &preview_dir,
            &request,
            viewport.width,
            viewport.height,
            roles.len(),
        )
    }
}

fn generate_static_preview(
    preview_dir: &Path,
    request: &GeneratePreviewRequest<'_>,
    viewport_width: i64,
    viewport_height: i64,
    piece_count: usize,
) -> AppResult<GeneratedPreview> {
    let image = image::open(request.source_path)?;
    let viewport = crop_and_resize(&image, request.crop, viewport_width, viewport_height)?;
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

    Ok(GeneratedPreview {
        current_preview_path,
        piece_paths,
    })
}

fn generate_gif_preview(
    preview_dir: &Path,
    request: &GeneratePreviewRequest<'_>,
    viewport_width: i64,
    viewport_height: i64,
    piece_count: usize,
) -> AppResult<GeneratedPreview> {
    let file = File::open(request.source_path)?;
    let decoder = GifDecoder::new(BufReader::new(file))?;
    let frames = decoder.into_frames().collect_frames()?;
    if frames.is_empty() {
        return Err(AppError::new("gif", "GIF 프레임을 찾을 수 없습니다."));
    }

    let mut viewport_frames = Vec::with_capacity(frames.len());
    let mut piece_frames: Vec<Vec<Frame>> = (0..piece_count)
        .map(|_| Vec::with_capacity(frames.len()))
        .collect();
    let repeat = output_repeat_for_settings(
        request.gif_loop_mode,
        request.gif_loop_count,
        request.source_gif_loop_mode.unwrap_or("preserve"),
        request.source_gif_loop_count,
    )?;

    for frame in frames {
        let delay = frame.delay();
        let source_frame = DynamicImage::ImageRgba8(frame.into_buffer());
        let viewport =
            crop_and_resize(&source_frame, request.crop, viewport_width, viewport_height)?;
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
    }

    let current_preview_path = preview_dir.join("preview.gif");
    write_gif(&current_preview_path, viewport_frames, repeat)?;

    let mut piece_paths = Vec::with_capacity(piece_count);
    for (piece_index, frames) in piece_frames.into_iter().enumerate() {
        let piece_path = preview_dir.join(format!("piece-{piece_index:02}.gif"));
        write_gif(&piece_path, frames, repeat)?;
        piece_paths.push(piece_path);
    }

    Ok(GeneratedPreview {
        current_preview_path,
        piece_paths,
    })
}

fn crop_and_resize(
    image: &DynamicImage,
    crop: CropRect,
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

fn crop_with_padding(image: &DynamicImage, crop: CropRect) -> RgbaImage {
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
    let width = u32::try_from(cell_width.max(1)).unwrap_or(u32::MAX);
    let height = u32::try_from(cell_height.max(1)).unwrap_or(u32::MAX);

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
    use image::{DynamicImage, ImageBuffer, Rgba};

    use super::{crop_and_resize, CropRect};

    #[test]
    fn crop_with_padding_keeps_requested_output_size() {
        let source = ImageBuffer::from_pixel(10, 10, Rgba([255, 0, 0, 255]));
        let image = DynamicImage::ImageRgba8(source);
        let output = crop_and_resize(
            &image,
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
