use tauri::State;

use crate::app_state::AppState;
use crate::db::repositories::ai as ai_repository;
use crate::error::{AppError, AppResult};
use crate::models::{
    ActivateAiCandidatePayload, AiNormalizationPreviewDto, AiReviewStateDto,
    AiSourceMutationResultDto, CreateAiIconRootPayload, CreateAiIconRootResultDto,
    ImportAiCandidatePayload, PreviewAiCandidateNormalizationPayload, RepairAiToOriginalPayload,
    RestoreAiVersionPayload,
};

#[tauri::command]
pub fn get_ai_review_state(
    state: State<'_, AppState>,
    collection_id: String,
    icon_id: String,
) -> AppResult<AiReviewStateDto> {
    let connection = state.connection()?;
    ai_repository::get_ai_review_state(&connection, &collection_id, &icon_id)
}

#[tauri::command]
pub fn import_local_ai_candidate(
    state: State<'_, AppState>,
    collection_id: String,
    payload: ImportAiCandidatePayload,
) -> AppResult<AiReviewStateDto> {
    let paths = state.paths().clone();
    let mut connection = state.render_connection()?;
    ai_repository::import_local_ai_candidate(&mut connection, &paths, &collection_id, payload)
}

#[tauri::command]
pub fn preview_ai_candidate_normalization(
    state: State<'_, AppState>,
    collection_id: String,
    payload: PreviewAiCandidateNormalizationPayload,
) -> AppResult<AiNormalizationPreviewDto> {
    let paths = state.paths().clone();
    let connection = state.render_connection()?;
    ai_repository::preview_ai_candidate_normalization(&connection, &paths, &collection_id, payload)
}

#[tauri::command]
pub fn activate_ai_candidate(
    state: State<'_, AppState>,
    collection_id: String,
    payload: ActivateAiCandidatePayload,
) -> AppResult<AiSourceMutationResultDto> {
    require_preview_signature(payload.expected_preview_signature.as_deref())?;
    let paths = state.paths().clone();
    let mut connection = state.render_connection()?;
    ai_repository::activate_ai_candidate(&mut connection, &paths, &collection_id, payload)
}

#[tauri::command]
pub fn create_ai_icon_root(
    state: State<'_, AppState>,
    collection_id: String,
    payload: CreateAiIconRootPayload,
) -> AppResult<CreateAiIconRootResultDto> {
    require_preview_signature(payload.expected_preview_signature.as_deref())?;
    let paths = state.paths().clone();
    let mut connection = state.render_connection()?;
    ai_repository::create_ai_icon_root(&mut connection, &paths, &collection_id, payload)
}
#[tauri::command]
pub fn restore_ai_version(
    state: State<'_, AppState>,
    collection_id: String,
    payload: RestoreAiVersionPayload,
) -> AppResult<AiSourceMutationResultDto> {
    let paths = state.paths().clone();
    let mut connection = state.render_connection()?;
    ai_repository::restore_ai_version(&mut connection, &paths, &collection_id, payload)
}

#[tauri::command]
pub fn repair_ai_to_original(
    state: State<'_, AppState>,
    collection_id: String,
    payload: RepairAiToOriginalPayload,
) -> AppResult<AiReviewStateDto> {
    let paths = state.paths().clone();
    let mut connection = state.render_connection()?;
    ai_repository::repair_ai_to_original(&mut connection, &paths, &collection_id, payload)
}

fn require_preview_signature(signature: Option<&str>) -> AppResult<()> {
    if signature.is_some_and(|value| !value.trim().is_empty()) {
        return Ok(());
    }
    Err(AppError::new(
        "ai_normalization_preview_required",
        "적용하기 전에 현재 설정으로 규격화 미리보기를 확인해 주세요.",
    ))
}
