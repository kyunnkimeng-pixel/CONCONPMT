use serde::{Deserialize, Serialize};

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
    pub shape: String,
    pub order_index: i64,
    pub cell_width_override: Option<i64>,
    pub cell_height_override: Option<i64>,
    pub thumbnail_url: Option<String>,
    pub current_preview_url: Option<String>,
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
    pub gif_loop_mode: String,
    pub gif_loop_count: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportImageFilePayload {
    pub original_filename: String,
    pub bytes: Vec<u8>,
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
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportValidationIssueDto {
    pub severity: String,
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
    pub display_name: String,
    pub alt_text: String,
    pub output_format: String,
    pub width: i64,
    pub height: i64,
    pub byte_size: Option<i64>,
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
}
