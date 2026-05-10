use tauri::State;

use crate::app_state::AppState;
use crate::db::repositories::export_profiles as export_profile_repository;
use crate::error::AppResult;
use crate::models::{
    ExportCollectionResultDto, ExportProfileDto, ExportRequestPayload, ExportValidationResultDto,
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
