use tauri::State;

use crate::app_state::AppState;
use crate::db::repositories::library as library_repository;
use crate::error::AppResult;
use crate::models::LibraryCleanupResultDto;

#[tauri::command]
pub fn preview_library_cleanup(
    state: State<'_, AppState>,
) -> AppResult<LibraryCleanupResultDto> {
    let paths = state.paths().clone();
    let connection = state.connection()?;
    library_repository::cleanup_library(&connection, &paths, false)
}

#[tauri::command]
pub fn cleanup_library(state: State<'_, AppState>) -> AppResult<LibraryCleanupResultDto> {
    let paths = state.paths().clone();
    let connection = state.connection()?;
    library_repository::cleanup_library(&connection, &paths, true)
}
