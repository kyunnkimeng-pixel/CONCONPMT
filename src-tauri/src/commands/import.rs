use tauri::State;

use crate::app_state::AppState;
use crate::db::repositories::imports as import_repository;
use crate::error::AppResult;
use crate::models::{ImportImageFilePayload, ImportImagesResultDto};

#[tauri::command]
pub fn import_image_files(
    state: State<'_, AppState>,
    collection_id: String,
    files: Vec<ImportImageFilePayload>,
) -> AppResult<ImportImagesResultDto> {
    let paths = state.paths().clone();
    let mut connection = state.connection()?;

    import_repository::import_image_files(&mut connection, &paths, &collection_id, files)
}
