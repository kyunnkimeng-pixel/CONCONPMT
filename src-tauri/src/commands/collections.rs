use tauri::State;

use crate::app_state::AppState;
use crate::db::repositories::collections as collection_repository;
use crate::error::AppResult;
use crate::models::CollectionDto;

#[tauri::command]
pub fn list_collections(state: State<'_, AppState>) -> AppResult<Vec<CollectionDto>> {
    let connection = state.connection()?;
    collection_repository::list_collections(&connection)
}

#[tauri::command]
pub fn create_collection(
    state: State<'_, AppState>,
    name: Option<String>,
) -> AppResult<CollectionDto> {
    let mut connection = state.connection()?;
    collection_repository::create_collection(&mut connection, name)
}

#[tauri::command]
pub fn rename_collection(
    state: State<'_, AppState>,
    collection_id: String,
    name: String,
) -> AppResult<CollectionDto> {
    let connection = state.connection()?;
    collection_repository::rename_collection(&connection, &collection_id, name)
}

#[tauri::command]
pub fn delete_collection(state: State<'_, AppState>, collection_id: String) -> AppResult<()> {
    let mut connection = state.connection()?;
    collection_repository::delete_collection(&mut connection, &collection_id)
}

#[tauri::command]
pub fn duplicate_collection(
    state: State<'_, AppState>,
    collection_id: String,
) -> AppResult<CollectionDto> {
    let mut connection = state.connection()?;
    collection_repository::duplicate_collection(&mut connection, &collection_id)
}

#[tauri::command]
pub fn set_collection_cover_icon(
    state: State<'_, AppState>,
    collection_id: String,
    icon_id: String,
) -> AppResult<CollectionDto> {
    let connection = state.connection()?;
    collection_repository::set_collection_cover_icon(&connection, &collection_id, &icon_id)
}
