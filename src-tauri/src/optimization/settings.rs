use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OptimizationPreset {
    Quality,
    Balanced,
    Smallest,
}

impl OptimizationPreset {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Quality => "quality",
            Self::Balanced => "balanced",
            Self::Smallest => "smallest",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GifCandidateSettings {
    pub preset: String,
    pub frame_step: usize,
    pub encoder_speed: i32,
    pub color_limit: Option<i64>,
    pub fps_limit: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StaticCandidateSettings {
    pub preset: String,
    pub quality: Option<i64>,
    pub strategy: String,
}

pub fn gif_settings_for_preset(
    preset: OptimizationPreset,
    frame_count: usize,
) -> GifCandidateSettings {
    match preset {
        OptimizationPreset::Quality => GifCandidateSettings {
            preset: preset.as_str().to_string(),
            frame_step: 1,
            encoder_speed: 10,
            color_limit: Some(256),
            fps_limit: None,
        },
        OptimizationPreset::Balanced => GifCandidateSettings {
            preset: preset.as_str().to_string(),
            frame_step: if frame_count > 3 { 2 } else { 1 },
            encoder_speed: 20,
            color_limit: Some(128),
            fps_limit: Some(15),
        },
        OptimizationPreset::Smallest => GifCandidateSettings {
            preset: preset.as_str().to_string(),
            frame_step: if frame_count > 8 {
                4
            } else if frame_count > 4 {
                3
            } else {
                2
            },
            encoder_speed: 30,
            color_limit: Some(64),
            fps_limit: Some(10),
        },
    }
}

pub fn static_settings_for_format(
    preset: OptimizationPreset,
    format: &str,
) -> StaticCandidateSettings {
    let quality = match (format, preset) {
        ("jpg", OptimizationPreset::Quality) => Some(92),
        ("jpg", OptimizationPreset::Balanced) => Some(82),
        ("jpg", OptimizationPreset::Smallest) => Some(68),
        _ => None,
    };
    StaticCandidateSettings {
        preset: preset.as_str().to_string(),
        quality,
        strategy: if format == "jpg" {
            "jpeg_quality_ladder".to_string()
        } else {
            "png_reencode_preserve_alpha".to_string()
        },
    }
}
