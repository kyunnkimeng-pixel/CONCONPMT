use std::fs::File;
use std::io::{Cursor, Read};
use std::path::Path;

use image::{DynamicImage, GenericImageView, ImageError, ImageFormat, ImageReader, Limits};

use crate::error::{AppError, AppResult};

pub const MAX_IMPORT_FILE_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_IMPORT_DIMENSION: u32 = 12_000;
pub const MAX_IMPORT_PIXELS: u64 = 32_000_000;
pub const MAX_GIF_FRAMES: i64 = 500;
pub const MAX_GIF_TOTAL_FRAME_PIXELS: u64 = 128_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidatedCropRect {
    pub x: i64,
    pub y: i64,
    pub width: u32,
    pub height: u32,
}

pub fn validate_crop_rect(x: f64, y: f64, width: f64, height: f64) -> AppResult<ValidatedCropRect> {
    if !x.is_finite() || !y.is_finite() || !width.is_finite() || !height.is_finite() {
        return Err(AppError::new(
            "validation",
            "크롭 좌표와 크기는 유한한 숫자여야 합니다.",
        ));
    }
    let max_dimension = f64::from(MAX_IMPORT_DIMENSION);
    if x.abs() > max_dimension || y.abs() > max_dimension {
        return Err(AppError::new(
            "validation",
            "크롭 좌표가 지원 범위를 벗어났습니다.",
        ));
    }
    if width < 1.0 || height < 1.0 || width > max_dimension || height > max_dimension {
        return Err(AppError::new(
            "validation",
            "크롭 너비와 높이는 1~12,000px 범위여야 합니다.",
        ));
    }

    let x = x.round() as i64;
    let y = y.round() as i64;
    let width = width.round() as u32;
    let height = height.round() as u32;
    validate_import_dimensions(width, height)?;
    x.checked_add(i64::from(width))
        .and_then(|_| y.checked_add(i64::from(height)))
        .ok_or_else(|| AppError::new("validation", "크롭 영역 좌표가 너무 큽니다."))?;

    Ok(ValidatedCropRect {
        x,
        y,
        width,
        height,
    })
}

pub fn decode_import_image(bytes: &[u8], format: ImageFormat) -> AppResult<DynamicImage> {
    validate_import_file_size(bytes.len())?;

    let limits = decoder_limits();
    let mut dimension_reader = ImageReader::with_format(Cursor::new(bytes), format);
    dimension_reader.limits(limits.clone());
    let (width, height) = dimension_reader
        .into_dimensions()
        .map_err(import_decode_error)?;
    validate_import_dimensions(width, height)?;

    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    reader.limits(limits);
    let image = reader.decode().map_err(import_decode_error)?;
    let decoded_dimensions = image.dimensions();
    validate_import_dimensions(decoded_dimensions.0, decoded_dimensions.1)?;

    Ok(image)
}

pub fn read_import_file_bytes(path: &Path) -> AppResult<Vec<u8>> {
    let file = File::open(path)?;
    let metadata_size = usize::try_from(file.metadata()?.len())
        .map_err(|_| AppError::new("validation", "원본 파일 크기를 확인할 수 없습니다."))?;
    validate_import_file_size(metadata_size)?;

    let mut bytes = Vec::with_capacity(metadata_size.min(MAX_IMPORT_FILE_BYTES));
    let mut limited = file.take((MAX_IMPORT_FILE_BYTES as u64).saturating_add(1));
    limited.read_to_end(&mut bytes)?;
    validate_import_file_size(bytes.len())?;
    Ok(bytes)
}

pub fn validate_import_file_size(byte_size: usize) -> AppResult<()> {
    if byte_size > MAX_IMPORT_FILE_BYTES {
        return Err(AppError::new(
            "validation",
            "원본 파일은 최대 64MB까지 가져올 수 있습니다.",
        ));
    }

    Ok(())
}

pub fn validate_import_dimensions(width: u32, height: u32) -> AppResult<()> {
    if width == 0 || height == 0 {
        return Err(AppError::new(
            "validation",
            "가로세로 크기가 없는 이미지는 가져올 수 없습니다.",
        ));
    }
    if width > MAX_IMPORT_DIMENSION || height > MAX_IMPORT_DIMENSION {
        return Err(AppError::new("validation", image_limit_message()));
    }

    let pixels = u64::from(width).saturating_mul(u64::from(height));
    if pixels > MAX_IMPORT_PIXELS {
        return Err(AppError::new(
            "validation",
            "이미지 전체 픽셀 수는 최대 3,200만 픽셀까지 지원합니다.",
        ));
    }

    Ok(())
}

pub fn validate_gif_workload(width: u32, height: u32, frame_count: i64) -> Result<(), String> {
    if frame_count > MAX_GIF_FRAMES {
        return Err(format!(
            "GIF는 최대 {MAX_GIF_FRAMES}프레임까지 가져올 수 있습니다."
        ));
    }

    let total_frame_pixels = u64::from(width)
        .saturating_mul(u64::from(height))
        .saturating_mul(frame_count.max(0) as u64);
    if total_frame_pixels > MAX_GIF_TOTAL_FRAME_PIXELS {
        return Err("GIF의 프레임 수와 해상도 조합이 너무 큽니다.".to_string());
    }

    Ok(())
}

fn image_limit_message() -> &'static str {
    "이미지는 한 변 최대 12,000px, 전체 3,200만 픽셀까지 가져올 수 있습니다."
}

fn decoder_limits() -> Limits {
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_IMPORT_DIMENSION);
    limits.max_image_height = Some(MAX_IMPORT_DIMENSION);
    limits.max_alloc = Some(MAX_IMPORT_PIXELS * 4 + 16 * 1024 * 1024);
    limits
}

fn import_decode_error(error: ImageError) -> AppError {
    match error {
        ImageError::Limits(_) => AppError::new("validation", image_limit_message()),
        _ => AppError::new("validation", "이미지 파일을 해석할 수 없습니다."),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        validate_crop_rect, validate_gif_workload, validate_import_dimensions,
        validate_import_file_size, MAX_GIF_FRAMES, MAX_IMPORT_FILE_BYTES,
    };

    #[test]
    fn import_limits_reject_oversized_file_and_dimensions() {
        assert!(validate_import_file_size(MAX_IMPORT_FILE_BYTES).is_ok());
        assert!(validate_import_file_size(MAX_IMPORT_FILE_BYTES + 1).is_err());
        assert!(validate_import_dimensions(8_000, 4_000).is_ok());
        assert!(validate_import_dimensions(8_001, 4_000).is_err());
    }

    #[test]
    fn gif_workload_rejects_excessive_frames() {
        assert!(validate_gif_workload(1, 1, MAX_GIF_FRAMES).is_ok());
        assert!(validate_gif_workload(1, 1, MAX_GIF_FRAMES + 1).is_err());
    }

    #[test]
    fn crop_validation_rejects_non_finite_and_excessive_geometry() {
        assert!(validate_crop_rect(0.0, 0.0, 8_000.0, 4_000.0).is_ok());
        assert!(validate_crop_rect(f64::NAN, 0.0, 20.0, 20.0).is_err());
        assert!(validate_crop_rect(0.0, f64::INFINITY, 20.0, 20.0).is_err());
        assert!(validate_crop_rect(0.0, 0.0, f64::INFINITY, 20.0).is_err());
        assert!(validate_crop_rect(0.0, 0.0, 12_001.0, 1.0).is_err());
        assert!(validate_crop_rect(0.0, 0.0, 8_000.0, 8_000.0).is_err());
        assert!(validate_crop_rect(12_001.0, 0.0, 1.0, 1.0).is_err());
    }
}
