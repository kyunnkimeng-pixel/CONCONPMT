use serde::{Deserialize, Serialize};

use crate::imaging::effects::EffectRecipe;
use crate::imaging::motion::MotionRecipe;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionDto {
    pub id: String,
    pub name: String,
    pub cover_source_file_id: Option<String>,
    pub cover_icon_id: Option<String>,
    pub cover_image_url: Option<String>,
    pub icon_count: i64,
    pub default_cell_width: i64,
    pub default_cell_height: i64,
    pub preview_width: i64,
    pub preview_height: i64,
    pub export_format: String,
    pub max_bytes: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IconDto {
    pub id: String,
    pub collection_id: String,
    pub source_file_id: String,
    pub display_name: String,
    pub note: Option<String>,
    pub icon_kind: String,
    pub readiness: String,
    pub placeholder_text: Option<String>,
    pub shape: String,
    pub order_index: i64,
    pub cell_width_override: Option<i64>,
    pub cell_height_override: Option<i64>,
    pub thumbnail_url: Option<String>,
    pub thumbnail_override_url: Option<String>,
    pub current_preview_url: Option<String>,
    pub transform_quarter_turns: i64,
    pub transform_flip_horizontal: bool,
    pub transform_flip_vertical: bool,
    pub gif_loop_mode: String,
    pub gif_loop_count: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
    pub pieces: Vec<IconPieceDto>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IconPieceDto {
    pub id: String,
    pub icon_id: String,
    pub piece_index: i64,
    pub piece_role: String,
    pub alt_text: String,
    pub generated_preview_url: Option<String>,
    pub last_export_url: Option<String>,
    pub export_status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceFileDto {
    pub id: String,
    pub original_filename: String,
    pub original_image_url: String,
    pub mime_type: String,
    pub width: i64,
    pub height: i64,
    pub byte_size: i64,
    pub is_animated: bool,
    pub frame_count: Option<i64>,
    pub original_loop_mode: String,
    pub original_loop_count: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CropSettingsDto {
    pub crop_mode: String,
    pub crop_x: f64,
    pub crop_y: f64,
    pub crop_w: f64,
    pub crop_h: f64,
    pub preset_position: String,
    pub source_width_at_apply: Option<i64>,
    pub source_height_at_apply: Option<i64>,
    pub viewport_width_at_apply: i64,
    pub viewport_height_at_apply: i64,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IconEditorStateDto {
    pub icon: IconDto,
    pub source: SourceFileDto,
    pub crop: CropSettingsDto,
    pub text_overlay: TextOverlayDto,
    pub effect_recipe: EffectRecipe,
    pub effect_revision: i64,
    pub motion_recipe: MotionRecipe,
    pub motion_revision: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextOverlayDto {
    pub enabled: bool,
    pub text: String,
    pub font_path: Option<String>,
    pub font_size: f64,
    pub x: f64,
    pub y: f64,
    pub color: String,
    pub stroke_color: String,
    pub stroke_width: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyIconCropPayload {
    pub icon_id: String,
    pub shape: String,
    pub crop_mode: String,
    pub crop_x: f64,
    pub crop_y: f64,
    pub crop_w: f64,
    pub crop_h: f64,
    pub preset_position: String,
    pub cell_width: i64,
    pub cell_height: i64,
    #[serde(default)]
    pub transform_quarter_turns: i64,
    #[serde(default)]
    pub transform_flip_horizontal: bool,
    #[serde(default)]
    pub transform_flip_vertical: bool,
    #[serde(default)]
    pub piece_ids: Vec<String>,
    pub gif_loop_mode: String,
    pub gif_loop_count: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateIconTextOverlayPayload {
    pub icon_id: String,
    pub enabled: bool,
    pub text: String,
    pub font_path: Option<String>,
    pub font_size: f64,
    pub x: f64,
    pub y: f64,
    pub color: String,
    pub stroke_color: String,
    pub stroke_width: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewIconEffectsPayload {
    pub icon_id: String,
    pub recipe: EffectRecipe,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateIconEffectsPayload {
    pub icon_id: String,
    pub expected_revision: i64,
    pub recipe: EffectRecipe,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectPreviewDto {
    pub preview_path: String,
    pub byte_size: i64,
    pub max_piece_byte_size: i64,
    pub max_bytes: i64,
    pub frame_count: i64,
    pub processing_ms: i64,
    pub warnings: Vec<String>,
    pub recipe_signature: String,
    pub generated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewIconMotionPayload {
    pub icon_id: String,
    pub recipe: MotionRecipe,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateIconMotionPayload {
    pub icon_id: String,
    pub expected_revision: i64,
    pub expected_render_signature: String,
    pub recipe: MotionRecipe,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MotionPreviewDto {
    pub preview_path: String,
    pub poster_path: String,
    pub byte_size: i64,
    pub piece_byte_sizes: Vec<i64>,
    pub max_piece_byte_size: i64,
    pub max_bytes: i64,
    pub passes_byte_limit: bool,
    pub frame_count: i64,
    pub duration_ms: i64,
    pub effective_fps: f64,
    pub timing_source: String,
    pub loop_mode: String,
    pub loop_count: Option<i64>,
    pub clipped: bool,
    pub clipped_frame_count: i64,
    pub processing_ms: i64,
    pub warnings: Vec<String>,
    pub render_signature: String,
    pub generated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportImageFilePayload {
    pub original_filename: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePlaceholderIconPayload {
    pub label: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCollectionSettingsPayload {
    pub default_cell_width: i64,
    pub default_cell_height: i64,
    pub preview_width: i64,
    pub preview_height: i64,
    pub export_format: String,
    pub max_bytes: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettingsDto {
    pub last_open_collection_id: Option<String>,
    pub last_view_mode: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveAppSettingsPayload {
    pub last_open_collection_id: Option<String>,
    pub last_view_mode: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryCleanupResultDto {
    pub orphaned_source_files: i64,
    pub removed_original_files: i64,
    pub removed_thumbnail_files: i64,
    pub removed_temp_files: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportImagesResultDto {
    pub collection: CollectionDto,
    pub imported_icons: Vec<IconDto>,
    pub rejected_files: Vec<RejectedImportFileDto>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RejectedImportFileDto {
    pub original_filename: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportProfileDto {
    pub id: String,
    pub collection_id: String,
    pub name: String,
    pub profile_type: String,
    pub target_format: String,
    pub target_cell_width: i64,
    pub target_cell_height: i64,
    pub preview_width: i64,
    pub preview_height: i64,
    pub max_bytes: i64,
    pub allowed_formats: Vec<String>,
    pub filename_mode: String,
    pub include_alt_txt: bool,
    pub strict_warnings: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequestPayload {
    pub profile_id: String,
    pub target_format: String,
    pub target_cell_width: i64,
    pub target_cell_height: i64,
    pub max_bytes: i64,
    pub filename_mode: String,
    pub include_alt_txt: bool,
    pub strict_warnings: bool,
    pub output_directory: Option<String>,
    pub open_folder_after_export: bool,
    pub open_alt_txt_after_export: bool,
    #[serde(default)]
    pub excluded_piece_ids: Vec<String>,
    #[serde(default = "default_resize_filter")]
    pub resize_filter: String,
}

fn default_resize_filter() -> String {
    "lanczos3".to_string()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportValidationIssueDto {
    pub severity: String,
    pub blocking: bool,
    pub code: String,
    pub message: String,
    pub piece_id: Option<String>,
    pub icon_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportPlanItemDto {
    pub export_index: i64,
    pub file_name: String,
    pub icon_id: String,
    pub piece_id: String,
    pub piece_role: String,
    pub display_name: String,
    pub alt_text: String,
    pub output_format: String,
    pub width: i64,
    pub height: i64,
    pub byte_size: Option<i64>,
    pub limit_bytes: i64,
    pub included: bool,
    pub is_animated: bool,
    pub source_preview_url: Option<String>,
    pub export_path: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportValidationResultDto {
    pub can_export: bool,
    pub profile: ExportProfileDto,
    pub output_count: i64,
    pub errors: Vec<ExportValidationIssueDto>,
    pub warnings: Vec<ExportValidationIssueDto>,
    pub items: Vec<ExportPlanItemDto>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportCollectionResultDto {
    pub validation: ExportValidationResultDto,
    pub export_directory: Option<String>,
    pub alt_txt_path: Option<String>,
    pub manifest_path: Option<String>,
    pub report_txt_path: Option<String>,
    pub report_json_path: Option<String>,
    pub issues_csv_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct OptimizationAdvancedSettingsPayload {
    pub target_max_bytes: Option<i64>,
    pub safety_margin_percent: Option<f64>,
    pub fps_limit: Option<i64>,
    pub playback_fps: Option<i64>,
    pub frame_step: Option<i64>,
    pub color_limit: Option<i64>,
    pub jpeg_quality: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportAssetAnalysisDto {
    pub icon_id: String,
    pub profile_id: String,
    pub piece_id: String,
    pub baseline_variant_id: String,
    pub baseline_bytes: i64,
    pub target_max_bytes: i64,
    pub over_by_bytes: i64,
    pub over_ratio: f64,
    pub format: String,
    pub width: i64,
    pub height: i64,
    pub frame_count: Option<i64>,
    pub duration_ms: Option<i64>,
    pub average_fps: Option<f64>,
    pub loop_mode: Option<String>,
    pub has_transparency: Option<bool>,
    pub status: String,
    pub explanation_for_user: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OptimizationCandidateDto {
    pub id: String,
    pub icon_id: String,
    pub profile_id: String,
    pub piece_id: String,
    pub preset: String,
    pub path: String,
    pub preview_url: String,
    pub format: String,
    pub measured_byte_size: i64,
    pub target_max_bytes: i64,
    pub passes: bool,
    pub width: i64,
    pub height: i64,
    pub frame_count: Option<i64>,
    pub original_frame_count: Option<i64>,
    pub duration_ms: Option<i64>,
    pub original_duration_ms: Option<i64>,
    pub loop_mode: Option<String>,
    pub color_limit: Option<i64>,
    pub fps_limit: Option<i64>,
    pub quality: Option<i64>,
    pub quality_impact: String,
    pub settings_json: String,
    pub summary: String,
    pub is_active_for_export: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OptimizationResultDto {
    pub analysis: ExportAssetAnalysisDto,
    pub candidates: Vec<OptimizationCandidateDto>,
    pub already_passes: bool,
    pub fallback_suggestions: Vec<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyOptimizationResultDto {
    pub candidate: OptimizationCandidateDto,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GifPlaybackPreviewResultDto {
    pub preview_path: String,
    pub playback_fps: i64,
    pub generated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClearOptimizationResultDto {
    pub icon_id: String,
    pub profile_id: String,
    pub piece_id: Option<String>,
    pub cleared_count: i64,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveVariantDto {
    pub candidate: OptimizationCandidateDto,
    pub stale: bool,
}
