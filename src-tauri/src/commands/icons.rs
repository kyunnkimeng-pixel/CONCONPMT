use tauri::State;

use crate::app_state::AppState;
use crate::db::repositories::collections as collection_repository;
use crate::db::repositories::icons as icon_repository;
use crate::error::AppResult;
use crate::models::{CollectionDto, IconDto};

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
