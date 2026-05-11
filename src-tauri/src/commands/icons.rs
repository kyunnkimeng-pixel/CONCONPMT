use tauri::State;

use crate::app_state::AppState;
use crate::db::repositories::collections as collection_repository;
use crate::db::repositories::icons as icon_repository;
use crate::error::AppResult;
use crate::models::{CollectionDto, CreatePlaceholderIconPayload, IconDto, ImportImageFilePayload};

#[tauri::command]
pub fn list_icons(state: State<'_, AppState>, collection_id: String) -> AppResult<Vec<IconDto>> {
    let connection = state.connection()?;
    icon_repository::list_icons(&connection, &collection_id)
}

#[tauri::command]
pub fn update_icon_piece_alt(
    state: State<'_, AppState>,
    collection_id: String,
    piece_id: String,
    alt_text: String,
) -> AppResult<IconDto> {
    let connection = state.connection()?;
    icon_repository::update_icon_piece_alt(&connection, &collection_id, &piece_id, alt_text)
}

#[tauri::command]
pub fn rename_icon(
    state: State<'_, AppState>,
    collection_id: String,
    icon_id: String,
    display_name: String,
) -> AppResult<IconDto> {
    let connection = state.connection()?;
    icon_repository::rename_icon(&connection, &collection_id, &icon_id, display_name)
}

#[tauri::command]
pub fn set_icon_thumbnail_override(
    state: State<'_, AppState>,
    collection_id: String,
    icon_id: String,
    file: ImportImageFilePayload,
) -> AppResult<IconDto> {
    let paths = state.paths().clone();
    let mut connection = state.connection()?;
    icon_repository::set_icon_thumbnail_override(
        &mut connection,
        &paths,
        &collection_id,
        &icon_id,
        file,
    )
}

#[tauri::command]
pub fn create_placeholder_icon(
    state: State<'_, AppState>,
    collection_id: String,
    payload: CreatePlaceholderIconPayload,
) -> AppResult<IconDto> {
    let paths = state.paths().clone();
    let mut connection = state.connection()?;
    icon_repository::create_placeholder_icon(&mut connection, &paths, &collection_id, payload)
}

#[tauri::command]
pub fn replace_icon_source(
    state: State<'_, AppState>,
    collection_id: String,
    icon_id: String,
    file: ImportImageFilePayload,
) -> AppResult<IconDto> {
    let paths = state.paths().clone();
    let mut connection = state.connection()?;
    icon_repository::replace_icon_source(&mut connection, &paths, &collection_id, &icon_id, file)
}

#[tauri::command]
pub fn set_icons_readiness(
    state: State<'_, AppState>,
    collection_id: String,
    icon_ids: Vec<String>,
    readiness: String,
) -> AppResult<Vec<IconDto>> {
    let connection = state.connection()?;
    icon_repository::set_icons_readiness(&connection, &collection_id, icon_ids, readiness)
}

#[tauri::command]
pub fn duplicate_icon(
    state: State<'_, AppState>,
    collection_id: String,
    icon_id: String,
) -> AppResult<IconDto> {
    let mut connection = state.connection()?;
    icon_repository::duplicate_icon(&mut connection, &collection_id, &icon_id)
}

#[tauri::command]
pub fn delete_icons(
    state: State<'_, AppState>,
    collection_id: String,
    icon_ids: Vec<String>,
) -> AppResult<CollectionDto> {
    let mut connection = state.connection()?;
    icon_repository::delete_icons(&mut connection, &collection_id, icon_ids)?;
    collection_repository::get_collection(&connection, &collection_id)
}

#[tauri::command]
pub fn reorder_icons(
    state: State<'_, AppState>,
    collection_id: String,
    icon_ids: Vec<String>,
) -> AppResult<Vec<IconDto>> {
    let connection = state.connection()?;
    icon_repository::reorder_icons(&connection, &collection_id, icon_ids)
}

#[tauri::command]
pub fn reveal_icon_original(
    state: State<'_, AppState>,
    collection_id: String,
    icon_id: String,
) -> AppResult<()> {
    let connection = state.connection()?;
    let path = icon_repository::original_path_for_icon(&connection, &collection_id, &icon_id)?;
    crate::export::open_export_path(&path)
}

#[tauri::command]
pub fn reveal_icon_export_result(
    state: State<'_, AppState>,
    collection_id: String,
    icon_id: String,
) -> AppResult<()> {
    let connection = state.connection()?;
    let path = icon_repository::export_result_path_for_icon(&connection, &collection_id, &icon_id)?;
    crate::export::open_export_path(&path)
}
