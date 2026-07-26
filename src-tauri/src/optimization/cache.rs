use sha2::{Digest, Sha256};

use crate::error::AppResult;
use crate::imaging::effects::EffectRecipe;
use crate::imaging::export_render::ExportCropRect;
use crate::imaging::motion::MotionRecipe;
use crate::imaging::text_overlay::TextOverlayRenderSpec;
use crate::imaging::transform::ImageTransform;

pub fn hash_text(parts: &[String]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part.as_bytes());
        hasher.update([0]);
    }
    let digest = hasher.finalize();
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn render_recipe_crop_hash(
    shape: &str,
    crop: &ExportCropRect,
    cell_width: i64,
    cell_height: i64,
    piece_index: usize,
    transform: ImageTransform,
    gif_loop_mode: &str,
    gif_loop_count: Option<i64>,
    text_overlay: Option<&TextOverlayRenderSpec>,
    effects: &EffectRecipe,
    motion: &MotionRecipe,
) -> AppResult<String> {
    let mut parts = vec![
        "render_recipe_v4".to_string(),
        shape.to_string(),
        format!("{:.3}", crop.x),
        format!("{:.3}", crop.y),
        format!("{:.3}", crop.width),
        format!("{:.3}", crop.height),
        cell_width.to_string(),
        cell_height.to_string(),
        piece_index.to_string(),
        transform.quarter_turns.to_string(),
        transform.flip_horizontal.to_string(),
        transform.flip_vertical.to_string(),
        gif_loop_mode.to_string(),
        gif_loop_count.unwrap_or_default().to_string(),
    ];
    if let Some(text_overlay) = text_overlay {
        parts.extend(text_overlay.normalized_hash_parts());
    }
    parts.extend(effects.normalized_hash_parts()?);
    parts.extend(motion.normalized_hash_parts()?);
    Ok(hash_text(&parts))
}
