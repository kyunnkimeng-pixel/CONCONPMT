use std::cmp::Ordering;

use crate::db::repositories::optimization::ProcessedAssetVariantRecord;

#[cfg_attr(not(test), allow(dead_code))]
pub fn sort_best_candidates(candidates: &mut [ProcessedAssetVariantRecord], target_max_bytes: i64) {
    candidates.sort_by(|left, right| {
        let left_score = score_candidate(left, target_max_bytes);
        let right_score = score_candidate(right, target_max_bytes);
        right_score
            .partial_cmp(&left_score)
            .unwrap_or(Ordering::Equal)
    });
}

#[cfg_attr(not(test), allow(dead_code))]
fn score_candidate(candidate: &ProcessedAssetVariantRecord, target_max_bytes: i64) -> f64 {
    let passes = candidate.byte_size <= target_max_bytes;
    let preset_penalty = match candidate.preset.as_deref() {
        Some("quality") => 0.0,
        Some("balanced") => 1.5,
        Some("smallest") => 3.0,
        _ => 2.0,
    };
    let size_ratio = candidate.byte_size as f64 / target_max_bytes.max(1) as f64;
    let pass_score = if passes { 100.0 } else { 0.0 };
    let margin_score = if passes {
        (1.0 - (1.0 - size_ratio).abs()).max(0.0) * 20.0
    } else {
        -size_ratio * 20.0
    };

    pass_score + margin_score - preset_penalty
}

#[cfg(test)]
mod tests {
    use super::sort_best_candidates;
    use crate::db::repositories::optimization::ProcessedAssetVariantRecord;

    #[test]
    fn scoring_prefers_less_damaged_passing_candidate() {
        let mut candidates = vec![
            record("smallest", 500_000),
            record("quality", 1_900_000),
            record("balanced", 1_300_000),
        ];

        sort_best_candidates(&mut candidates, 2_000_000);

        assert_eq!(candidates[0].preset.as_deref(), Some("quality"));
    }

    fn record(preset: &str, byte_size: i64) -> ProcessedAssetVariantRecord {
        ProcessedAssetVariantRecord {
            id: preset.to_string(),
            icon_id: "icon".to_string(),
            piece_id: Some("piece".to_string()),
            profile_id: Some("profile".to_string()),
            source_file_id: Some("source".to_string()),
            kind: "optimized_gif".to_string(),
            preset: Some(preset.to_string()),
            path: "candidate.gif".to_string(),
            format: "gif".to_string(),
            width: 100,
            height: 100,
            byte_size,
            frame_count: Some(10),
            duration_ms: Some(1_000),
            loop_mode: Some("infinite".to_string()),
            settings_json: "{}".to_string(),
            source_hash: "source".to_string(),
            crop_hash: "crop".to_string(),
            profile_hash: "profile".to_string(),
            settings_hash: preset.to_string(),
            is_active_for_export: false,
        }
    }
}
