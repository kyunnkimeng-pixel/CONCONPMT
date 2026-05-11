use tauri::State;

use crate::app_state::AppState;
use crate::db::repositories::export_profiles as export_profile_repository;
use crate::error::AppResult;
use crate::models::{
    ActiveVariantDto, ApplyOptimizationResultDto, ClearOptimizationResultDto,
    ExportAssetAnalysisDto, ExportCollectionResultDto, ExportProfileDto, ExportRequestPayload,
    ExportValidationResultDto, OptimizationAdvancedSettingsPayload, OptimizationCandidateDto,
    OptimizationResultDto,
};

#[tauri::command]
pub fn list_export_profiles(
    state: State<'_, AppState>,
    collection_id: String,
) -> AppResult<Vec<ExportProfileDto>> {
    let connection = state.connection()?;
    export_profile_repository::list_export_profiles(&connection, &collection_id)
}

#[tauri::command]
pub fn save_export_profile_settings(
    state: State<'_, AppState>,
    collection_id: String,
    payload: ExportRequestPayload,
) -> AppResult<ExportProfileDto> {
    let connection = state.connection()?;
    export_profile_repository::update_export_profile_settings(&connection, &collection_id, &payload)
}

#[tauri::command]
pub fn validate_export_collection(
    state: State<'_, AppState>,
    collection_id: String,
    payload: ExportRequestPayload,
) -> AppResult<ExportValidationResultDto> {
    let paths = state.paths().clone();
    let connection = state.connection()?;
    crate::export::validate_export_collection(&connection, &paths, &collection_id, &payload)
}

#[tauri::command]
pub fn export_collection(
    state: State<'_, AppState>,
    collection_id: String,
    payload: ExportRequestPayload,
) -> AppResult<ExportCollectionResultDto> {
    let paths = state.paths().clone();
    let mut connection = state.connection()?;
    crate::export::export_collection(&mut connection, &paths, &collection_id, &payload)
}

#[tauri::command]
pub fn open_export_path(_state: State<'_, AppState>, path: String) -> AppResult<()> {
    crate::export::open_export_path(&path)
}

#[tauri::command]
pub fn pick_export_directory(initial_directory: Option<String>) -> AppResult<Option<String>> {
    let mut dialog = rfd::FileDialog::new().set_title("내보내기 폴더 선택");
    if let Some(path) = initial_directory {
        if !path.trim().is_empty() {
            dialog = dialog.set_directory(path);
        }
    }

    Ok(dialog
        .pick_folder()
        .map(|path| path.to_string_lossy().to_string()))
}

#[tauri::command]
pub fn analyze_export_asset_candidate(
    state: State<'_, AppState>,
    icon_id: String,
    profile_id: String,
    piece_id: Option<String>,
) -> AppResult<ExportAssetAnalysisDto> {
    let paths = state.paths().clone();
    let connection = state.connection()?;
    crate::optimization::analyze_export_asset_candidate(
        &connection,
        &paths,
        &icon_id,
        &profile_id,
        piece_id.as_deref(),
    )
}

#[tauri::command]
pub fn generate_gif_optimization_candidates(
    state: State<'_, AppState>,
    icon_id: String,
    profile_id: String,
    piece_id: Option<String>,
    mode: Option<String>,
    advanced_settings: Option<OptimizationAdvancedSettingsPayload>,
) -> AppResult<OptimizationResultDto> {
    let paths = state.paths().clone();
    let connection = state.connection()?;
    crate::optimization::generate_gif_optimization_candidates(
        &connection,
        &paths,
        &icon_id,
        &profile_id,
        piece_id.as_deref(),
        mode,
        advanced_settings,
    )
}

#[tauri::command]
pub fn generate_static_optimization_candidates(
    state: State<'_, AppState>,
    icon_id: String,
    profile_id: String,
    piece_id: Option<String>,
    mode: Option<String>,
    advanced_settings: Option<OptimizationAdvancedSettingsPayload>,
) -> AppResult<OptimizationResultDto> {
    let paths = state.paths().clone();
    let connection = state.connection()?;
    crate::optimization::generate_static_optimization_candidates(
        &connection,
        &paths,
        &icon_id,
        &profile_id,
        piece_id.as_deref(),
        mode,
        advanced_settings,
    )
}

#[tauri::command]
pub fn list_optimization_candidates(
    state: State<'_, AppState>,
    icon_id: String,
    profile_id: String,
    piece_id: Option<String>,
) -> AppResult<Vec<OptimizationCandidateDto>> {
    let connection = state.connection()?;
    crate::optimization::list_optimization_candidates(
        &connection,
        &icon_id,
        &profile_id,
        piece_id.as_deref(),
    )
}

#[tauri::command]
pub fn apply_optimization_candidate(
    state: State<'_, AppState>,
    candidate_id: String,
) -> AppResult<ApplyOptimizationResultDto> {
    let connection = state.connection()?;
    crate::optimization::apply_optimization_candidate(&connection, &candidate_id)
}

#[tauri::command]
pub fn clear_optimization_candidate(
    state: State<'_, AppState>,
    icon_id: String,
    profile_id: String,
    piece_id: Option<String>,
) -> AppResult<ClearOptimizationResultDto> {
    let connection = state.connection()?;
    crate::optimization::clear_optimization_candidate(
        &connection,
        &icon_id,
        &profile_id,
        piece_id.as_deref(),
    )
}

#[tauri::command]
pub fn revalidate_export_item(
    state: State<'_, AppState>,
    icon_id: String,
    profile_id: String,
    piece_id: Option<String>,
) -> AppResult<ExportAssetAnalysisDto> {
    let paths = state.paths().clone();
    let connection = state.connection()?;
    crate::optimization::revalidate_export_item(
        &connection,
        &paths,
        &icon_id,
        &profile_id,
        piece_id.as_deref(),
    )
}

#[tauri::command]
pub fn get_active_export_variant(
    state: State<'_, AppState>,
    icon_id: String,
    profile_id: String,
    piece_id: Option<String>,
) -> AppResult<Option<ActiveVariantDto>> {
    let connection = state.connection()?;
    crate::optimization::get_active_export_variant(
        &connection,
        &icon_id,
        &profile_id,
        piece_id.as_deref(),
    )
}
