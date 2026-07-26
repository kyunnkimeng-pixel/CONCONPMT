use std::collections::HashSet;

use image::imageops;
use image::{Rgba, RgbaImage};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

pub const EFFECT_RECIPE_SCHEMA: &str = "pmtcon-effects-v1";
pub const EFFECT_RECIPE_VERSION: i64 = 1;
pub const MAX_EFFECT_STEPS: usize = 16;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EffectRecipe {
    pub version: i64,
    pub effects: Vec<EffectStep>,
}

impl Default for EffectRecipe {
    fn default() -> Self {
        Self {
            version: EFFECT_RECIPE_VERSION,
            effects: Vec::new(),
        }
    }
}

impl EffectRecipe {
    pub fn has_enabled_effects(&self) -> bool {
        self.effects.iter().any(EffectStep::enabled)
    }

    pub fn normalized_hash_parts(&self) -> AppResult<Vec<String>> {
        validate_effect_recipe(self)?;
        let mut parts = vec![EFFECT_RECIPE_SCHEMA.to_string(), self.version.to_string()];
        parts.extend(self.effects.iter().map(EffectStep::render_hash_part));
        Ok(parts)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum EffectStep {
    #[serde(rename = "pixelate")]
    Pixelate {
        id: String,
        enabled: bool,
        #[serde(rename = "blockSize")]
        block_size: u32,
    },
    #[serde(rename = "color_adjust")]
    ColorAdjust {
        id: String,
        enabled: bool,
        brightness: i32,
        contrast: i32,
        saturation: i32,
        hue: i32,
    },
    #[serde(rename = "tone")]
    Tone {
        id: String,
        enabled: bool,
        mode: ToneMode,
        amount: u32,
    },
    #[serde(rename = "blur")]
    Blur {
        id: String,
        enabled: bool,
        radius: u32,
    },
    #[serde(rename = "sharpen")]
    Sharpen {
        id: String,
        enabled: bool,
        amount: u32,
    },
    #[serde(rename = "outline")]
    Outline {
        id: String,
        enabled: bool,
        radius: u32,
        color: String,
    },
    #[serde(rename = "shadow")]
    Shadow {
        id: String,
        enabled: bool,
        #[serde(rename = "offsetX")]
        offset_x: i32,
        #[serde(rename = "offsetY")]
        offset_y: i32,
        #[serde(rename = "blurRadius")]
        blur_radius: u32,
        color: String,
    },
}

impl EffectStep {
    fn id(&self) -> &str {
        match self {
            Self::Pixelate { id, .. }
            | Self::ColorAdjust { id, .. }
            | Self::Tone { id, .. }
            | Self::Blur { id, .. }
            | Self::Sharpen { id, .. }
            | Self::Outline { id, .. }
            | Self::Shadow { id, .. } => id,
        }
    }

    pub fn enabled(&self) -> bool {
        match self {
            Self::Pixelate { enabled, .. }
            | Self::ColorAdjust { enabled, .. }
            | Self::Tone { enabled, .. }
            | Self::Blur { enabled, .. }
            | Self::Sharpen { enabled, .. }
            | Self::Outline { enabled, .. }
            | Self::Shadow { enabled, .. } => *enabled,
        }
    }

    fn render_hash_part(&self) -> String {
        match self {
            Self::Pixelate {
                enabled,
                block_size,
                ..
            } => format!("pixelate|{enabled}|{block_size}"),
            Self::ColorAdjust {
                enabled,
                brightness,
                contrast,
                saturation,
                hue,
                ..
            } => format!("color_adjust|{enabled}|{brightness}|{contrast}|{saturation}|{hue}"),
            Self::Tone {
                enabled,
                mode,
                amount,
                ..
            } => format!("tone|{enabled}|{mode:?}|{amount}"),
            Self::Blur {
                enabled, radius, ..
            } => format!("blur|{enabled}|{radius}"),
            Self::Sharpen {
                enabled, amount, ..
            } => format!("sharpen|{enabled}|{amount}"),
            Self::Outline {
                enabled,
                radius,
                color,
                ..
            } => format!(
                "outline|{enabled}|{radius}|{}",
                color.trim().to_ascii_lowercase()
            ),
            Self::Shadow {
                enabled,
                offset_x,
                offset_y,
                blur_radius,
                color,
                ..
            } => format!(
                "shadow|{enabled}|{offset_x}|{offset_y}|{blur_radius}|{}",
                color.trim().to_ascii_lowercase()
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToneMode {
    Grayscale,
    Sepia,
}

pub fn parse_effect_recipe_json(value: &str) -> AppResult<EffectRecipe> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(EffectRecipe::default());
    }
    let recipe = serde_json::from_str::<EffectRecipe>(value).map_err(|error| {
        AppError::new(
            "validation",
            format!("저장된 효과 recipe를 읽을 수 없습니다: {error}"),
        )
    })?;
    validate_effect_recipe(&recipe)?;
    Ok(recipe)
}

pub fn effect_recipe_json(recipe: &EffectRecipe) -> AppResult<String> {
    validate_effect_recipe(recipe)?;
    serde_json::to_string(recipe).map_err(|error| {
        AppError::new(
            "validation",
            format!("효과 recipe를 저장 형식으로 만들 수 없습니다: {error}"),
        )
    })
}

pub fn validate_effect_recipe(recipe: &EffectRecipe) -> AppResult<()> {
    if recipe.version != EFFECT_RECIPE_VERSION {
        return Err(AppError::new(
            "validation",
            format!(
                "지원하지 않는 효과 recipe 버전입니다. 현재 지원 버전은 {}입니다.",
                EFFECT_RECIPE_VERSION
            ),
        ));
    }
    if recipe.effects.len() > MAX_EFFECT_STEPS {
        return Err(AppError::new(
            "validation",
            format!("효과는 최대 {MAX_EFFECT_STEPS}개까지 쌓을 수 있습니다."),
        ));
    }

    let mut ids = HashSet::with_capacity(recipe.effects.len());
    for step in &recipe.effects {
        let id = step.id().trim();
        if id.is_empty() || id.chars().count() > 80 {
            return Err(AppError::new(
                "validation",
                "효과 단계 ID는 1~80자여야 합니다.",
            ));
        }
        if !ids.insert(id) {
            return Err(AppError::new(
                "validation",
                "같은 효과 단계 ID를 두 번 사용할 수 없습니다.",
            ));
        }

        match step {
            EffectStep::Pixelate { block_size, .. } => {
                validate_range("픽셀 블록 크기", *block_size, 1, 64)?;
            }
            EffectStep::ColorAdjust {
                brightness,
                contrast,
                saturation,
                hue,
                ..
            } => {
                validate_signed_range("밝기", *brightness, -100, 100)?;
                validate_signed_range("대비", *contrast, -100, 100)?;
                validate_signed_range("채도", *saturation, -100, 100)?;
                validate_signed_range("색조", *hue, -180, 180)?;
            }
            EffectStep::Tone { amount, .. } => {
                validate_range("색감 강도", *amount, 0, 100)?;
            }
            EffectStep::Blur { radius, .. } => {
                validate_range("블러 반경", *radius, 0, 32)?;
            }
            EffectStep::Sharpen { amount, .. } => {
                validate_range("선명화 강도", *amount, 0, 100)?;
            }
            EffectStep::Outline { radius, color, .. } => {
                validate_range("윤곽선 두께", *radius, 1, 32)?;
                let _ = parse_hex_rgba(color)?;
            }
            EffectStep::Shadow {
                offset_x,
                offset_y,
                blur_radius,
                color,
                ..
            } => {
                validate_signed_range("그림자 X 거리", *offset_x, -128, 128)?;
                validate_signed_range("그림자 Y 거리", *offset_y, -128, 128)?;
                validate_range("그림자 흐림", *blur_radius, 0, 32)?;
                let _ = parse_hex_rgba(color)?;
            }
        }
    }

    Ok(())
}

pub fn apply_effect_recipe(image: &mut RgbaImage, recipe: &EffectRecipe) -> AppResult<()> {
    validate_effect_recipe(recipe)?;
    for step in recipe.effects.iter().filter(|step| step.enabled()) {
        match step {
            EffectStep::Pixelate { block_size, .. } => pixelate(image, *block_size),
            EffectStep::ColorAdjust {
                brightness,
                contrast,
                saturation,
                hue,
                ..
            } => color_adjust(image, *brightness, *contrast, *saturation, *hue),
            EffectStep::Tone { mode, amount, .. } => apply_tone(image, *mode, *amount),
            EffectStep::Blur { radius, .. } => blur(image, *radius),
            EffectStep::Sharpen { amount, .. } => sharpen(image, *amount),
            EffectStep::Outline { radius, color, .. } => {
                outline(image, *radius, parse_hex_rgba(color)?)
            }
            EffectStep::Shadow {
                offset_x,
                offset_y,
                blur_radius,
                color,
                ..
            } => shadow(
                image,
                *offset_x,
                *offset_y,
                *blur_radius,
                parse_hex_rgba(color)?,
            ),
        }
    }
    Ok(())
}

fn validate_range(label: &str, value: u32, min: u32, max: u32) -> AppResult<()> {
    if (min..=max).contains(&value) {
        Ok(())
    } else {
        Err(AppError::new(
            "validation",
            format!("{label}는 {min}~{max} 범위여야 합니다."),
        ))
    }
}

fn validate_signed_range(label: &str, value: i32, min: i32, max: i32) -> AppResult<()> {
    if (min..=max).contains(&value) {
        Ok(())
    } else {
        Err(AppError::new(
            "validation",
            format!("{label}는 {min}~{max} 범위여야 합니다."),
        ))
    }
}

fn pixelate(image: &mut RgbaImage, block_size: u32) {
    if block_size <= 1 {
        return;
    }
    let width = image.width();
    let height = image.height();

    for top in (0..height).step_by(block_size as usize) {
        for left in (0..width).step_by(block_size as usize) {
            let right = (left + block_size).min(width);
            let bottom = (top + block_size).min(height);
            let mut alpha_sum = 0_u64;
            let mut premultiplied = [0_u64; 3];
            let mut count = 0_u64;

            for y in top..bottom {
                for x in left..right {
                    let pixel = image.get_pixel(x, y).0;
                    let alpha = u64::from(pixel[3]);
                    alpha_sum += alpha;
                    premultiplied[0] += u64::from(pixel[0]) * alpha;
                    premultiplied[1] += u64::from(pixel[1]) * alpha;
                    premultiplied[2] += u64::from(pixel[2]) * alpha;
                    count += 1;
                }
            }

            let alpha = if count == 0 {
                0
            } else {
                ((alpha_sum + count / 2) / count) as u8
            };
            let color = if alpha_sum == 0 {
                [0, 0, 0]
            } else {
                [
                    ((premultiplied[0] + alpha_sum / 2) / alpha_sum) as u8,
                    ((premultiplied[1] + alpha_sum / 2) / alpha_sum) as u8,
                    ((premultiplied[2] + alpha_sum / 2) / alpha_sum) as u8,
                ]
            };
            let pixel = Rgba([color[0], color[1], color[2], alpha]);
            for y in top..bottom {
                for x in left..right {
                    image.put_pixel(x, y, pixel);
                }
            }
        }
    }
}

fn color_adjust(image: &mut RgbaImage, brightness: i32, contrast: i32, saturation: i32, hue: i32) {
    let contrast_factor = ((100.0 + contrast as f32) / 100.0).powi(2);
    let saturation_factor = (100.0 + saturation as f32) / 100.0;
    let brightness_offset = brightness as f32 * 2.55;

    for pixel in image.pixels_mut() {
        let alpha = pixel[3];
        let mut channels = [
            pixel[0] as f32 + brightness_offset,
            pixel[1] as f32 + brightness_offset,
            pixel[2] as f32 + brightness_offset,
        ];
        for channel in &mut channels {
            *channel = (((*channel / 255.0 - 0.5) * contrast_factor) + 0.5) * 255.0;
        }
        let luminance = channels[0] * 0.2126 + channels[1] * 0.7152 + channels[2] * 0.0722;
        for channel in &mut channels {
            *channel = luminance + (*channel - luminance) * saturation_factor;
        }
        let rotated = rotate_hue(
            clamp_byte(channels[0]),
            clamp_byte(channels[1]),
            clamp_byte(channels[2]),
            hue,
        );
        *pixel = Rgba([rotated[0], rotated[1], rotated[2], alpha]);
    }
}

fn rotate_hue(red: u8, green: u8, blue: u8, degrees: i32) -> [u8; 3] {
    if degrees == 0 {
        return [red, green, blue];
    }
    let red = red as f32 / 255.0;
    let green = green as f32 / 255.0;
    let blue = blue as f32 / 255.0;
    let max = red.max(green).max(blue);
    let min = red.min(green).min(blue);
    let delta = max - min;
    let mut hue = if delta <= f32::EPSILON {
        0.0
    } else if (max - red).abs() <= f32::EPSILON {
        60.0 * ((green - blue) / delta).rem_euclid(6.0)
    } else if (max - green).abs() <= f32::EPSILON {
        60.0 * (((blue - red) / delta) + 2.0)
    } else {
        60.0 * (((red - green) / delta) + 4.0)
    };
    hue = (hue + degrees as f32).rem_euclid(360.0);
    let saturation = if max <= f32::EPSILON {
        0.0
    } else {
        delta / max
    };
    let chroma = max * saturation;
    let x = chroma * (1.0 - ((hue / 60.0).rem_euclid(2.0) - 1.0).abs());
    let m = max - chroma;
    let (red, green, blue) = match hue {
        value if value < 60.0 => (chroma, x, 0.0),
        value if value < 120.0 => (x, chroma, 0.0),
        value if value < 180.0 => (0.0, chroma, x),
        value if value < 240.0 => (0.0, x, chroma),
        value if value < 300.0 => (x, 0.0, chroma),
        _ => (chroma, 0.0, x),
    };
    [
        clamp_byte((red + m) * 255.0),
        clamp_byte((green + m) * 255.0),
        clamp_byte((blue + m) * 255.0),
    ]
}

fn apply_tone(image: &mut RgbaImage, mode: ToneMode, amount: u32) {
    if amount == 0 {
        return;
    }
    let amount = amount as f32 / 100.0;
    for pixel in image.pixels_mut() {
        let original = [pixel[0] as f32, pixel[1] as f32, pixel[2] as f32];
        let target = match mode {
            ToneMode::Grayscale => {
                let gray = original[0] * 0.2126 + original[1] * 0.7152 + original[2] * 0.0722;
                [gray, gray, gray]
            }
            ToneMode::Sepia => [
                original[0] * 0.393 + original[1] * 0.769 + original[2] * 0.189,
                original[0] * 0.349 + original[1] * 0.686 + original[2] * 0.168,
                original[0] * 0.272 + original[1] * 0.534 + original[2] * 0.131,
            ],
        };
        pixel[0] = clamp_byte(original[0] + (target[0] - original[0]) * amount);
        pixel[1] = clamp_byte(original[1] + (target[1] - original[1]) * amount);
        pixel[2] = clamp_byte(original[2] + (target[2] - original[2]) * amount);
    }
}

fn blur(image: &mut RgbaImage, radius: u32) {
    if radius == 0 {
        return;
    }
    *image = blur_premultiplied(image, radius as f32);
}

fn sharpen(image: &mut RgbaImage, amount: u32) {
    if amount == 0 || image.width() < 2 || image.height() < 2 {
        return;
    }
    let alpha = image.pixels().map(|pixel| pixel[3]).collect::<Vec<_>>();
    let premultiplied = premultiply_rgba(image);
    let strength = amount as f32 / 100.0;
    let kernel = [
        0.0,
        -strength,
        0.0,
        -strength,
        1.0 + 4.0 * strength,
        -strength,
        0.0,
        -strength,
        0.0,
    ];
    let mut filtered = imageops::filter3x3(&premultiplied, &kernel);
    for (pixel, original_alpha) in filtered.pixels_mut().zip(alpha) {
        pixel[3] = original_alpha;
    }
    *image = unpremultiply_rgba(&filtered);
}

fn outline(image: &mut RgbaImage, radius: u32, color: [u8; 4]) {
    if image.width() == 0 || image.height() == 0 {
        return;
    }
    let width = image.width() as usize;
    let height = image.height() as usize;
    let source_alpha = image.pixels().map(|pixel| pixel[3]).collect::<Vec<_>>();
    let dilated = dilate_alpha(&source_alpha, width, height, radius as usize);
    let original = image.clone();

    for (index, pixel) in image.pixels_mut().enumerate() {
        let alpha = multiply_alpha(dilated[index], color[3]);
        *pixel = Rgba([color[0], color[1], color[2], alpha]);
    }
    composite_over(image, &original);
}

fn shadow(image: &mut RgbaImage, offset_x: i32, offset_y: i32, blur_radius: u32, color: [u8; 4]) {
    let width = image.width();
    let height = image.height();
    if width == 0 || height == 0 {
        return;
    }
    let original = image.clone();
    let mut mask = RgbaImage::from_pixel(width, height, Rgba([color[0], color[1], color[2], 0]));
    for (target, source) in mask.pixels_mut().zip(original.pixels()) {
        target[3] = multiply_alpha(source[3], color[3]);
    }
    if blur_radius > 0 {
        mask = blur_premultiplied(&mask, blur_radius as f32);
    }

    *image = RgbaImage::from_pixel(width, height, Rgba([0, 0, 0, 0]));
    for y in 0..height {
        for x in 0..width {
            let target_x = i64::from(x) + i64::from(offset_x);
            let target_y = i64::from(y) + i64::from(offset_y);
            if target_x < 0
                || target_y < 0
                || target_x >= i64::from(width)
                || target_y >= i64::from(height)
            {
                continue;
            }
            image.put_pixel(target_x as u32, target_y as u32, *mask.get_pixel(x, y));
        }
    }
    composite_over(image, &original);
}

fn blur_premultiplied(image: &RgbaImage, radius: f32) -> RgbaImage {
    let premultiplied = premultiply_rgba(image);
    let blurred = imageops::blur(&premultiplied, radius);
    unpremultiply_rgba(&blurred)
}

fn premultiply_rgba(image: &RgbaImage) -> RgbaImage {
    RgbaImage::from_fn(image.width(), image.height(), |x, y| {
        let pixel = image.get_pixel(x, y);
        let alpha = u16::from(pixel[3]);
        Rgba([
            ((u16::from(pixel[0]) * alpha + 127) / 255) as u8,
            ((u16::from(pixel[1]) * alpha + 127) / 255) as u8,
            ((u16::from(pixel[2]) * alpha + 127) / 255) as u8,
            pixel[3],
        ])
    })
}

fn unpremultiply_rgba(image: &RgbaImage) -> RgbaImage {
    RgbaImage::from_fn(image.width(), image.height(), |x, y| {
        let pixel = image.get_pixel(x, y);
        let alpha = u16::from(pixel[3]);
        if alpha == 0 {
            return Rgba([0, 0, 0, 0]);
        }
        Rgba([
            ((u16::from(pixel[0]) * 255 + alpha / 2) / alpha).min(255) as u8,
            ((u16::from(pixel[1]) * 255 + alpha / 2) / alpha).min(255) as u8,
            ((u16::from(pixel[2]) * 255 + alpha / 2) / alpha).min(255) as u8,
            pixel[3],
        ])
    })
}

fn dilate_alpha(source: &[u8], width: usize, height: usize, radius: usize) -> Vec<u8> {
    let mut horizontal = vec![0_u8; source.len()];
    for y in 0..height {
        for x in 0..width {
            let start = x.saturating_sub(radius);
            let end = (x + radius + 1).min(width);
            horizontal[y * width + x] = source[y * width + start..y * width + end]
                .iter()
                .copied()
                .max()
                .unwrap_or(0);
        }
    }

    let mut output = vec![0_u8; source.len()];
    for y in 0..height {
        let start = y.saturating_sub(radius);
        let end = (y + radius + 1).min(height);
        for x in 0..width {
            let mut maximum = 0_u8;
            for row in start..end {
                maximum = maximum.max(horizontal[row * width + x]);
            }
            output[y * width + x] = maximum;
        }
    }
    output
}

fn composite_over(bottom: &mut RgbaImage, top: &RgbaImage) {
    for (bottom_pixel, top_pixel) in bottom.pixels_mut().zip(top.pixels()) {
        *bottom_pixel = alpha_over(*bottom_pixel, *top_pixel);
    }
}

fn alpha_over(bottom: Rgba<u8>, top: Rgba<u8>) -> Rgba<u8> {
    let top_alpha = top[3] as f32 / 255.0;
    let bottom_alpha = bottom[3] as f32 / 255.0;
    let output_alpha = top_alpha + bottom_alpha * (1.0 - top_alpha);
    if output_alpha <= f32::EPSILON {
        return Rgba([0, 0, 0, 0]);
    }

    let mut output = [0_u8; 4];
    for channel in 0..3 {
        let premultiplied = top[channel] as f32 * top_alpha
            + bottom[channel] as f32 * bottom_alpha * (1.0 - top_alpha);
        output[channel] = clamp_byte(premultiplied / output_alpha);
    }
    output[3] = clamp_byte(output_alpha * 255.0);
    Rgba(output)
}

fn multiply_alpha(first: u8, second: u8) -> u8 {
    ((u16::from(first) * u16::from(second) + 127) / 255) as u8
}

fn clamp_byte(value: f32) -> u8 {
    value.round().clamp(0.0, 255.0) as u8
}

fn parse_hex_rgba(value: &str) -> AppResult<[u8; 4]> {
    let value = value.trim();
    let raw = value.strip_prefix('#').ok_or_else(invalid_effect_color)?;
    if raw.len() != 6 && raw.len() != 8 {
        return Err(invalid_effect_color());
    }

    let parse = |start: usize| {
        u8::from_str_radix(&raw[start..start + 2], 16).map_err(|_| invalid_effect_color())
    };
    Ok([
        parse(0)?,
        parse(2)?,
        parse(4)?,
        if raw.len() == 8 { parse(6)? } else { 255 },
    ])
}

fn invalid_effect_color() -> AppError {
    AppError::new(
        "validation",
        "효과 색상은 #RRGGBB 또는 #RRGGBBAA 형식이어야 합니다.",
    )
}

#[cfg(test)]
mod tests {
    use image::{Rgba, RgbaImage};

    use super::{
        apply_effect_recipe, effect_recipe_json, parse_effect_recipe_json, validate_effect_recipe,
        EffectRecipe, EffectStep, ToneMode, EFFECT_RECIPE_VERSION,
    };

    fn recipe(effects: Vec<EffectStep>) -> EffectRecipe {
        EffectRecipe {
            version: EFFECT_RECIPE_VERSION,
            effects,
        }
    }

    #[test]
    fn recipe_roundtrip_keeps_order_and_rejects_duplicate_ids() {
        let value = recipe(vec![
            EffectStep::Blur {
                id: "blur".to_string(),
                enabled: true,
                radius: 2,
            },
            EffectStep::Pixelate {
                id: "pixel".to_string(),
                enabled: false,
                block_size: 4,
            },
        ]);
        let json = effect_recipe_json(&value).unwrap();
        assert_eq!(parse_effect_recipe_json(&json).unwrap(), value);

        let duplicate = recipe(vec![
            EffectStep::Blur {
                id: "same".to_string(),
                enabled: true,
                radius: 2,
            },
            EffectStep::Sharpen {
                id: "same".to_string(),
                enabled: true,
                amount: 50,
            },
        ]);
        assert!(validate_effect_recipe(&duplicate).is_err());

        let missing_hash = recipe(vec![EffectStep::Outline {
            id: "outline".to_string(),
            enabled: true,
            radius: 1,
            color: "ffffff".to_string(),
        }]);
        assert!(validate_effect_recipe(&missing_hash).is_err());
    }

    #[test]
    fn render_hash_ignores_step_ids_but_preserves_order() {
        let first = recipe(vec![
            EffectStep::Blur {
                id: "draft-a".to_string(),
                enabled: true,
                radius: 2,
            },
            EffectStep::Pixelate {
                id: "draft-b".to_string(),
                enabled: true,
                block_size: 4,
            },
        ]);
        let renamed = recipe(vec![
            EffectStep::Blur {
                id: "saved-a".to_string(),
                enabled: true,
                radius: 2,
            },
            EffectStep::Pixelate {
                id: "saved-b".to_string(),
                enabled: true,
                block_size: 4,
            },
        ]);
        let reordered = recipe(vec![
            EffectStep::Pixelate {
                id: "saved-b".to_string(),
                enabled: true,
                block_size: 4,
            },
            EffectStep::Blur {
                id: "saved-a".to_string(),
                enabled: true,
                radius: 2,
            },
        ]);

        assert_eq!(
            first.normalized_hash_parts().unwrap(),
            renamed.normalized_hash_parts().unwrap()
        );
        assert_ne!(
            first.normalized_hash_parts().unwrap(),
            reordered.normalized_hash_parts().unwrap()
        );
    }

    #[test]
    fn color_effects_preserve_alpha_and_rotate_primary_hue() {
        let mut image = RgbaImage::from_pixel(1, 1, Rgba([255, 0, 0, 91]));
        apply_effect_recipe(
            &mut image,
            &recipe(vec![EffectStep::ColorAdjust {
                id: "color".to_string(),
                enabled: true,
                brightness: 0,
                contrast: 0,
                saturation: 0,
                hue: 120,
            }]),
        )
        .unwrap();
        let pixel = image.get_pixel(0, 0);
        assert!(pixel[1] > 245);
        assert!(pixel[0] < 10);
        assert_eq!(pixel[3], 91);
    }

    #[test]
    fn grayscale_and_sepia_are_deterministic() {
        let source = RgbaImage::from_pixel(1, 1, Rgba([200, 100, 20, 255]));
        let mut grayscale = source.clone();
        apply_effect_recipe(
            &mut grayscale,
            &recipe(vec![EffectStep::Tone {
                id: "gray".to_string(),
                enabled: true,
                mode: ToneMode::Grayscale,
                amount: 100,
            }]),
        )
        .unwrap();
        let grayscale_pixel = grayscale.get_pixel(0, 0);
        assert_eq!(grayscale_pixel[0], grayscale_pixel[1]);
        assert_eq!(grayscale_pixel[1], grayscale_pixel[2]);

        let mut sepia_a = source.clone();
        let mut sepia_b = source;
        let sepia_recipe = recipe(vec![EffectStep::Tone {
            id: "sepia".to_string(),
            enabled: true,
            mode: ToneMode::Sepia,
            amount: 100,
        }]);
        apply_effect_recipe(&mut sepia_a, &sepia_recipe).unwrap();
        apply_effect_recipe(&mut sepia_b, &sepia_recipe).unwrap();
        assert_eq!(sepia_a, sepia_b);
        let sepia_pixel = sepia_a.get_pixel(0, 0);
        assert!(sepia_pixel[0] >= sepia_pixel[1]);
        assert!(sepia_pixel[1] >= sepia_pixel[2]);
    }

    #[test]
    fn ordered_recipes_produce_different_pixels() {
        let mut source = RgbaImage::from_pixel(5, 5, Rgba([0, 0, 0, 255]));
        source.put_pixel(2, 2, Rgba([255, 30, 220, 255]));
        let pixelate = EffectStep::Pixelate {
            id: "pixel".to_string(),
            enabled: true,
            block_size: 2,
        };
        let blur = EffectStep::Blur {
            id: "blur".to_string(),
            enabled: true,
            radius: 1,
        };
        let mut first = source.clone();
        let mut second = source;
        apply_effect_recipe(&mut first, &recipe(vec![pixelate.clone(), blur.clone()])).unwrap();
        apply_effect_recipe(&mut second, &recipe(vec![blur, pixelate])).unwrap();
        assert_ne!(first, second);
    }

    #[test]
    fn blur_uses_premultiplied_alpha_without_dark_transparent_fringe() {
        let mut image = RgbaImage::from_pixel(5, 1, Rgba([0, 0, 0, 0]));
        image.put_pixel(2, 0, Rgba([255, 0, 0, 255]));
        apply_effect_recipe(
            &mut image,
            &recipe(vec![EffectStep::Blur {
                id: "blur".to_string(),
                enabled: true,
                radius: 1,
            }]),
        )
        .unwrap();

        let fringe = image.get_pixel(1, 0);
        assert!(fringe[3] > 0);
        assert!(fringe[0] > 245);
        assert_eq!(fringe[1], 0);
        assert_eq!(fringe[2], 0);
    }

    #[test]
    fn outline_and_shadow_keep_canvas_and_fill_transparent_neighbors() {
        let mut image = RgbaImage::from_pixel(5, 5, Rgba([0, 0, 0, 0]));
        image.put_pixel(2, 2, Rgba([255, 255, 255, 255]));
        apply_effect_recipe(
            &mut image,
            &recipe(vec![
                EffectStep::Outline {
                    id: "outline".to_string(),
                    enabled: true,
                    radius: 1,
                    color: "#ff0000ff".to_string(),
                },
                EffectStep::Shadow {
                    id: "shadow".to_string(),
                    enabled: true,
                    offset_x: 1,
                    offset_y: 1,
                    blur_radius: 0,
                    color: "#0000ffff".to_string(),
                },
            ]),
        )
        .unwrap();

        assert_eq!(image.dimensions(), (5, 5));
        assert!(image.get_pixel(1, 2)[3] > 0);
        assert!(image.get_pixel(3, 3)[3] > 0);
        assert_eq!(*image.get_pixel(2, 2), Rgba([255, 255, 255, 255]));
    }

    #[test]
    fn disabled_effect_is_identity() {
        let original = RgbaImage::from_pixel(3, 3, Rgba([10, 20, 30, 40]));
        let mut output = original.clone();
        apply_effect_recipe(
            &mut output,
            &recipe(vec![EffectStep::Blur {
                id: "blur".to_string(),
                enabled: false,
                radius: 32,
            }]),
        )
        .unwrap();
        assert_eq!(output, original);
    }
}
