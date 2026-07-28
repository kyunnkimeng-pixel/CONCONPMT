use std::io::Cursor;

use image::imageops::{self, FilterType};
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use sha2::{Digest, Sha256};

use crate::error::{AppError, AppResult};
use crate::imaging::import_limits::{
    validate_import_dimensions, MAX_IMPORT_DIMENSION, MAX_IMPORT_PIXELS,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiNormalizationMode {
    ContainPad,
    CoverCrop,
}

impl AiNormalizationMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ContainPad => "contain_pad",
            Self::CoverCrop => "cover_crop",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiNormalizationKind {
    Identity,
    ContainPad,
    CoverCrop,
}

impl AiNormalizationKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Identity => "identity",
            Self::ContainPad => "contain_pad",
            Self::CoverCrop => "cover_crop",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiNormalizationAlignment {
    TopLeft,
    Top,
    TopRight,
    Left,
    Center,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
}

impl AiNormalizationAlignment {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TopLeft => "top_left",
            Self::Top => "top",
            Self::TopRight => "top_right",
            Self::Left => "left",
            Self::Center => "center",
            Self::Right => "right",
            Self::BottomLeft => "bottom_left",
            Self::Bottom => "bottom",
            Self::BottomRight => "bottom_right",
        }
    }

    const fn horizontal_anchor(self) -> AxisAnchor {
        match self {
            Self::TopLeft | Self::Left | Self::BottomLeft => AxisAnchor::Start,
            Self::Top | Self::Center | Self::Bottom => AxisAnchor::Center,
            Self::TopRight | Self::Right | Self::BottomRight => AxisAnchor::End,
        }
    }

    const fn vertical_anchor(self) -> AxisAnchor {
        match self {
            Self::TopLeft | Self::Top | Self::TopRight => AxisAnchor::Start,
            Self::Left | Self::Center | Self::Right => AxisAnchor::Center,
            Self::BottomLeft | Self::Bottom | Self::BottomRight => AxisAnchor::End,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiNormalizationResizeFilter {
    Lanczos3,
    Nearest,
}

impl AiNormalizationResizeFilter {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Lanczos3 => "lanczos3",
            Self::Nearest => "nearest",
        }
    }

    const fn image_filter(self) -> FilterType {
        match self {
            Self::Lanczos3 => FilterType::Lanczos3,
            Self::Nearest => FilterType::Nearest,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AiNormalizationOptions {
    pub mode: AiNormalizationMode,
    pub alignment: AiNormalizationAlignment,
    pub resize_filter: AiNormalizationResizeFilter,
    pub pad_rgba: [u8; 4],
}

impl Default for AiNormalizationOptions {
    fn default() -> Self {
        Self {
            mode: AiNormalizationMode::ContainPad,
            alignment: AiNormalizationAlignment::Center,
            resize_filter: AiNormalizationResizeFilter::Lanczos3,
            pad_rgba: [0, 0, 0, 0],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AiNormalizationGeometry {
    pub kind: AiNormalizationKind,
    pub source_width: u32,
    pub source_height: u32,
    pub target_width: u32,
    pub target_height: u32,
    pub resized_width: u32,
    pub resized_height: u32,
    /// X offset in the resized image used by cover-crop.
    pub crop_x: u32,
    /// Y offset in the resized image used by cover-crop.
    pub crop_y: u32,
    /// X offset in the target canvas used by contain-pad.
    pub paste_x: u32,
    /// Y offset in the target canvas used by contain-pad.
    pub paste_y: u32,
}

#[derive(Debug, Clone)]
pub struct AiNormalizedRgba {
    pub image: RgbaImage,
    pub geometry: AiNormalizationGeometry,
}

#[derive(Debug, Clone)]
pub struct AiNormalizedPng {
    pub bytes: Vec<u8>,
    pub sha256: String,
    pub width: u32,
    pub height: u32,
    pub geometry: AiNormalizationGeometry,
}

/// Computes the canonical integer geometry used by both preview and commit.
///
/// Contain uses round-half-up for the non-limiting axis. Cover uses ceil so
/// that rounding cannot leave an empty output pixel. Center alignment assigns
/// an odd remainder to the right or bottom side by flooring the leading offset.
pub fn normalization_geometry(
    source_width: u32,
    source_height: u32,
    target_width: u32,
    target_height: u32,
    mode: AiNormalizationMode,
    alignment: AiNormalizationAlignment,
) -> AppResult<AiNormalizationGeometry> {
    validate_dimensions(source_width, source_height, "AI 후보")?;
    validate_dimensions(target_width, target_height, "대상 캔버스")?;

    if source_width == target_width && source_height == target_height {
        return Ok(AiNormalizationGeometry {
            kind: AiNormalizationKind::Identity,
            source_width,
            source_height,
            target_width,
            target_height,
            resized_width: source_width,
            resized_height: source_height,
            crop_x: 0,
            crop_y: 0,
            paste_x: 0,
            paste_y: 0,
        });
    }

    match mode {
        AiNormalizationMode::ContainPad => contain_geometry(
            source_width,
            source_height,
            target_width,
            target_height,
            alignment,
        ),
        AiNormalizationMode::CoverCrop => cover_geometry(
            source_width,
            source_height,
            target_width,
            target_height,
            alignment,
        ),
    }
}

pub fn normalize_static_image(
    source: &DynamicImage,
    target_width: u32,
    target_height: u32,
    options: AiNormalizationOptions,
) -> AppResult<AiNormalizedRgba> {
    let source = source.to_rgba8();
    normalize_rgba_image(&source, target_width, target_height, options)
}

pub fn normalize_rgba_image(
    source: &RgbaImage,
    target_width: u32,
    target_height: u32,
    options: AiNormalizationOptions,
) -> AppResult<AiNormalizedRgba> {
    let geometry = normalization_geometry(
        source.width(),
        source.height(),
        target_width,
        target_height,
        options.mode,
        options.alignment,
    )?;

    validate_intermediate_workload(geometry.resized_width, geometry.resized_height)?;

    let image = match geometry.kind {
        AiNormalizationKind::Identity => source.clone(),
        AiNormalizationKind::ContainPad => {
            let resized = imageops::resize(
                source,
                geometry.resized_width,
                geometry.resized_height,
                options.resize_filter.image_filter(),
            );
            let mut canvas =
                RgbaImage::from_pixel(target_width, target_height, Rgba(options.pad_rgba));
            imageops::replace(
                &mut canvas,
                &resized,
                i64::from(geometry.paste_x),
                i64::from(geometry.paste_y),
            );
            canvas
        }
        AiNormalizationKind::CoverCrop => {
            let resized = imageops::resize(
                source,
                geometry.resized_width,
                geometry.resized_height,
                options.resize_filter.image_filter(),
            );
            imageops::crop_imm(
                &resized,
                geometry.crop_x,
                geometry.crop_y,
                target_width,
                target_height,
            )
            .to_image()
        }
    };

    Ok(AiNormalizedRgba { image, geometry })
}

pub fn encode_normalized_png(normalized: AiNormalizedRgba) -> AppResult<AiNormalizedPng> {
    let width = normalized.image.width();
    let height = normalized.image.height();
    let mut cursor = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(normalized.image)
        .write_to(&mut cursor, ImageFormat::Png)
        .map_err(AppError::from)?;
    let bytes = cursor.into_inner();
    let sha256 = format!("{:x}", Sha256::digest(&bytes));

    Ok(AiNormalizedPng {
        bytes,
        sha256,
        width,
        height,
        geometry: normalized.geometry,
    })
}

pub fn normalize_static_image_to_png(
    source: &DynamicImage,
    target_width: u32,
    target_height: u32,
    options: AiNormalizationOptions,
) -> AppResult<AiNormalizedPng> {
    encode_normalized_png(normalize_static_image(
        source,
        target_width,
        target_height,
        options,
    )?)
}

fn contain_geometry(
    source_width: u32,
    source_height: u32,
    target_width: u32,
    target_height: u32,
    alignment: AiNormalizationAlignment,
) -> AppResult<AiNormalizationGeometry> {
    let source_wider_than_target = u128::from(source_width) * u128::from(target_height)
        >= u128::from(source_height) * u128::from(target_width);
    let (resized_width, resized_height) = if source_wider_than_target {
        (
            target_width,
            scale_round_half_up(source_height, target_width, source_width, target_height)?,
        )
    } else {
        (
            scale_round_half_up(source_width, target_height, source_height, target_width)?,
            target_height,
        )
    };
    let paste_x = aligned_offset(target_width - resized_width, alignment.horizontal_anchor());
    let paste_y = aligned_offset(target_height - resized_height, alignment.vertical_anchor());

    Ok(AiNormalizationGeometry {
        kind: AiNormalizationKind::ContainPad,
        source_width,
        source_height,
        target_width,
        target_height,
        resized_width,
        resized_height,
        crop_x: 0,
        crop_y: 0,
        paste_x,
        paste_y,
    })
}

fn cover_geometry(
    source_width: u32,
    source_height: u32,
    target_width: u32,
    target_height: u32,
    alignment: AiNormalizationAlignment,
) -> AppResult<AiNormalizationGeometry> {
    let target_wider_than_source = u128::from(target_width) * u128::from(source_height)
        >= u128::from(target_height) * u128::from(source_width);
    let (resized_width, resized_height) = if target_wider_than_source {
        (
            target_width,
            scale_ceil(source_height, target_width, source_width)?,
        )
    } else {
        (
            scale_ceil(source_width, target_height, source_height)?,
            target_height,
        )
    };
    let crop_x = aligned_offset(resized_width - target_width, alignment.horizontal_anchor());
    let crop_y = aligned_offset(resized_height - target_height, alignment.vertical_anchor());

    Ok(AiNormalizationGeometry {
        kind: AiNormalizationKind::CoverCrop,
        source_width,
        source_height,
        target_width,
        target_height,
        resized_width,
        resized_height,
        crop_x,
        crop_y,
        paste_x: 0,
        paste_y: 0,
    })
}

fn scale_round_half_up(
    source_axis: u32,
    target_limit: u32,
    source_limit: u32,
    clamp_max: u32,
) -> AppResult<u32> {
    let numerator = u128::from(source_axis) * u128::from(target_limit);
    let denominator = u128::from(source_limit);
    let scaled = (numerator + denominator / 2) / denominator;
    checked_scaled_dimension(scaled, clamp_max)
}

fn scale_ceil(source_axis: u32, target_limit: u32, source_limit: u32) -> AppResult<u32> {
    let numerator = u128::from(source_axis) * u128::from(target_limit);
    let denominator = u128::from(source_limit);
    let scaled = numerator.div_ceil(denominator);
    checked_scaled_dimension(scaled, u32::MAX)
}

fn checked_scaled_dimension(value: u128, clamp_max: u32) -> AppResult<u32> {
    let value = value.max(1).min(u128::from(clamp_max));
    u32::try_from(value).map_err(|_| {
        AppError::new(
            "ai_normalization_dimensions",
            "AI 후보 이미지의 비율 때문에 안전한 정규화 크기를 계산할 수 없습니다.",
        )
    })
}

#[derive(Debug, Clone, Copy)]
enum AxisAnchor {
    Start,
    Center,
    End,
}

const fn aligned_offset(extra: u32, anchor: AxisAnchor) -> u32 {
    match anchor {
        AxisAnchor::Start => 0,
        AxisAnchor::Center => extra / 2,
        AxisAnchor::End => extra,
    }
}

fn validate_dimensions(width: u32, height: u32, label: &str) -> AppResult<()> {
    validate_import_dimensions(width, height).map_err(|_| {
        AppError::new(
            "ai_normalization_dimensions",
            format!("{label} 크기가 AI 정규화 안전 범위를 벗어났습니다."),
        )
    })
}

fn validate_intermediate_workload(width: u32, height: u32) -> AppResult<()> {
    if width > MAX_IMPORT_DIMENSION
        || height > MAX_IMPORT_DIMENSION
        || u64::from(width) * u64::from(height) > MAX_IMPORT_PIXELS
    {
        return Err(AppError::new(
            "ai_normalization_workload",
            "AI 후보 이미지의 비율 때문에 정규화 처리 크기가 안전 범위를 벗어났습니다.",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use image::{DynamicImage, Rgba};

    use super::{
        normalization_geometry, normalize_rgba_image, normalize_static_image_to_png,
        AiNormalizationAlignment as Alignment, AiNormalizationKind as Kind,
        AiNormalizationMode as Mode, AiNormalizationOptions as Options,
        AiNormalizationResizeFilter as ResizeFilter,
    };

    fn pixel_fixture() -> image::RgbaImage {
        image::RgbaImage::from_fn(3, 1, |x, _| match x {
            0 => Rgba([255, 0, 0, 255]),
            1 => Rgba([0, 255, 0, 255]),
            _ => Rgba([0, 0, 255, 255]),
        })
    }

    #[test]
    fn identity_is_canonical_when_canvas_already_matches() {
        let geometry =
            normalization_geometry(3, 2, 3, 2, Mode::CoverCrop, Alignment::BottomRight).unwrap();
        assert_eq!(geometry.kind, Kind::Identity);
        assert_eq!((geometry.resized_width, geometry.resized_height), (3, 2));
        assert_eq!((geometry.crop_x, geometry.crop_y), (0, 0));
        assert_eq!((geometry.paste_x, geometry.paste_y), (0, 0));
    }

    #[test]
    fn contain_uses_round_half_up_and_puts_odd_remainder_last() {
        let geometry =
            normalization_geometry(3, 2, 4, 4, Mode::ContainPad, Alignment::Center).unwrap();
        assert_eq!(geometry.kind, Kind::ContainPad);
        assert_eq!((geometry.resized_width, geometry.resized_height), (4, 3));
        assert_eq!((geometry.paste_x, geometry.paste_y), (0, 0));

        let half_up =
            normalization_geometry(4, 1, 2, 2, Mode::ContainPad, Alignment::Center).unwrap();
        assert_eq!((half_up.resized_width, half_up.resized_height), (2, 1));
        assert_eq!((half_up.paste_x, half_up.paste_y), (0, 0));
    }

    #[test]
    fn cover_uses_ceil_and_alignment_for_crop_offset() {
        let centered =
            normalization_geometry(3, 2, 4, 4, Mode::CoverCrop, Alignment::Center).unwrap();
        assert_eq!((centered.resized_width, centered.resized_height), (6, 4));
        assert_eq!((centered.crop_x, centered.crop_y), (1, 0));

        let right = normalization_geometry(3, 2, 4, 4, Mode::CoverCrop, Alignment::Right).unwrap();
        assert_eq!((right.crop_x, right.crop_y), (2, 0));
    }

    #[test]
    fn all_nine_alignments_map_to_expected_offsets() {
        let cases = [
            (Alignment::TopLeft, (0, 0)),
            (Alignment::Top, (1, 0)),
            (Alignment::TopRight, (2, 0)),
            (Alignment::Left, (0, 1)),
            (Alignment::Center, (1, 1)),
            (Alignment::Right, (2, 1)),
            (Alignment::BottomLeft, (0, 2)),
            (Alignment::Bottom, (1, 2)),
            (Alignment::BottomRight, (2, 2)),
        ];
        for (alignment, expected) in cases {
            assert_eq!(
                (
                    super::aligned_offset(2, alignment.horizontal_anchor()),
                    super::aligned_offset(2, alignment.vertical_anchor()),
                ),
                expected,
                "{alignment:?}"
            );
        }
    }

    #[test]
    fn contain_nearest_pixel_fixture_uses_requested_pad_and_alignment() {
        let source = image::RgbaImage::from_pixel(1, 1, Rgba([200, 10, 20, 255]));
        let normalized = normalize_rgba_image(
            &source,
            3,
            2,
            Options {
                mode: Mode::ContainPad,
                alignment: Alignment::Right,
                resize_filter: ResizeFilter::Nearest,
                pad_rgba: [1, 2, 3, 4],
            },
        )
        .unwrap();
        let expected = [
            Rgba([1, 2, 3, 4]),
            Rgba([200, 10, 20, 255]),
            Rgba([200, 10, 20, 255]),
            Rgba([1, 2, 3, 4]),
            Rgba([200, 10, 20, 255]),
            Rgba([200, 10, 20, 255]),
        ];
        assert_eq!(
            normalized.image.pixels().copied().collect::<Vec<_>>(),
            expected
        );
    }

    #[test]
    fn cover_nearest_pixel_fixture_selects_left_center_and_right() {
        let source = pixel_fixture();
        let cases = [
            (Alignment::Left, Rgba([255, 0, 0, 255])),
            (Alignment::Center, Rgba([0, 255, 0, 255])),
            (Alignment::Right, Rgba([0, 0, 255, 255])),
        ];
        for (alignment, expected) in cases {
            let normalized = normalize_rgba_image(
                &source,
                1,
                1,
                Options {
                    mode: Mode::CoverCrop,
                    alignment,
                    resize_filter: ResizeFilter::Nearest,
                    pad_rgba: [0, 0, 0, 0],
                },
            )
            .unwrap();
            assert_eq!(*normalized.image.get_pixel(0, 0), expected);
        }
    }

    #[test]
    fn lanczos_solid_fixture_and_png_sha_are_deterministic() {
        let source =
            DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(1, 1, Rgba([12, 34, 56, 128])));
        let options = Options {
            mode: Mode::ContainPad,
            alignment: Alignment::Center,
            resize_filter: ResizeFilter::Lanczos3,
            pad_rgba: [0, 0, 0, 0],
        };
        let first = normalize_static_image_to_png(&source, 2, 2, options).unwrap();
        let second = normalize_static_image_to_png(&source, 2, 2, options).unwrap();
        assert_eq!(first.geometry.kind, Kind::ContainPad);
        assert_eq!((first.width, first.height), (2, 2));
        assert_eq!(first.bytes, second.bytes);
        assert_eq!(first.sha256, second.sha256);
        let decoded = image::load_from_memory(&first.bytes).unwrap().to_rgba8();
        assert!(decoded
            .pixels()
            .all(|pixel| *pixel == Rgba([12, 34, 56, 128])));
    }

    #[test]
    fn rejects_zero_dimensions_and_unsafe_cover_intermediate() {
        assert!(normalization_geometry(0, 1, 1, 1, Mode::ContainPad, Alignment::Center).is_err());
        assert!(normalization_geometry(1, 1, 0, 1, Mode::ContainPad, Alignment::Center).is_err());

        let narrow = image::RgbaImage::from_pixel(1, 12_000, Rgba([0, 0, 0, 0]));
        let error = normalize_rgba_image(
            &narrow,
            12_000,
            1,
            Options {
                mode: Mode::CoverCrop,
                alignment: Alignment::Center,
                resize_filter: ResizeFilter::Nearest,
                pad_rgba: [0, 0, 0, 0],
            },
        )
        .unwrap_err();
        assert_eq!(error.code, "ai_normalization_workload");
    }
}
