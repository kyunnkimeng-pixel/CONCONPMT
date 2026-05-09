use tauri::State;

use crate::app_state::AppState;
use crate::db::repositories::editor as editor_repository;
use crate::error::AppResult;
use crate::models::{ApplyIconCropPayload, IconDto, IconEditorStateDto};

#[tauri::command]
pub fn get_icon_editor_state(
    state: State<'_, AppState>,
    collection_id: String,
    icon_id: String,
) -> AppResult<IconEditorStateDto> {
    let connection = state.connection()?;
    editor_repository::get_icon_editor_state(&connection, &collection_id, &icon_id)
}

#[tauri::command]
pub fn apply_icon_crop(
    state: State<'_, AppState>,
    collection_id: String,
    payload: ApplyIconCropPayload,
) -> AppResult<IconDto> {
    let paths = state.paths().clone();
    let mut connection = state.connection()?;
    editor_repository::apply_icon_crop(&mut connection, &paths, &collection_id, payload)
}
