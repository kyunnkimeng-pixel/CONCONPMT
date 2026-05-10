use tauri::State;

use crate::app_state::AppState;
use crate::db::repositories::settings as settings_repository;
use crate::error::AppResult;
use crate::models::{AppSettingsDto, SaveAppSettingsPayload};

#[tauri::command]
pub fn get_app_settings(state: State<'_, AppState>) -> AppResult<AppSettingsDto> {
    let connection = state.connection()?;
    settings_repository::get_app_settings(&connection)
}

#[tauri::command]
pub fn save_app_settings(
    state: State<'_, AppState>,
    payload: SaveAppSettingsPayload,
) -> AppResult<AppSettingsDto> {
    let connection = state.connection()?;
    settings_repository::save_app_settings(&connection, payload)
}
