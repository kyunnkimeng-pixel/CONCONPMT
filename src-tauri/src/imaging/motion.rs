use std::f64::consts::{PI, TAU};

use image::{Rgba, RgbaImage};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::imaging::import_limits::MAX_GIF_FRAMES;

pub const MOTION_RECIPE_SCHEMA: &str = "pmtcon-motion-v1";
pub const MOTION_RECIPE_VERSION: i64 = 1;
pub const MIN_MOTION_DURATION_MS: i64 = 100;
pub const MAX_MOTION_DURATION_MS: i64 = 10_000;
pub const MIN_MOTION_FPS: i64 = 1;
pub const MAX_MOTION_FPS: i64 = 50;
pub const MIN_CYCLES_PER_LOOP: u32 = 1;
pub const MAX_CYCLES_PER_LOOP: u32 = 16;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MotionRecipe {
    pub version: i64,
    pub duration_ms: i64,
    pub fps: i64,
    pub seed: u32,
    pub interpolation: MotionInterpolation,
    pub edge_mode: MotionEdgeMode,
    pub spatial: Option<SpatialMotion>,
    pub displacement: Option<DisplacementMotion>,
    pub color_opacity: Option<ColorOpacityMotion>,
    pub overlay: Option<OverlayMotion>,
}

impl Default for MotionRecipe {
    fn default() -> Self {
        Self {
            version: MOTION_RECIPE_VERSION,
            duration_ms: 1_000,
            fps: 12,
            seed: 1,
            interpolation: MotionInterpolation::Bilinear,
            edge_mode: MotionEdgeMode::Transparent,
            spatial: None,
            displacement: None,
            color_opacity: None,
            overlay: None,
        }
    }
}

impl MotionRecipe {
    pub fn has_enabled_motion(&self) -> bool {
        self.spatial.as_ref().is_some_and(SpatialMotion::enabled)
            || self
                .displacement
                .as_ref()
                .is_some_and(DisplacementMotion::enabled)
            || self
                .color_opacity
                .as_ref()
                .is_some_and(ColorOpacityMotion::enabled)
            || self.overlay.as_ref().is_some_and(OverlayMotion::enabled)
    }

    pub fn normalized_hash_parts(&self) -> AppResult<Vec<String>> {
        validate_motion_recipe(self)?;
        Ok(vec![
            MOTION_RECIPE_SCHEMA.to_string(),
            self.version.to_string(),
            self.duration_ms.to_string(),
            self.fps.to_string(),
            self.seed.to_string(),
            self.interpolation.as_str().to_string(),
            self.edge_mode.as_str().to_string(),
            self.spatial
                .as_ref()
                .map(SpatialMotion::hash_part)
                .unwrap_or_else(|| "spatial:none".to_string()),
            self.displacement
                .as_ref()
                .map(DisplacementMotion::hash_part)
                .unwrap_or_else(|| "displacement:none".to_string()),
            self.color_opacity
                .as_ref()
                .map(ColorOpacityMotion::hash_part)
                .unwrap_or_else(|| "color_opacity:none".to_string()),
            self.overlay
                .as_ref()
                .map(OverlayMotion::hash_part)
                .unwrap_or_else(|| "overlay:none".to_string()),
        ])
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MotionInterpolation {
    Nearest,
    Bilinear,
}

impl MotionInterpolation {
    fn as_str(self) -> &'static str {
        match self {
            Self::Nearest => "nearest",
            Self::Bilinear => "bilinear",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MotionEdgeMode {
    Transparent,
    Clamp,
    Mirror,
}

impl MotionEdgeMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Transparent => "transparent",
            Self::Clamp => "clamp",
            Self::Mirror => "mirror",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MotionAxis {
    Horizontal,
    Vertical,
}

impl MotionAxis {
    fn as_str(self) -> &'static str {
        match self {
            Self::Horizontal => "horizontal",
            Self::Vertical => "vertical",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum SpatialMotion {
    #[serde(rename = "shake")]
    Shake {
        enabled: bool,
        #[serde(rename = "cyclesPerLoop")]
        cycles_per_loop: u32,
        #[serde(rename = "amplitudeX")]
        amplitude_x: u32,
        #[serde(rename = "amplitudeY")]
        amplitude_y: u32,
    },
    #[serde(rename = "bounce")]
    Bounce {
        enabled: bool,
        #[serde(rename = "cyclesPerLoop")]
        cycles_per_loop: u32,
        #[serde(rename = "heightPx")]
        height_px: u32,
    },
    #[serde(rename = "breathe")]
    Breathe {
        enabled: bool,
        #[serde(rename = "cyclesPerLoop")]
        cycles_per_loop: u32,
        #[serde(rename = "scalePercent")]
        scale_percent: u32,
    },
    #[serde(rename = "rock")]
    Rock {
        enabled: bool,
        #[serde(rename = "cyclesPerLoop")]
        cycles_per_loop: u32,
        #[serde(rename = "angleDegrees")]
        angle_degrees: u32,
    },
    #[serde(rename = "spin")]
    Spin {
        enabled: bool,
        #[serde(rename = "cyclesPerLoop")]
        cycles_per_loop: u32,
        clockwise: bool,
    },
}

impl SpatialMotion {
    pub fn enabled(&self) -> bool {
        match self {
            Self::Shake { enabled, .. }
            | Self::Bounce { enabled, .. }
            | Self::Breathe { enabled, .. }
            | Self::Rock { enabled, .. }
            | Self::Spin { enabled, .. } => *enabled,
        }
    }

    fn cycles_per_loop(&self) -> u32 {
        match self {
            Self::Shake {
                cycles_per_loop, ..
            }
            | Self::Bounce {
                cycles_per_loop, ..
            }
            | Self::Breathe {
                cycles_per_loop, ..
            }
            | Self::Rock {
                cycles_per_loop, ..
            }
            | Self::Spin {
                cycles_per_loop, ..
            } => *cycles_per_loop,
        }
    }

    fn hash_part(&self) -> String {
        match self {
            Self::Shake {
                enabled,
                cycles_per_loop,
                amplitude_x,
                amplitude_y,
            } => format!("spatial:shake|{enabled}|{cycles_per_loop}|{amplitude_x}|{amplitude_y}"),
            Self::Bounce {
                enabled,
                cycles_per_loop,
                height_px,
            } => format!("spatial:bounce|{enabled}|{cycles_per_loop}|{height_px}"),
            Self::Breathe {
                enabled,
                cycles_per_loop,
                scale_percent,
            } => format!("spatial:breathe|{enabled}|{cycles_per_loop}|{scale_percent}"),
            Self::Rock {
                enabled,
                cycles_per_loop,
                angle_degrees,
            } => format!("spatial:rock|{enabled}|{cycles_per_loop}|{angle_degrees}"),
            Self::Spin {
                enabled,
                cycles_per_loop,
                clockwise,
            } => format!("spatial:spin|{enabled}|{cycles_per_loop}|{clockwise}"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum DisplacementMotion {
    #[serde(rename = "wave")]
    Wave {
        enabled: bool,
        #[serde(rename = "cyclesPerLoop")]
        cycles_per_loop: u32,
        axis: MotionAxis,
        #[serde(rename = "amplitudePx")]
        amplitude_px: u32,
        #[serde(rename = "wavelengthPx")]
        wavelength_px: u32,
    },
    #[serde(rename = "jelly")]
    Jelly {
        enabled: bool,
        #[serde(rename = "cyclesPerLoop")]
        cycles_per_loop: u32,
        #[serde(rename = "amplitudeX")]
        amplitude_x: u32,
        #[serde(rename = "amplitudeY")]
        amplitude_y: u32,
        #[serde(rename = "wavelengthX")]
        wavelength_x: u32,
        #[serde(rename = "wavelengthY")]
        wavelength_y: u32,
    },
    #[serde(rename = "ripple")]
    Ripple {
        enabled: bool,
        #[serde(rename = "cyclesPerLoop")]
        cycles_per_loop: u32,
        #[serde(rename = "amplitudePx")]
        amplitude_px: u32,
        #[serde(rename = "wavelengthPx")]
        wavelength_px: u32,
        #[serde(rename = "centerXPercent")]
        center_x_percent: u32,
        #[serde(rename = "centerYPercent")]
        center_y_percent: u32,
    },
    #[serde(rename = "glitchBands")]
    GlitchBands {
        enabled: bool,
        #[serde(rename = "cyclesPerLoop")]
        cycles_per_loop: u32,
        #[serde(rename = "amplitudePx")]
        amplitude_px: u32,
        #[serde(rename = "bandHeightPx")]
        band_height_px: u32,
        #[serde(rename = "stepsPerCycle")]
        steps_per_cycle: u32,
    },
}

impl DisplacementMotion {
    pub fn enabled(&self) -> bool {
        match self {
            Self::Wave { enabled, .. }
            | Self::Jelly { enabled, .. }
            | Self::Ripple { enabled, .. }
            | Self::GlitchBands { enabled, .. } => *enabled,
        }
    }

    fn cycles_per_loop(&self) -> u32 {
        match self {
            Self::Wave {
                cycles_per_loop, ..
            }
            | Self::Jelly {
                cycles_per_loop, ..
            }
            | Self::Ripple {
                cycles_per_loop, ..
            }
            | Self::GlitchBands {
                cycles_per_loop, ..
            } => *cycles_per_loop,
        }
    }

    fn hash_part(&self) -> String {
        match self {
            Self::Wave {
                enabled,
                cycles_per_loop,
                axis,
                amplitude_px,
                wavelength_px,
            } => format!(
                "displacement:wave|{enabled}|{cycles_per_loop}|{}|{amplitude_px}|{wavelength_px}",
                axis.as_str()
            ),
            Self::Jelly {
                enabled,
                cycles_per_loop,
                amplitude_x,
                amplitude_y,
                wavelength_x,
                wavelength_y,
            } => format!(
                "displacement:jelly|{enabled}|{cycles_per_loop}|{amplitude_x}|{amplitude_y}|{wavelength_x}|{wavelength_y}"
            ),
            Self::Ripple {
                enabled,
                cycles_per_loop,
                amplitude_px,
                wavelength_px,
                center_x_percent,
                center_y_percent,
            } => format!(
                "displacement:ripple|{enabled}|{cycles_per_loop}|{amplitude_px}|{wavelength_px}|{center_x_percent}|{center_y_percent}"
            ),
            Self::GlitchBands {
                enabled,
                cycles_per_loop,
                amplitude_px,
                band_height_px,
                steps_per_cycle,
            } => format!(
                "displacement:glitch_bands|{enabled}|{cycles_per_loop}|{amplitude_px}|{band_height_px}|{steps_per_cycle}"
            ),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum ColorOpacityMotion {
    #[serde(rename = "hueCycle")]
    HueCycle {
        enabled: bool,
        #[serde(rename = "cyclesPerLoop")]
        cycles_per_loop: u32,
        #[serde(rename = "rangeDegrees")]
        range_degrees: u32,
    },
    #[serde(rename = "tintPulse")]
    TintPulse {
        enabled: bool,
        #[serde(rename = "cyclesPerLoop")]
        cycles_per_loop: u32,
        color: String,
        #[serde(rename = "amountPercent")]
        amount_percent: u32,
    },
    #[serde(rename = "brightnessSaturationPulse")]
    BrightnessSaturationPulse {
        enabled: bool,
        #[serde(rename = "cyclesPerLoop")]
        cycles_per_loop: u32,
        #[serde(rename = "brightnessPercent")]
        brightness_percent: u32,
        #[serde(rename = "saturationPercent")]
        saturation_percent: u32,
    },
    #[serde(rename = "flash")]
    Flash {
        enabled: bool,
        #[serde(rename = "cyclesPerLoop")]
        cycles_per_loop: u32,
        color: String,
        #[serde(rename = "intensityPercent")]
        intensity_percent: u32,
    },
}

impl ColorOpacityMotion {
    pub fn enabled(&self) -> bool {
        match self {
            Self::HueCycle { enabled, .. }
            | Self::TintPulse { enabled, .. }
            | Self::BrightnessSaturationPulse { enabled, .. }
            | Self::Flash { enabled, .. } => *enabled,
        }
    }

    fn cycles_per_loop(&self) -> u32 {
        match self {
            Self::HueCycle {
                cycles_per_loop, ..
            }
            | Self::TintPulse {
                cycles_per_loop, ..
            }
            | Self::BrightnessSaturationPulse {
                cycles_per_loop, ..
            }
            | Self::Flash {
                cycles_per_loop, ..
            } => *cycles_per_loop,
        }
    }

    fn hash_part(&self) -> String {
        match self {
            Self::HueCycle {
                enabled,
                cycles_per_loop,
                range_degrees,
            } => format!(
                "color_opacity:hue_cycle|{enabled}|{cycles_per_loop}|{range_degrees}"
            ),
            Self::TintPulse {
                enabled,
                cycles_per_loop,
                color,
                amount_percent,
            } => format!(
                "color_opacity:tint_pulse|{enabled}|{cycles_per_loop}|{}|{amount_percent}",
                normalized_color(color)
            ),
            Self::BrightnessSaturationPulse {
                enabled,
                cycles_per_loop,
                brightness_percent,
                saturation_percent,
            } => format!(
                "color_opacity:brightness_saturation_pulse|{enabled}|{cycles_per_loop}|{brightness_percent}|{saturation_percent}"
            ),
            Self::Flash {
                enabled,
                cycles_per_loop,
                color,
                intensity_percent,
            } => format!(
                "color_opacity:flash|{enabled}|{cycles_per_loop}|{}|{intensity_percent}",
                normalized_color(color)
            ),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum OverlayMotion {
    #[serde(rename = "focusLines")]
    FocusLines {
        enabled: bool,
        #[serde(rename = "cyclesPerLoop")]
        cycles_per_loop: u32,
        color: String,
        #[serde(rename = "lineCount")]
        line_count: u32,
        #[serde(rename = "lineWidthPx")]
        line_width_px: u32,
        #[serde(rename = "innerRadiusPercent")]
        inner_radius_percent: u32,
        #[serde(rename = "opacityPercent")]
        opacity_percent: u32,
    },
    #[serde(rename = "sparkle")]
    Sparkle {
        enabled: bool,
        #[serde(rename = "cyclesPerLoop")]
        cycles_per_loop: u32,
        color: String,
        count: u32,
        #[serde(rename = "sizePx")]
        size_px: u32,
        #[serde(rename = "opacityPercent")]
        opacity_percent: u32,
    },
    #[serde(rename = "expansionRing")]
    ExpansionRing {
        enabled: bool,
        #[serde(rename = "cyclesPerLoop")]
        cycles_per_loop: u32,
        color: String,
        #[serde(rename = "lineWidthPx")]
        line_width_px: u32,
        #[serde(rename = "maxRadiusPercent")]
        max_radius_percent: u32,
        #[serde(rename = "opacityPercent")]
        opacity_percent: u32,
    },
}

impl OverlayMotion {
    pub fn enabled(&self) -> bool {
        match self {
            Self::FocusLines { enabled, .. }
            | Self::Sparkle { enabled, .. }
            | Self::ExpansionRing { enabled, .. } => *enabled,
        }
    }

    fn cycles_per_loop(&self) -> u32 {
        match self {
            Self::FocusLines {
                cycles_per_loop, ..
            }
            | Self::Sparkle {
                cycles_per_loop, ..
            }
            | Self::ExpansionRing {
                cycles_per_loop, ..
            } => *cycles_per_loop,
        }
    }

    fn hash_part(&self) -> String {
        match self {
            Self::FocusLines {
                enabled,
                cycles_per_loop,
                color,
                line_count,
                line_width_px,
                inner_radius_percent,
                opacity_percent,
            } => format!(
                "overlay:focus_lines|{enabled}|{cycles_per_loop}|{}|{line_count}|{line_width_px}|{inner_radius_percent}|{opacity_percent}",
                normalized_color(color)
            ),
            Self::Sparkle {
                enabled,
                cycles_per_loop,
                color,
                count,
                size_px,
                opacity_percent,
            } => format!(
                "overlay:sparkle|{enabled}|{cycles_per_loop}|{}|{count}|{size_px}|{opacity_percent}",
                normalized_color(color)
            ),
            Self::ExpansionRing {
                enabled,
                cycles_per_loop,
                color,
                line_width_px,
                max_radius_percent,
                opacity_percent,
            } => format!(
                "overlay:expansion_ring|{enabled}|{cycles_per_loop}|{}|{line_width_px}|{max_radius_percent}|{opacity_percent}",
                normalized_color(color)
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MotionFrameContext {
    pub elapsed_ms: u64,
    pub total_duration_ms: u64,
}

impl MotionFrameContext {
    pub fn normalized_phase(self) -> AppResult<f64> {
        if self.total_duration_ms == 0 {
            return Err(AppError::new(
                "validation",
                "모션 전체 재생시간은 1ms 이상이어야 합니다.",
            ));
        }
        if self.elapsed_ms == self.total_duration_ms {
            return Ok(1.0);
        }
        Ok((self.elapsed_ms % self.total_duration_ms) as f64 / self.total_duration_ms as f64)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MotionFrameTiming {
    pub frame_index: usize,
    pub elapsed_ms: u64,
    pub duration_ms: u32,
    pub total_duration_ms: u64,
}

#[derive(Debug, Clone)]
pub struct MotionRenderResult {
    pub image: RgbaImage,
    pub clipped_pixel_count: u64,
}

pub fn parse_motion_recipe_json(value: &str) -> AppResult<MotionRecipe> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(MotionRecipe::default());
    }
    let recipe = serde_json::from_str::<MotionRecipe>(value).map_err(|error| {
        AppError::new(
            "validation",
            format!("저장된 모션 recipe를 읽을 수 없습니다: {error}"),
        )
    })?;
    validate_motion_recipe(&recipe)?;
    Ok(recipe)
}

pub fn motion_recipe_json(recipe: &MotionRecipe) -> AppResult<String> {
    validate_motion_recipe(recipe)?;
    serde_json::to_string(recipe).map_err(|error| {
        AppError::new(
            "validation",
            format!("모션 recipe를 저장 형식으로 만들 수 없습니다: {error}"),
        )
    })
}

pub fn validate_motion_recipe(recipe: &MotionRecipe) -> AppResult<()> {
    if recipe.version != MOTION_RECIPE_VERSION {
        return Err(AppError::new(
            "validation",
            format!(
                "지원하지 않는 모션 recipe 버전입니다. 현재 지원 버전은 {}입니다.",
                MOTION_RECIPE_VERSION
            ),
        ));
    }
    validate_i64_range(
        "모션 재생시간",
        recipe.duration_ms,
        MIN_MOTION_DURATION_MS,
        MAX_MOTION_DURATION_MS,
    )?;
    validate_i64_range("모션 FPS", recipe.fps, MIN_MOTION_FPS, MAX_MOTION_FPS)?;
    validate_static_frame_count(recipe)?;

    if let Some(spatial) = &recipe.spatial {
        validate_cycles(spatial.cycles_per_loop())?;
        match spatial {
            SpatialMotion::Shake {
                amplitude_x,
                amplitude_y,
                ..
            } => {
                validate_u32_range("흔들기 X 진폭", *amplitude_x, 0, 128)?;
                validate_u32_range("흔들기 Y 진폭", *amplitude_y, 0, 128)?;
            }
            SpatialMotion::Bounce { height_px, .. } => {
                validate_u32_range("통통 튀기 높이", *height_px, 0, 128)?;
            }
            SpatialMotion::Breathe { scale_percent, .. } => {
                validate_u32_range("호흡 크기", *scale_percent, 0, 50)?;
            }
            SpatialMotion::Rock { angle_degrees, .. } => {
                validate_u32_range("까딱 각도", *angle_degrees, 0, 45)?;
            }
            SpatialMotion::Spin { .. } => {}
        }
    }

    if let Some(displacement) = &recipe.displacement {
        validate_cycles(displacement.cycles_per_loop())?;
        match displacement {
            DisplacementMotion::Wave {
                amplitude_px,
                wavelength_px,
                ..
            } => {
                validate_u32_range("물결 진폭", *amplitude_px, 0, 128)?;
                validate_wavelength(*wavelength_px)?;
            }
            DisplacementMotion::Jelly {
                amplitude_x,
                amplitude_y,
                wavelength_x,
                wavelength_y,
                ..
            } => {
                validate_u32_range("젤리 X 진폭", *amplitude_x, 0, 128)?;
                validate_u32_range("젤리 Y 진폭", *amplitude_y, 0, 128)?;
                validate_wavelength(*wavelength_x)?;
                validate_wavelength(*wavelength_y)?;
            }
            DisplacementMotion::Ripple {
                amplitude_px,
                wavelength_px,
                center_x_percent,
                center_y_percent,
                ..
            } => {
                validate_u32_range("리플 진폭", *amplitude_px, 0, 128)?;
                validate_wavelength(*wavelength_px)?;
                validate_percent("리플 중심 X", *center_x_percent)?;
                validate_percent("리플 중심 Y", *center_y_percent)?;
            }
            DisplacementMotion::GlitchBands {
                amplitude_px,
                band_height_px,
                steps_per_cycle,
                ..
            } => {
                validate_u32_range("글리치 진폭", *amplitude_px, 0, 128)?;
                validate_u32_range("글리치 밴드 높이", *band_height_px, 1, 256)?;
                validate_u32_range("글리치 단계 수", *steps_per_cycle, 1, 32)?;
            }
        }
    }

    if let Some(color_opacity) = &recipe.color_opacity {
        validate_cycles(color_opacity.cycles_per_loop())?;
        match color_opacity {
            ColorOpacityMotion::HueCycle { range_degrees, .. } => {
                validate_u32_range("색상 순환 범위", *range_degrees, 0, 180)?;
            }
            ColorOpacityMotion::TintPulse {
                color,
                amount_percent,
                ..
            } => {
                let _ = parse_hex_rgba(color)?;
                validate_percent("색조 박동 강도", *amount_percent)?;
            }
            ColorOpacityMotion::BrightnessSaturationPulse {
                brightness_percent,
                saturation_percent,
                ..
            } => {
                validate_percent("밝기 박동 강도", *brightness_percent)?;
                validate_percent("채도 박동 강도", *saturation_percent)?;
            }
            ColorOpacityMotion::Flash {
                color,
                intensity_percent,
                ..
            } => {
                let _ = parse_hex_rgba(color)?;
                validate_percent("번쩍임 강도", *intensity_percent)?;
            }
        }
    }

    if let Some(overlay) = &recipe.overlay {
        validate_cycles(overlay.cycles_per_loop())?;
        match overlay {
            OverlayMotion::FocusLines {
                color,
                line_count,
                line_width_px,
                inner_radius_percent,
                opacity_percent,
                ..
            } => {
                let _ = parse_hex_rgba(color)?;
                validate_u32_range("집중선 개수", *line_count, 4, 64)?;
                validate_u32_range("집중선 두께", *line_width_px, 1, 16)?;
                validate_u32_range("집중선 안쪽 반경", *inner_radius_percent, 0, 90)?;
                validate_percent("집중선 불투명도", *opacity_percent)?;
            }
            OverlayMotion::Sparkle {
                color,
                count,
                size_px,
                opacity_percent,
                ..
            } => {
                let _ = parse_hex_rgba(color)?;
                validate_u32_range("반짝임 개수", *count, 1, 64)?;
                validate_u32_range("반짝임 크기", *size_px, 1, 32)?;
                validate_percent("반짝임 불투명도", *opacity_percent)?;
            }
            OverlayMotion::ExpansionRing {
                color,
                line_width_px,
                max_radius_percent,
                opacity_percent,
                ..
            } => {
                let _ = parse_hex_rgba(color)?;
                validate_u32_range("확산 링 두께", *line_width_px, 1, 16)?;
                validate_u32_range("확산 링 최대 반경", *max_radius_percent, 10, 100)?;
                validate_percent("확산 링 불투명도", *opacity_percent)?;
            }
        }
    }

    Ok(())
}

pub fn static_motion_schedule(recipe: &MotionRecipe) -> AppResult<Vec<MotionFrameTiming>> {
    validate_motion_recipe(recipe)?;
    let frame_count = derived_static_frame_count(recipe)?;
    let total_centiseconds = u64::try_from((recipe.duration_ms + 5) / 10)
        .unwrap_or(1)
        .max(1);
    let total_duration_ms = total_centiseconds.saturating_mul(10);
    let frame_count_u64 = u64::try_from(frame_count).unwrap_or(u64::MAX);
    let mut output = Vec::with_capacity(frame_count);
    let mut elapsed_centiseconds = 0_u64;

    for frame_index in 0..frame_count {
        let next_boundary = (u64::try_from(frame_index + 1)
            .unwrap_or(u64::MAX)
            .saturating_mul(total_centiseconds))
            / frame_count_u64;
        let duration_centiseconds = next_boundary.saturating_sub(elapsed_centiseconds);
        if duration_centiseconds == 0 {
            return Err(AppError::new(
                "validation",
                "현재 재생시간과 FPS 조합은 GIF 시간 단위로 표현할 수 없습니다.",
            ));
        }
        output.push(MotionFrameTiming {
            frame_index,
            elapsed_ms: elapsed_centiseconds.saturating_mul(10),
            duration_ms: u32::try_from(duration_centiseconds.saturating_mul(10))
                .unwrap_or(u32::MAX),
            total_duration_ms,
        });
        elapsed_centiseconds = next_boundary;
    }

    Ok(output)
}

pub fn apply_motion_recipe(
    source: &RgbaImage,
    recipe: &MotionRecipe,
    context: MotionFrameContext,
) -> AppResult<MotionRenderResult> {
    validate_motion_recipe(recipe)?;
    let phase = context.normalized_phase()?;
    if !recipe.has_enabled_motion() || source.width() == 0 || source.height() == 0 {
        return Ok(MotionRenderResult {
            image: source.clone(),
            clipped_pixel_count: 0,
        });
    }

    let mut image = source.clone();
    let mut clipped_pixel_count = 0_u64;

    if let Some(spatial) = recipe.spatial.as_ref().filter(|motion| motion.enabled()) {
        let result = apply_spatial(
            &image,
            spatial,
            recipe.seed,
            phase,
            recipe.interpolation,
            recipe.edge_mode,
        );
        image = result.image;
        clipped_pixel_count = clipped_pixel_count.saturating_add(result.clipped_pixel_count);
    }

    if let Some(displacement) = recipe
        .displacement
        .as_ref()
        .filter(|motion| motion.enabled())
    {
        let result = apply_displacement(
            &image,
            displacement,
            recipe.seed,
            phase,
            recipe.interpolation,
            recipe.edge_mode,
        );
        image = result.image;
        clipped_pixel_count = clipped_pixel_count.saturating_add(result.clipped_pixel_count);
    }

    if let Some(color_opacity) = recipe
        .color_opacity
        .as_ref()
        .filter(|motion| motion.enabled())
    {
        apply_color_opacity(&mut image, color_opacity, phase)?;
    }

    if let Some(overlay) = recipe.overlay.as_ref().filter(|motion| motion.enabled()) {
        apply_overlay(&mut image, overlay, recipe.seed, phase)?;
    }

    Ok(MotionRenderResult {
        image,
        clipped_pixel_count,
    })
}

fn validate_static_frame_count(recipe: &MotionRecipe) -> AppResult<()> {
    let frame_count = derived_static_frame_count(recipe)?;
    if frame_count < 2 {
        return Err(AppError::new(
            "validation",
            "모션 GIF는 최소 2프레임이 되도록 재생시간 또는 FPS를 높여 주세요.",
        ));
    }
    if i64::try_from(frame_count).unwrap_or(i64::MAX) > MAX_GIF_FRAMES {
        return Err(AppError::new(
            "validation",
            format!("모션 GIF는 최대 {MAX_GIF_FRAMES}프레임까지 만들 수 있습니다."),
        ));
    }
    Ok(())
}

fn derived_static_frame_count(recipe: &MotionRecipe) -> AppResult<usize> {
    let product = recipe
        .duration_ms
        .checked_mul(recipe.fps)
        .ok_or_else(|| AppError::new("validation", "모션 프레임 수가 너무 큽니다."))?;
    let rounded = product
        .checked_add(500)
        .ok_or_else(|| AppError::new("validation", "모션 프레임 수가 너무 큽니다."))?
        / 1_000;
    usize::try_from(rounded.max(0))
        .map_err(|_| AppError::new("validation", "모션 프레임 수가 너무 큽니다."))
}

fn validate_cycles(value: u32) -> AppResult<()> {
    validate_u32_range(
        "루프당 모션 주기",
        value,
        MIN_CYCLES_PER_LOOP,
        MAX_CYCLES_PER_LOOP,
    )
}

fn validate_wavelength(value: u32) -> AppResult<()> {
    validate_u32_range("변위 파장", value, 2, 1_024)
}

fn validate_percent(label: &str, value: u32) -> AppResult<()> {
    validate_u32_range(label, value, 0, 100)
}

fn validate_u32_range(label: &str, value: u32, min: u32, max: u32) -> AppResult<()> {
    if !(min..=max).contains(&value) {
        return Err(AppError::new(
            "validation",
            format!("{label} 값은 {min}~{max} 범위여야 합니다."),
        ));
    }
    Ok(())
}

fn validate_i64_range(label: &str, value: i64, min: i64, max: i64) -> AppResult<()> {
    if !(min..=max).contains(&value) {
        return Err(AppError::new(
            "validation",
            format!("{label} 값은 {min}~{max} 범위여야 합니다."),
        ));
    }
    Ok(())
}

fn apply_spatial(
    source: &RgbaImage,
    motion: &SpatialMotion,
    seed: u32,
    phase: f64,
    interpolation: MotionInterpolation,
    edge_mode: MotionEdgeMode,
) -> MotionRenderResult {
    let width = f64::from(source.width());
    let height = f64::from(source.height());
    let center_x = (width - 1.0) / 2.0;
    let center_y = (height - 1.0) / 2.0;
    let theta = TAU * f64::from(motion.cycles_per_loop()) * phase;

    match motion {
        SpatialMotion::Shake {
            amplitude_x,
            amplitude_y,
            ..
        } => {
            let phase_x = seeded_unit(seed, 0x1001) * TAU;
            let phase_y = seeded_unit(seed, 0x1002) * TAU;
            let dx = f64::from(*amplitude_x)
                * (0.7 * (theta + phase_x).sin() + 0.3 * (theta * 3.0 + phase_y).sin());
            let dy = f64::from(*amplitude_y)
                * (0.7 * (theta + phase_y).sin() + 0.3 * (theta * 2.0 + phase_x).sin());
            warp_image(source, interpolation, edge_mode, move |x, y| {
                (x - dx, y - dy)
            })
        }
        SpatialMotion::Bounce { height_px, .. } => {
            let dy = -f64::from(*height_px)
                * (PI * f64::from(motion.cycles_per_loop()) * phase)
                    .sin()
                    .abs();
            warp_image(source, interpolation, edge_mode, move |x, y| (x, y - dy))
        }
        SpatialMotion::Breathe { scale_percent, .. } => {
            let amount = f64::from(*scale_percent) / 100.0;
            let scale = 1.0 + amount * (0.5 - 0.5 * theta.cos());
            warp_image(source, interpolation, edge_mode, move |x, y| {
                (
                    center_x + (x - center_x) / scale,
                    center_y + (y - center_y) / scale,
                )
            })
        }
        SpatialMotion::Rock { angle_degrees, .. } => {
            let angle = f64::from(*angle_degrees).to_radians() * theta.sin();
            inverse_rotate(source, interpolation, edge_mode, center_x, center_y, angle)
        }
        SpatialMotion::Spin { clockwise, .. } => {
            let direction = if *clockwise { 1.0 } else { -1.0 };
            inverse_rotate(
                source,
                interpolation,
                edge_mode,
                center_x,
                center_y,
                direction * theta,
            )
        }
    }
}

fn inverse_rotate(
    source: &RgbaImage,
    interpolation: MotionInterpolation,
    edge_mode: MotionEdgeMode,
    center_x: f64,
    center_y: f64,
    angle: f64,
) -> MotionRenderResult {
    let cosine = angle.cos();
    let sine = angle.sin();
    warp_image(source, interpolation, edge_mode, move |x, y| {
        let dx = x - center_x;
        let dy = y - center_y;
        (
            center_x + cosine * dx + sine * dy,
            center_y - sine * dx + cosine * dy,
        )
    })
}

fn apply_displacement(
    source: &RgbaImage,
    motion: &DisplacementMotion,
    seed: u32,
    phase: f64,
    interpolation: MotionInterpolation,
    edge_mode: MotionEdgeMode,
) -> MotionRenderResult {
    let theta = TAU * f64::from(motion.cycles_per_loop()) * phase;
    match motion {
        DisplacementMotion::Wave {
            axis,
            amplitude_px,
            wavelength_px,
            ..
        } => {
            let amplitude = f64::from(*amplitude_px);
            let wavelength = f64::from(*wavelength_px);
            let axis = *axis;
            warp_image(source, interpolation, edge_mode, move |x, y| match axis {
                MotionAxis::Horizontal => (x - amplitude * (TAU * y / wavelength + theta).sin(), y),
                MotionAxis::Vertical => (x, y - amplitude * (TAU * x / wavelength + theta).sin()),
            })
        }
        DisplacementMotion::Jelly {
            amplitude_x,
            amplitude_y,
            wavelength_x,
            wavelength_y,
            ..
        } => {
            let amplitude_x = f64::from(*amplitude_x);
            let amplitude_y = f64::from(*amplitude_y);
            let wavelength_x = f64::from(*wavelength_x);
            let wavelength_y = f64::from(*wavelength_y);
            let offset = seeded_unit(seed, 0x2001) * TAU;
            warp_image(source, interpolation, edge_mode, move |x, y| {
                (
                    x - amplitude_x * (TAU * y / wavelength_y + theta).sin(),
                    y - amplitude_y * (TAU * x / wavelength_x + theta + offset).sin(),
                )
            })
        }
        DisplacementMotion::Ripple {
            amplitude_px,
            wavelength_px,
            center_x_percent,
            center_y_percent,
            ..
        } => {
            let amplitude = f64::from(*amplitude_px);
            let wavelength = f64::from(*wavelength_px);
            let center_x = (f64::from(source.width()) - 1.0) * f64::from(*center_x_percent) / 100.0;
            let center_y =
                (f64::from(source.height()) - 1.0) * f64::from(*center_y_percent) / 100.0;
            warp_image(source, interpolation, edge_mode, move |x, y| {
                let dx = x - center_x;
                let dy = y - center_y;
                let distance = (dx * dx + dy * dy).sqrt();
                if distance <= f64::EPSILON {
                    return (x, y);
                }
                let offset = amplitude * (TAU * distance / wavelength - theta).sin();
                (x - dx / distance * offset, y - dy / distance * offset)
            })
        }
        DisplacementMotion::GlitchBands {
            amplitude_px,
            band_height_px,
            steps_per_cycle,
            ..
        } => {
            let amplitude = f64::from(*amplitude_px);
            let band_height = f64::from(*band_height_px);
            let total_steps = u64::from(motion.cycles_per_loop()) * u64::from(*steps_per_cycle);
            let step = ((phase * total_steps as f64).floor() as u64) % total_steps.max(1);
            warp_image(source, interpolation, edge_mode, move |x, y| {
                let band = (y / band_height).floor().max(0.0) as u64;
                let offset =
                    seeded_signed(seed, 0x3000_u64 ^ band ^ step.rotate_left(17)) * amplitude;
                (x - offset, y)
            })
        }
    }
}

fn warp_image<F>(
    source: &RgbaImage,
    interpolation: MotionInterpolation,
    edge_mode: MotionEdgeMode,
    mut inverse_map: F,
) -> MotionRenderResult
where
    F: FnMut(f64, f64) -> (f64, f64),
{
    let width = source.width();
    let height = source.height();
    let mut output = RgbaImage::new(width, height);
    let mut clipped_pixel_count = 0_u64;
    let max_x = f64::from(width.saturating_sub(1));
    let max_y = f64::from(height.saturating_sub(1));

    for y in 0..height {
        for x in 0..width {
            let (source_x, source_y) = inverse_map(f64::from(x), f64::from(y));
            if source_x < 0.0 || source_y < 0.0 || source_x > max_x || source_y > max_y {
                clipped_pixel_count = clipped_pixel_count.saturating_add(1);
            }
            let pixel = sample_rgba(source, source_x, source_y, interpolation, edge_mode);
            output.put_pixel(x, y, pixel);
        }
    }

    MotionRenderResult {
        image: output,
        clipped_pixel_count,
    }
}

fn sample_rgba(
    source: &RgbaImage,
    x: f64,
    y: f64,
    interpolation: MotionInterpolation,
    edge_mode: MotionEdgeMode,
) -> Rgba<u8> {
    match interpolation {
        MotionInterpolation::Nearest => {
            sample_integer(source, x.round() as i64, y.round() as i64, edge_mode)
        }
        MotionInterpolation::Bilinear => {
            let x0 = x.floor() as i64;
            let y0 = y.floor() as i64;
            let x_fraction = x - x.floor();
            let y_fraction = y - y.floor();
            let samples = [
                (
                    sample_integer(source, x0, y0, edge_mode),
                    (1.0 - x_fraction) * (1.0 - y_fraction),
                ),
                (
                    sample_integer(source, x0 + 1, y0, edge_mode),
                    x_fraction * (1.0 - y_fraction),
                ),
                (
                    sample_integer(source, x0, y0 + 1, edge_mode),
                    (1.0 - x_fraction) * y_fraction,
                ),
                (
                    sample_integer(source, x0 + 1, y0 + 1, edge_mode),
                    x_fraction * y_fraction,
                ),
            ];
            premultiplied_weighted_pixel(&samples)
        }
    }
}

fn sample_integer(source: &RgbaImage, x: i64, y: i64, edge_mode: MotionEdgeMode) -> Rgba<u8> {
    let width = i64::from(source.width());
    let height = i64::from(source.height());
    if width <= 0 || height <= 0 {
        return Rgba([0, 0, 0, 0]);
    }

    let coordinates = match edge_mode {
        MotionEdgeMode::Transparent => {
            if x < 0 || y < 0 || x >= width || y >= height {
                return Rgba([0, 0, 0, 0]);
            }
            (x, y)
        }
        MotionEdgeMode::Clamp => (x.clamp(0, width - 1), y.clamp(0, height - 1)),
        MotionEdgeMode::Mirror => (mirror_index(x, width), mirror_index(y, height)),
    };

    *source.get_pixel(coordinates.0 as u32, coordinates.1 as u32)
}

fn mirror_index(value: i64, length: i64) -> i64 {
    if length <= 1 {
        return 0;
    }
    let period = length * 2 - 2;
    let normalized = value.rem_euclid(period);
    if normalized < length {
        normalized
    } else {
        period - normalized
    }
}

fn premultiplied_weighted_pixel(samples: &[(Rgba<u8>, f64); 4]) -> Rgba<u8> {
    let mut alpha = 0.0;
    let mut premultiplied = [0.0; 3];
    for (pixel, weight) in samples {
        if *weight <= 0.0 {
            continue;
        }
        let sample_alpha = f64::from(pixel[3]) / 255.0;
        alpha += sample_alpha * weight;
        for channel in 0..3 {
            premultiplied[channel] += f64::from(pixel[channel]) * sample_alpha * weight;
        }
    }
    if alpha <= f64::EPSILON {
        return Rgba([0, 0, 0, 0]);
    }
    Rgba([
        clamp_byte(premultiplied[0] / alpha),
        clamp_byte(premultiplied[1] / alpha),
        clamp_byte(premultiplied[2] / alpha),
        clamp_byte(alpha * 255.0),
    ])
}

fn apply_color_opacity(
    image: &mut RgbaImage,
    motion: &ColorOpacityMotion,
    phase: f64,
) -> AppResult<()> {
    let theta = TAU * f64::from(motion.cycles_per_loop()) * phase;
    let pulse = 0.5 - 0.5 * theta.cos();
    match motion {
        ColorOpacityMotion::HueCycle { range_degrees, .. } => {
            let shift = f64::from(*range_degrees) * theta.sin();
            for pixel in image.pixels_mut() {
                if pixel[3] == 0 {
                    continue;
                }
                let (hue, saturation, value) = rgb_to_hsv(pixel[0], pixel[1], pixel[2]);
                let rgb = hsv_to_rgb((hue + shift).rem_euclid(360.0), saturation, value);
                pixel[0] = rgb[0];
                pixel[1] = rgb[1];
                pixel[2] = rgb[2];
            }
        }
        ColorOpacityMotion::TintPulse {
            color,
            amount_percent,
            ..
        } => {
            let tint = parse_hex_rgba(color)?;
            let amount = f64::from(*amount_percent) / 100.0 * pulse * (f64::from(tint[3]) / 255.0);
            tint_existing_pixels(image, tint, amount);
        }
        ColorOpacityMotion::BrightnessSaturationPulse {
            brightness_percent,
            saturation_percent,
            ..
        } => {
            let brightness = f64::from(*brightness_percent) / 100.0 * pulse;
            let saturation_boost = f64::from(*saturation_percent) / 100.0 * pulse;
            for pixel in image.pixels_mut() {
                if pixel[3] == 0 {
                    continue;
                }
                let luminance = 0.2126 * f64::from(pixel[0])
                    + 0.7152 * f64::from(pixel[1])
                    + 0.0722 * f64::from(pixel[2]);
                for channel in 0..3 {
                    let saturated = luminance
                        + (f64::from(pixel[channel]) - luminance) * (1.0 + saturation_boost);
                    pixel[channel] = clamp_byte(saturated + 255.0 * brightness);
                }
            }
        }
        ColorOpacityMotion::Flash {
            color,
            intensity_percent,
            ..
        } => {
            let flash = parse_hex_rgba(color)?;
            let intensity = f64::from(*intensity_percent) / 100.0;
            let visible = 1.0 - intensity * (1.0 - pulse);
            let tint_amount = intensity * pulse * (f64::from(flash[3]) / 255.0);
            for pixel in image.pixels_mut() {
                if pixel[3] == 0 {
                    continue;
                }
                for channel in 0..3 {
                    pixel[channel] = clamp_byte(
                        f64::from(pixel[channel]) * (1.0 - tint_amount)
                            + f64::from(flash[channel]) * tint_amount,
                    );
                }
                pixel[3] = clamp_byte(f64::from(pixel[3]) * visible);
            }
        }
    }
    Ok(())
}

fn tint_existing_pixels(image: &mut RgbaImage, tint: [u8; 4], amount: f64) {
    let amount = amount.clamp(0.0, 1.0);
    for pixel in image.pixels_mut() {
        if pixel[3] == 0 {
            continue;
        }
        for channel in 0..3 {
            pixel[channel] = clamp_byte(
                f64::from(pixel[channel]) * (1.0 - amount) + f64::from(tint[channel]) * amount,
            );
        }
    }
}

fn apply_overlay(
    image: &mut RgbaImage,
    motion: &OverlayMotion,
    seed: u32,
    phase: f64,
) -> AppResult<()> {
    let theta = TAU * f64::from(motion.cycles_per_loop()) * phase;
    match motion {
        OverlayMotion::FocusLines {
            color,
            line_count,
            line_width_px,
            inner_radius_percent,
            opacity_percent,
            ..
        } => {
            let mut color = parse_hex_rgba(color)?;
            color[3] = multiplied_alpha(
                color[3],
                clamp_byte(
                    f64::from(*opacity_percent) / 100.0 * (0.65 + 0.35 * theta.sin()) * 255.0,
                ),
            );
            let center_x = (f64::from(image.width()) - 1.0) / 2.0;
            let center_y = (f64::from(image.height()) - 1.0) / 2.0;
            let half_min = f64::from(image.width().min(image.height())) / 2.0;
            let inner_radius = half_min * f64::from(*inner_radius_percent) / 100.0;
            let outer_radius = (f64::from(image.width()).hypot(f64::from(image.height()))) * 0.6;
            for index in 0..*line_count {
                let angle = TAU * f64::from(index) / f64::from(*line_count)
                    + theta / f64::from(*line_count);
                draw_thick_line(
                    image,
                    center_x + inner_radius * angle.cos(),
                    center_y + inner_radius * angle.sin(),
                    center_x + outer_radius * angle.cos(),
                    center_y + outer_radius * angle.sin(),
                    *line_width_px,
                    color,
                );
            }
        }
        OverlayMotion::Sparkle {
            color,
            count,
            size_px,
            opacity_percent,
            ..
        } => {
            let base_color = parse_hex_rgba(color)?;
            for index in 0..*count {
                let salt = 0x5000_u64 + u64::from(index) * 5;
                let x = seeded_unit(seed, salt) * f64::from(image.width().saturating_sub(1));
                let y = seeded_unit(seed, salt + 1) * f64::from(image.height().saturating_sub(1));
                let phase_offset = seeded_unit(seed, salt + 2);
                let sparkle = (TAU * (f64::from(motion.cycles_per_loop()) * phase + phase_offset))
                    .sin()
                    .max(0.0);
                let mut color = base_color;
                color[3] = multiplied_alpha(
                    color[3],
                    clamp_byte(sparkle * f64::from(*opacity_percent) / 100.0 * 255.0),
                );
                let radius = ((f64::from(*size_px) * (0.4 + 0.6 * sparkle)).round() as i32).max(1);
                draw_sparkle(image, x.round() as i32, y.round() as i32, radius, color);
            }
        }
        OverlayMotion::ExpansionRing {
            color,
            line_width_px,
            max_radius_percent,
            opacity_percent,
            ..
        } => {
            let progress = (phase * f64::from(motion.cycles_per_loop())).fract();
            let max_radius = f64::from(image.width().min(image.height())) / 2.0
                * f64::from(*max_radius_percent)
                / 100.0;
            let mut color = parse_hex_rgba(color)?;
            color[3] = multiplied_alpha(
                color[3],
                clamp_byte((1.0 - progress) * f64::from(*opacity_percent) / 100.0 * 255.0),
            );
            draw_ring(
                image,
                (f64::from(image.width()) - 1.0) / 2.0,
                (f64::from(image.height()) - 1.0) / 2.0,
                max_radius * progress,
                *line_width_px,
                color,
            );
        }
    }
    Ok(())
}

fn draw_thick_line(
    image: &mut RgbaImage,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    width: u32,
    color: [u8; 4],
) {
    let steps = ((x1 - x0).abs().max((y1 - y0).abs()).ceil() as u32).max(1);
    let radius = i32::try_from(width.saturating_sub(1) / 2).unwrap_or(i32::MAX);
    for step in 0..=steps {
        let progress = f64::from(step) / f64::from(steps);
        let x = (x0 + (x1 - x0) * progress).round() as i32;
        let y = (y0 + (y1 - y0) * progress).round() as i32;
        draw_disc(image, x, y, radius, color);
    }
}

fn draw_sparkle(image: &mut RgbaImage, x: i32, y: i32, radius: i32, color: [u8; 4]) {
    for offset in -radius..=radius {
        put_pixel_over(image, x + offset, y, color);
        put_pixel_over(image, x, y + offset, color);
        if offset.abs() <= radius / 2 {
            put_pixel_over(image, x + offset, y + offset, color);
            put_pixel_over(image, x + offset, y - offset, color);
        }
    }
}

fn draw_ring(
    image: &mut RgbaImage,
    center_x: f64,
    center_y: f64,
    radius: f64,
    width: u32,
    color: [u8; 4],
) {
    let outer = radius + f64::from(width) / 2.0;
    let inner = (radius - f64::from(width) / 2.0).max(0.0);
    let min_x = (center_x - outer).floor() as i32;
    let max_x = (center_x + outer).ceil() as i32;
    let min_y = (center_y - outer).floor() as i32;
    let max_y = (center_y + outer).ceil() as i32;
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let distance = (f64::from(x) - center_x).hypot(f64::from(y) - center_y);
            if distance >= inner && distance <= outer {
                put_pixel_over(image, x, y, color);
            }
        }
    }
}

fn draw_disc(image: &mut RgbaImage, center_x: i32, center_y: i32, radius: i32, color: [u8; 4]) {
    for y in -radius..=radius {
        for x in -radius..=radius {
            if x * x + y * y <= radius * radius {
                put_pixel_over(image, center_x + x, center_y + y, color);
            }
        }
    }
}

fn put_pixel_over(image: &mut RgbaImage, x: i32, y: i32, color: [u8; 4]) {
    if x < 0 || y < 0 || x >= image.width() as i32 || y >= image.height() as i32 {
        return;
    }
    let bottom = *image.get_pixel(x as u32, y as u32);
    image.put_pixel(x as u32, y as u32, alpha_over(bottom, Rgba(color)));
}

fn alpha_over(bottom: Rgba<u8>, top: Rgba<u8>) -> Rgba<u8> {
    let top_alpha = f64::from(top[3]) / 255.0;
    let bottom_alpha = f64::from(bottom[3]) / 255.0;
    let output_alpha = top_alpha + bottom_alpha * (1.0 - top_alpha);
    if output_alpha <= f64::EPSILON {
        return Rgba([0, 0, 0, 0]);
    }
    let mut output = [0_u8; 4];
    for channel in 0..3 {
        output[channel] = clamp_byte(
            (f64::from(top[channel]) * top_alpha
                + f64::from(bottom[channel]) * bottom_alpha * (1.0 - top_alpha))
                / output_alpha,
        );
    }
    output[3] = clamp_byte(output_alpha * 255.0);
    Rgba(output)
}

fn rgb_to_hsv(red: u8, green: u8, blue: u8) -> (f64, f64, f64) {
    let red = f64::from(red) / 255.0;
    let green = f64::from(green) / 255.0;
    let blue = f64::from(blue) / 255.0;
    let maximum = red.max(green).max(blue);
    let minimum = red.min(green).min(blue);
    let delta = maximum - minimum;
    let hue = if delta <= f64::EPSILON {
        0.0
    } else if (maximum - red).abs() <= f64::EPSILON {
        60.0 * ((green - blue) / delta).rem_euclid(6.0)
    } else if (maximum - green).abs() <= f64::EPSILON {
        60.0 * ((blue - red) / delta + 2.0)
    } else {
        60.0 * ((red - green) / delta + 4.0)
    };
    let saturation = if maximum <= f64::EPSILON {
        0.0
    } else {
        delta / maximum
    };
    (hue, saturation, maximum)
}

fn hsv_to_rgb(hue: f64, saturation: f64, value: f64) -> [u8; 3] {
    let chroma = value * saturation;
    let segment = hue / 60.0;
    let secondary = chroma * (1.0 - (segment.rem_euclid(2.0) - 1.0).abs());
    let (red, green, blue) = match segment.floor() as i32 {
        0 => (chroma, secondary, 0.0),
        1 => (secondary, chroma, 0.0),
        2 => (0.0, chroma, secondary),
        3 => (0.0, secondary, chroma),
        4 => (secondary, 0.0, chroma),
        _ => (chroma, 0.0, secondary),
    };
    let match_value = value - chroma;
    [
        clamp_byte((red + match_value) * 255.0),
        clamp_byte((green + match_value) * 255.0),
        clamp_byte((blue + match_value) * 255.0),
    ]
}

fn parse_hex_rgba(value: &str) -> AppResult<[u8; 4]> {
    let value = value.trim();
    let raw = value.strip_prefix('#').ok_or_else(invalid_motion_color)?;
    if raw.len() != 6 && raw.len() != 8 {
        return Err(invalid_motion_color());
    }
    let parse = |start: usize| {
        u8::from_str_radix(&raw[start..start + 2], 16).map_err(|_| invalid_motion_color())
    };
    Ok([
        parse(0)?,
        parse(2)?,
        parse(4)?,
        if raw.len() == 8 { parse(6)? } else { 255 },
    ])
}

fn invalid_motion_color() -> AppError {
    AppError::new(
        "validation",
        "모션 색상은 #RRGGBB 또는 #RRGGBBAA 형식이어야 합니다.",
    )
}

fn normalized_color(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn clamp_byte(value: f64) -> u8 {
    value.round().clamp(0.0, 255.0) as u8
}

fn multiplied_alpha(first: u8, second: u8) -> u8 {
    ((u16::from(first) * u16::from(second) + 127) / 255) as u8
}

fn seeded_unit(seed: u32, salt: u64) -> f64 {
    let mixed = splitmix64(u64::from(seed) ^ salt);
    (mixed >> 11) as f64 / ((1_u64 << 53) as f64)
}

fn seeded_signed(seed: u32, salt: u64) -> f64 {
    seeded_unit(seed, salt) * 2.0 - 1.0
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9E37_79B9_7F4A_7C15);
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

#[cfg(test)]
mod tests {
    use image::{Rgba, RgbaImage};

    use super::{
        apply_motion_recipe, motion_recipe_json, parse_motion_recipe_json, sample_rgba,
        static_motion_schedule, validate_motion_recipe, ColorOpacityMotion, DisplacementMotion,
        MotionAxis, MotionEdgeMode, MotionFrameContext, MotionInterpolation, MotionRecipe,
        OverlayMotion, SpatialMotion, MOTION_RECIPE_VERSION,
    };

    fn solid_image() -> RgbaImage {
        RgbaImage::from_fn(16, 12, |x, y| {
            Rgba([
                (x * 11) as u8,
                (y * 17) as u8,
                ((x + y) * 7) as u8,
                if x == 0 || y == 0 { 128 } else { 255 },
            ])
        })
    }

    fn context(elapsed_ms: u64) -> MotionFrameContext {
        MotionFrameContext {
            elapsed_ms,
            total_duration_ms: 1_000,
        }
    }

    #[test]
    fn default_recipe_roundtrips_and_rejects_unknown_fields() {
        let recipe = MotionRecipe::default();
        let json = motion_recipe_json(&recipe).unwrap();
        assert_eq!(parse_motion_recipe_json(&json).unwrap(), recipe);
        assert!(parse_motion_recipe_json(
            r#"{"version":1,"durationMs":1000,"fps":20,"seed":0,"interpolation":"bilinear","edgeMode":"transparent","spatial":null,"displacement":null,"colorOpacity":null,"overlay":null,"unknown":1}"#
        )
        .is_err());
    }

    #[test]
    fn recipe_validation_rejects_version_timing_cycles_and_parameters() {
        let mut recipe = MotionRecipe::default();
        recipe.version = MOTION_RECIPE_VERSION + 1;
        assert!(validate_motion_recipe(&recipe).is_err());

        recipe = MotionRecipe::default();
        recipe.duration_ms = 100;
        recipe.fps = 1;
        assert!(validate_motion_recipe(&recipe).is_err());

        recipe = MotionRecipe::default();
        recipe.spatial = Some(SpatialMotion::Rock {
            enabled: true,
            cycles_per_loop: 0,
            angle_degrees: 10,
        });
        assert!(validate_motion_recipe(&recipe).is_err());

        recipe.spatial = Some(SpatialMotion::Rock {
            enabled: true,
            cycles_per_loop: 1,
            angle_degrees: 46,
        });
        assert!(validate_motion_recipe(&recipe).is_err());
    }

    #[test]
    fn static_schedule_distributes_centiseconds_without_duration_drift() {
        let recipe = MotionRecipe {
            duration_ms: 1_030,
            fps: 24,
            ..MotionRecipe::default()
        };
        let schedule = static_motion_schedule(&recipe).unwrap();
        assert_eq!(schedule.len(), 25);
        assert_eq!(
            schedule
                .iter()
                .map(|frame| u64::from(frame.duration_ms))
                .sum::<u64>(),
            1_030
        );
        assert!(schedule.iter().all(|frame| frame.duration_ms >= 10));
        assert_eq!(schedule[0].elapsed_ms, 0);
        assert_eq!(schedule.last().unwrap().total_duration_ms, 1_030);
    }

    #[test]
    fn normalized_hash_is_stable_and_includes_timing_and_seed() {
        let recipe = MotionRecipe {
            seed: 7,
            spatial: Some(SpatialMotion::Shake {
                enabled: true,
                cycles_per_loop: 2,
                amplitude_x: 4,
                amplitude_y: 3,
            }),
            ..MotionRecipe::default()
        };
        assert_eq!(
            recipe.normalized_hash_parts().unwrap(),
            recipe.normalized_hash_parts().unwrap()
        );
        let mut changed = recipe.clone();
        changed.seed = 8;
        assert_ne!(
            recipe.normalized_hash_parts().unwrap(),
            changed.normalized_hash_parts().unwrap()
        );
    }

    #[test]
    fn renderer_is_deterministic_and_loop_phase_wraps_exactly() {
        let recipe = MotionRecipe {
            seed: 42,
            spatial: Some(SpatialMotion::Shake {
                enabled: true,
                cycles_per_loop: 2,
                amplitude_x: 3,
                amplitude_y: 2,
            }),
            displacement: Some(DisplacementMotion::GlitchBands {
                enabled: true,
                cycles_per_loop: 1,
                amplitude_px: 2,
                band_height_px: 3,
                steps_per_cycle: 4,
            }),
            ..MotionRecipe::default()
        };
        let source = solid_image();
        let first = apply_motion_recipe(&source, &recipe, context(250)).unwrap();
        let second = apply_motion_recipe(&source, &recipe, context(250)).unwrap();
        assert_eq!(first.image, second.image);
        assert_eq!(first.clipped_pixel_count, second.clipped_pixel_count);

        let at_zero = apply_motion_recipe(&source, &recipe, context(0)).unwrap();
        let at_loop = apply_motion_recipe(&source, &recipe, context(1_000)).unwrap();
        assert_eq!(at_zero.image, at_loop.image);
    }

    #[test]
    fn different_seed_changes_seeded_motion() {
        let mut first_recipe = MotionRecipe {
            seed: 1,
            spatial: Some(SpatialMotion::Shake {
                enabled: true,
                cycles_per_loop: 1,
                amplitude_x: 4,
                amplitude_y: 4,
            }),
            ..MotionRecipe::default()
        };
        let source = solid_image();
        let first = apply_motion_recipe(&source, &first_recipe, context(330))
            .unwrap()
            .image;
        first_recipe.seed = 2;
        let second = apply_motion_recipe(&source, &first_recipe, context(330))
            .unwrap()
            .image;
        assert_ne!(first, second);
    }

    #[test]
    fn bilinear_sampling_uses_premultiplied_alpha() {
        let mut source = RgbaImage::from_pixel(2, 1, Rgba([0, 0, 0, 0]));
        source.put_pixel(0, 0, Rgba([255, 255, 255, 255]));
        let sampled = sample_rgba(
            &source,
            0.5,
            0.0,
            MotionInterpolation::Bilinear,
            MotionEdgeMode::Transparent,
        );
        assert_eq!(&sampled.0[..3], &[255, 255, 255]);
        assert!((127..=128).contains(&sampled[3]));
    }

    #[test]
    fn edge_modes_are_bounded_and_distinct() {
        let mut source = RgbaImage::new(2, 1);
        source.put_pixel(0, 0, Rgba([10, 0, 0, 255]));
        source.put_pixel(1, 0, Rgba([20, 0, 0, 255]));
        assert_eq!(
            sample_rgba(
                &source,
                -1.0,
                0.0,
                MotionInterpolation::Nearest,
                MotionEdgeMode::Transparent
            ),
            Rgba([0, 0, 0, 0])
        );
        assert_eq!(
            sample_rgba(
                &source,
                -1.0,
                0.0,
                MotionInterpolation::Nearest,
                MotionEdgeMode::Clamp
            ),
            Rgba([10, 0, 0, 255])
        );
        assert_eq!(
            sample_rgba(
                &source,
                -1.0,
                0.0,
                MotionInterpolation::Nearest,
                MotionEdgeMode::Mirror
            ),
            Rgba([20, 0, 0, 255])
        );
    }

    #[test]
    fn all_spatial_and_displacement_presets_render_at_original_size() {
        let spatials = vec![
            SpatialMotion::Shake {
                enabled: true,
                cycles_per_loop: 1,
                amplitude_x: 2,
                amplitude_y: 2,
            },
            SpatialMotion::Bounce {
                enabled: true,
                cycles_per_loop: 1,
                height_px: 3,
            },
            SpatialMotion::Breathe {
                enabled: true,
                cycles_per_loop: 1,
                scale_percent: 10,
            },
            SpatialMotion::Rock {
                enabled: true,
                cycles_per_loop: 1,
                angle_degrees: 10,
            },
            SpatialMotion::Spin {
                enabled: true,
                cycles_per_loop: 1,
                clockwise: true,
            },
        ];
        let displacements = vec![
            DisplacementMotion::Wave {
                enabled: true,
                cycles_per_loop: 1,
                axis: MotionAxis::Horizontal,
                amplitude_px: 2,
                wavelength_px: 8,
            },
            DisplacementMotion::Jelly {
                enabled: true,
                cycles_per_loop: 1,
                amplitude_x: 2,
                amplitude_y: 2,
                wavelength_x: 8,
                wavelength_y: 9,
            },
            DisplacementMotion::Ripple {
                enabled: true,
                cycles_per_loop: 1,
                amplitude_px: 2,
                wavelength_px: 8,
                center_x_percent: 50,
                center_y_percent: 50,
            },
            DisplacementMotion::GlitchBands {
                enabled: true,
                cycles_per_loop: 1,
                amplitude_px: 2,
                band_height_px: 2,
                steps_per_cycle: 4,
            },
        ];
        let source = solid_image();
        for spatial in spatials {
            let recipe = MotionRecipe {
                spatial: Some(spatial),
                ..MotionRecipe::default()
            };
            assert_eq!(
                apply_motion_recipe(&source, &recipe, context(250))
                    .unwrap()
                    .image
                    .dimensions(),
                source.dimensions()
            );
        }
        for displacement in displacements {
            let recipe = MotionRecipe {
                displacement: Some(displacement),
                ..MotionRecipe::default()
            };
            assert_eq!(
                apply_motion_recipe(&source, &recipe, context(250))
                    .unwrap()
                    .image
                    .dimensions(),
                source.dimensions()
            );
        }
    }

    #[test]
    fn all_color_and_overlay_presets_render() {
        let colors = vec![
            ColorOpacityMotion::HueCycle {
                enabled: true,
                cycles_per_loop: 1,
                range_degrees: 120,
            },
            ColorOpacityMotion::TintPulse {
                enabled: true,
                cycles_per_loop: 1,
                color: "#FF0000".to_string(),
                amount_percent: 60,
            },
            ColorOpacityMotion::BrightnessSaturationPulse {
                enabled: true,
                cycles_per_loop: 1,
                brightness_percent: 20,
                saturation_percent: 30,
            },
            ColorOpacityMotion::Flash {
                enabled: true,
                cycles_per_loop: 1,
                color: "#FFFFFF".to_string(),
                intensity_percent: 50,
            },
        ];
        let overlays = vec![
            OverlayMotion::FocusLines {
                enabled: true,
                cycles_per_loop: 1,
                color: "#FFFFFFFF".to_string(),
                line_count: 8,
                line_width_px: 1,
                inner_radius_percent: 40,
                opacity_percent: 80,
            },
            OverlayMotion::Sparkle {
                enabled: true,
                cycles_per_loop: 1,
                color: "#FFFFFFFF".to_string(),
                count: 4,
                size_px: 2,
                opacity_percent: 80,
            },
            OverlayMotion::ExpansionRing {
                enabled: true,
                cycles_per_loop: 1,
                color: "#FFFFFFFF".to_string(),
                line_width_px: 1,
                max_radius_percent: 90,
                opacity_percent: 80,
            },
        ];
        let source = solid_image();
        for color_opacity in colors {
            let recipe = MotionRecipe {
                color_opacity: Some(color_opacity),
                ..MotionRecipe::default()
            };
            let result = apply_motion_recipe(&source, &recipe, context(500)).unwrap();
            assert_eq!(result.image.dimensions(), source.dimensions());
        }
        for overlay in overlays {
            let recipe = MotionRecipe {
                overlay: Some(overlay),
                ..MotionRecipe::default()
            };
            let result = apply_motion_recipe(&source, &recipe, context(500)).unwrap();
            assert_eq!(result.image.dimensions(), source.dimensions());
        }
    }

    #[test]
    fn empty_or_disabled_recipe_is_an_exact_noop() {
        let source = solid_image();
        let empty = apply_motion_recipe(&source, &MotionRecipe::default(), context(200)).unwrap();
        assert_eq!(empty.image, source);
        assert_eq!(empty.clipped_pixel_count, 0);

        let disabled = MotionRecipe {
            overlay: Some(OverlayMotion::Sparkle {
                enabled: false,
                cycles_per_loop: 1,
                color: "#FFFFFF".to_string(),
                count: 4,
                size_px: 2,
                opacity_percent: 100,
            }),
            ..MotionRecipe::default()
        };
        assert_eq!(
            apply_motion_recipe(&source, &disabled, context(200))
                .unwrap()
                .image,
            source
        );
    }
}
