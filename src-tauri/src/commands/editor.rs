use tauri::State;

use crate::app_state::AppState;
use crate::db::repositories::editor as editor_repository;
use crate::db::repositories::motion_editor as motion_editor_repository;
use crate::error::AppResult;
use crate::models::{
    ApplyIconCropPayload, EffectPreviewDto, IconDto, IconEditorStateDto, MotionPreviewDto,
    PreviewIconEffectsPayload, PreviewIconMotionPayload, UpdateIconEffectsPayload,
    UpdateIconMotionPayload, UpdateIconTextOverlayPayload,
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
    // This flow renders before its transaction and does not yet recheck dependent
    // effect/motion signatures. Keep the global guard to prevent a stale preview commit.
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
    // This flow has the same dependent-render constraint as crop updates.
    // Serialize it until the repository gains prepare/render/commit revalidation.
    let mut connection = state.connection()?;
    editor_repository::update_icon_text_overlay(&mut connection, &paths, &collection_id, payload)
}

#[tauri::command]
pub fn preview_icon_effects(
    state: State<'_, AppState>,
    collection_id: String,
    payload: PreviewIconEffectsPayload,
) -> AppResult<EffectPreviewDto> {
    let paths = state.paths().clone();
    let connection = state.render_connection()?;
    editor_repository::preview_icon_effects(&connection, &paths, &collection_id, payload)
}

#[tauri::command]
pub fn update_icon_effects(
    state: State<'_, AppState>,
    collection_id: String,
    payload: UpdateIconEffectsPayload,
) -> AppResult<IconEditorStateDto> {
    let paths = state.paths().clone();
    let mut connection = state.render_connection()?;
    editor_repository::update_icon_effects(&mut connection, &paths, &collection_id, payload)
}

#[tauri::command]
pub fn preview_icon_motion(
    state: State<'_, AppState>,
    collection_id: String,
    payload: PreviewIconMotionPayload,
) -> AppResult<MotionPreviewDto> {
    let paths = state.paths().clone();
    let prepared = {
        let connection = state.connection()?;
        motion_editor_repository::prepare_motion_preview(&connection, &collection_id, payload)?
    };
    motion_editor_repository::render_motion_preview(&paths, prepared)
}

#[tauri::command]
pub fn update_icon_motion(
    state: State<'_, AppState>,
    collection_id: String,
    payload: UpdateIconMotionPayload,
) -> AppResult<IconEditorStateDto> {
    let paths = state.paths().clone();
    let prepared = {
        let connection = state.connection()?;
        motion_editor_repository::prepare_motion_update(&connection, &collection_id, payload)?
    };
    let rendered = motion_editor_repository::render_motion_update(&paths, prepared)?;
    let mut connection = state.connection()?;
    motion_editor_repository::commit_motion_update(&mut connection, rendered)
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
