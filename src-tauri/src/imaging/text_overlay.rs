use std::fs;
use std::path::{Path, PathBuf};

use fontdue::{Font, FontSettings};
use image::RgbaImage;

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, PartialEq)]
pub struct TextOverlayRenderSpec {
    pub enabled: bool,
    pub text: String,
    pub font_path: Option<PathBuf>,
    pub font_size: f32,
    pub x: f32,
    pub y: f32,
    pub color: [u8; 4],
    pub stroke_color: [u8; 4],
    pub stroke_width: f32,
}

impl TextOverlayRenderSpec {
    pub fn normalized_hash_parts(&self) -> Vec<String> {
        vec![
            self.enabled.to_string(),
            self.text.clone(),
            self.font_path
                .as_ref()
                .map(|path| path.to_string_lossy().to_string())
                .unwrap_or_default(),
            format!("{:.2}", self.font_size),
            format!("{:.4}", self.x),
            format!("{:.4}", self.y),
            rgba_to_hex(self.color),
            rgba_to_hex(self.stroke_color),
            format!("{:.2}", self.stroke_width),
        ]
    }
}

pub fn text_overlay_from_fields(
    enabled: bool,
    text: Option<String>,
    font_path: Option<String>,
    font_size: Option<f64>,
    x: Option<f64>,
    y: Option<f64>,
    color: Option<String>,
    stroke_color: Option<String>,
    stroke_width: Option<f64>,
) -> AppResult<Option<TextOverlayRenderSpec>> {
    let text = text.unwrap_or_default();
    if !enabled || text.trim().is_empty() {
        return Ok(None);
    }

    Ok(Some(TextOverlayRenderSpec {
        enabled,
        text,
        font_path: font_path
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from),
        font_size: font_size.unwrap_or(28.0).clamp(1.0, 512.0) as f32,
        x: x.unwrap_or(0.5).clamp(0.0, 1.0) as f32,
        y: y.unwrap_or(0.82).clamp(0.0, 1.0) as f32,
        color: parse_hex_rgba(color.as_deref().unwrap_or("#FFFFFF"))?,
        stroke_color: parse_hex_rgba(stroke_color.as_deref().unwrap_or("#000000"))?,
        stroke_width: stroke_width.unwrap_or(2.0).clamp(0.0, 64.0) as f32,
    }))
}

pub fn apply_text_overlay(
    image: &mut RgbaImage,
    spec: Option<&TextOverlayRenderSpec>,
) -> AppResult<()> {
    let Some(spec) = spec else {
        return Ok(());
    };
    if !spec.enabled || spec.text.trim().is_empty() {
        return Ok(());
    }

    let font_bytes = load_font_bytes(spec.font_path.as_deref())?;
    let font = Font::from_bytes(font_bytes, FontSettings::default())
        .map_err(|error| AppError::new("font", format!("폰트를 읽을 수 없습니다: {error}")))?;
    draw_multiline_text(image, &font, spec);
    Ok(())
}

fn draw_multiline_text(image: &mut RgbaImage, font: &Font, spec: &TextOverlayRenderSpec) {
    let lines = spec.text.lines().collect::<Vec<_>>();
    if lines.is_empty() {
        return;
    }

    let line_height = (spec.font_size * 1.2).max(1.0);
    let total_height = line_height * lines.len() as f32;
    let center_x = image.width() as f32 * spec.x;
    let center_y = image.height() as f32 * spec.y;
    let mut line_top = center_y - total_height / 2.0;

    for line in lines {
        let width = measure_text_width(font, line, spec.font_size);
        let start_x = center_x - width / 2.0;
        let baseline_y = line_top + spec.font_size;

        if spec.stroke_width > 0.0 && spec.stroke_color[3] > 0 {
            let radius = spec.stroke_width.ceil() as i32;
            for offset_y in -radius..=radius {
                for offset_x in -radius..=radius {
                    if offset_x == 0 && offset_y == 0 {
                        continue;
                    }
                    if (offset_x * offset_x + offset_y * offset_y) as f32
                        <= spec.stroke_width * spec.stroke_width
                    {
                        draw_line(
                            image,
                            font,
                            line,
                            spec.font_size,
                            start_x + offset_x as f32,
                            baseline_y + offset_y as f32,
                            spec.stroke_color,
                        );
                    }
                }
            }
        }

        draw_line(
            image,
            font,
            line,
            spec.font_size,
            start_x,
            baseline_y,
            spec.color,
        );
        line_top += line_height;
    }
}

fn draw_line(
    image: &mut RgbaImage,
    font: &Font,
    text: &str,
    size: f32,
    start_x: f32,
    baseline_y: f32,
    color: [u8; 4],
) {
    let mut pen_x = start_x;

    for character in text.chars() {
        if character == '\r' {
            continue;
        }
        let (metrics, bitmap) = font.rasterize(character, size);
        let glyph_x = pen_x + metrics.xmin as f32;
        let glyph_y = baseline_y - metrics.height as f32 - metrics.ymin as f32;

        for row in 0..metrics.height {
            for column in 0..metrics.width {
                let coverage = bitmap[row * metrics.width + column];
                if coverage == 0 {
                    continue;
                }
                let x = (glyph_x + column as f32).round() as i64;
                let y = (glyph_y + row as f32).round() as i64;
                if x < 0 || y < 0 || x >= i64::from(image.width()) || y >= i64::from(image.height())
                {
                    continue;
                }
                blend_pixel(image, x as u32, y as u32, color, coverage);
            }
        }

        pen_x += metrics.advance_width;
    }
}

fn blend_pixel(image: &mut RgbaImage, x: u32, y: u32, color: [u8; 4], coverage: u8) {
    let src_alpha = (f32::from(coverage) / 255.0) * (f32::from(color[3]) / 255.0);
    if src_alpha <= 0.0 {
        return;
    }

    let dst = image.get_pixel_mut(x, y);
    let dst_alpha = f32::from(dst.0[3]) / 255.0;
    let out_alpha = src_alpha + dst_alpha * (1.0 - src_alpha);
    if out_alpha <= 0.0 {
        return;
    }

    for (index, channel) in color.iter().take(3).enumerate() {
        let src = f32::from(*channel) / 255.0;
        let dst_channel = f32::from(dst.0[index]) / 255.0;
        let out = (src * src_alpha + dst_channel * dst_alpha * (1.0 - src_alpha)) / out_alpha;
        dst.0[index] = (out.clamp(0.0, 1.0) * 255.0).round() as u8;
    }
    dst.0[3] = (out_alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
}

fn measure_text_width(font: &Font, text: &str, size: f32) -> f32 {
    text.chars()
        .filter(|character| *character != '\r')
        .map(|character| font.metrics(character, size).advance_width)
        .sum::<f32>()
}

fn load_font_bytes(font_path: Option<&Path>) -> AppResult<Vec<u8>> {
    if let Some(path) = font_path {
        if path.is_file() {
            return Ok(fs::read(path)?);
        }
        return Err(AppError::not_found(
            "선택한 폰트 파일을 찾을 수 없습니다. 다시 선택해 주세요.",
        ));
    }

    for candidate in default_font_candidates() {
        if candidate.is_file() {
            return Ok(fs::read(candidate)?);
        }
    }

    Err(AppError::not_found(
        "상용 무료 기본 한글 폰트를 찾지 못했습니다. 고급 편집에서 OFL/MIT 등 사용 가능한 폰트 파일을 선택해 주세요.",
    ))
}

fn default_font_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if cfg!(target_os = "windows") {
        let fonts = PathBuf::from(r"C:\Windows\Fonts");
        candidates.extend([
            fonts.join("NotoSansKR-Regular.otf"),
            fonts.join("NotoSansKR-Regular.ttf"),
            fonts.join("NotoSansCJKkr-Regular.otf"),
            fonts.join("NanumGothic.ttf"),
            fonts.join("D2Coding.ttf"),
        ]);
    }
    candidates
}

fn parse_hex_rgba(value: &str) -> AppResult<[u8; 4]> {
    let normalized = value.trim().trim_start_matches('#');
    let parse_pair = |index: usize| -> AppResult<u8> {
        u8::from_str_radix(&normalized[index..index + 2], 16).map_err(|_| {
            AppError::new(
                "validation",
                "색상은 #RRGGBB 또는 #RRGGBBAA 형식이어야 합니다.",
            )
        })
    };

    match normalized.len() {
        6 => Ok([parse_pair(0)?, parse_pair(2)?, parse_pair(4)?, 255]),
        8 => Ok([
            parse_pair(0)?,
            parse_pair(2)?,
            parse_pair(4)?,
            parse_pair(6)?,
        ]),
        _ => Err(AppError::new(
            "validation",
            "색상은 #RRGGBB 또는 #RRGGBBAA 형식이어야 합니다.",
        )),
    }
}

fn rgba_to_hex(color: [u8; 4]) -> String {
    format!(
        "#{:02X}{:02X}{:02X}{:02X}",
        color[0], color[1], color[2], color[3]
    )
}

#[cfg(test)]
mod tests {
    use super::{parse_hex_rgba, TextOverlayRenderSpec};

    #[test]
    fn parses_rgb_and_rgba_hex() {
        assert_eq!(parse_hex_rgba("#ffffff").unwrap(), [255, 255, 255, 255]);
        assert_eq!(parse_hex_rgba("#00000080").unwrap(), [0, 0, 0, 128]);
    }

    #[test]
    fn text_overlay_hash_parts_include_font_and_position() {
        let spec = TextOverlayRenderSpec {
            enabled: true,
            text: "테스트".to_string(),
            font_path: Some("font.ttf".into()),
            font_size: 24.0,
            x: 0.5,
            y: 0.8,
            color: [255, 255, 255, 255],
            stroke_color: [0, 0, 0, 255],
            stroke_width: 2.0,
        };

        let parts = spec.normalized_hash_parts();
        assert!(parts.contains(&"테스트".to_string()));
        assert!(parts.iter().any(|part| part.contains("font.ttf")));
    }
}
