use tauri::State;

use crate::app_state::AppState;
use crate::db::repositories::editor as editor_repository;
use crate::error::AppResult;
use crate::models::{
    ApplyIconCropPayload, IconDto, IconEditorStateDto, UpdateIconTextOverlayPayload,
};

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

#[tauri::command]
pub fn update_icon_text_overlay(
    state: State<'_, AppState>,
    collection_id: String,
    payload: UpdateIconTextOverlayPayload,
) -> AppResult<IconEditorStateDto> {
    let paths = state.paths().clone();
    let mut connection = state.connection()?;
    editor_repository::update_icon_text_overlay(&mut connection, &paths, &collection_id, payload)
}

#[tauri::command]
pub fn pick_text_overlay_font(initial_directory: Option<String>) -> AppResult<Option<String>> {
    let mut dialog = rfd::FileDialog::new()
        .set_title("텍스트 폰트 선택")
        .add_filter("Font", &["ttf", "otf"]);
    if let Some(path) = initial_directory {
        if !path.trim().is_empty() {
            dialog = dialog.set_directory(path);
        }
    }

    Ok(dialog
        .pick_file()
        .map(|path| path.to_string_lossy().to_string()))
}
