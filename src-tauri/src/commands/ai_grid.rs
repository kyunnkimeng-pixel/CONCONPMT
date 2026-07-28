use serde::{Deserialize, Serialize};
use tauri::State;

use crate::app_state::AppState;
use crate::db::repositories::ai_grid::{
    self as ai_grid_repository, AiGridWorkspaceDto, CommitAiGridCandidatesResult,
    CommitGeneratedIconsResult, FinalizeGeneratedIconInput, PrepareAiGenerationRequest,
};
use crate::error::AppResult;
use crate::models::ImportImageFilePayload;
use crate::sheet::composer::{default_ai_generation_layout, default_ai_grid_layout, AiGridLayout};
use crate::sheet::grid::{SheetGridAnalysis, SheetGridSettings};
use crate::sheet::splitter::ReviewedGridDecision;

const DEFAULT_AI_GRID_CANVAS_SIZE: i64 = 1_024;
const DEFAULT_OUTPUT_MANIFEST: &str = r#"{"schema":"pmtcon-ai-grid-v1","kind":"manual-output"}"#;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareAiGridEditWorkspacePayload {
    pub selected_icon_ids: Vec<String>,
    pub layout: Option<AiGridLayout>,
    pub canvas_size: Option<i64>,
    pub retry_of_request_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareAiGenerationWorkspacePayload {
    pub target_names: Vec<String>,
    pub layout: Option<AiGridLayout>,
    pub canvas_size: Option<i64>,
    pub payload_input_signature: String,
    pub retry_of_request_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiGridReviewCommitDto {
    pub commit: CommitAiGridCandidatesResult,
    pub workspace: AiGridWorkspaceDto,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiGeneratedIconsCommitDto {
    pub commit: CommitGeneratedIconsResult,
    pub workspace: AiGridWorkspaceDto,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiGridInputDragResultDto {
    pub started: bool,
    pub native_drag_supported: bool,
    pub message: String,
}

#[tauri::command]
pub fn prepare_ai_grid_edit_workspace(
    state: State<'_, AppState>,
    collection_id: String,
    payload: PrepareAiGridEditWorkspacePayload,
) -> AppResult<AiGridWorkspaceDto> {
    let layout = payload.layout.unwrap_or(default_ai_grid_layout(
        payload.selected_icon_ids.len(),
        payload.canvas_size.unwrap_or(DEFAULT_AI_GRID_CANVAS_SIZE),
    )?);
    let paths = state.paths().clone();
    let mut connection = state.render_connection()?;
    let prepared = ai_grid_repository::prepare_ai_grid_edit(
        &mut connection,
        &paths,
        &collection_id,
        payload.selected_icon_ids,
        layout,
        payload.retry_of_request_id,
    )?;
    ai_grid_repository::get_ai_grid_workspace(&connection, &prepared.request_id)
}

#[tauri::command]
pub fn prepare_ai_generation_workspace(
    state: State<'_, AppState>,
    collection_id: String,
    payload: PrepareAiGenerationWorkspacePayload,
) -> AppResult<AiGridWorkspaceDto> {
    let layout = payload.layout.unwrap_or(default_ai_generation_layout(
        payload.target_names.len(),
        payload.canvas_size.unwrap_or(DEFAULT_AI_GRID_CANVAS_SIZE),
    )?);
    let mut connection = state.render_connection()?;
    let prepared = ai_grid_repository::prepare_ai_generation(
        &mut connection,
        &collection_id,
        PrepareAiGenerationRequest {
            target_names: payload.target_names,
            layout,
            payload_input_signature: payload.payload_input_signature,
            retry_of_request_id: payload.retry_of_request_id,
        },
    )?;
    ai_grid_repository::get_ai_grid_workspace(&connection, &prepared.request_id)
}

#[tauri::command]
pub fn get_ai_grid_workspace(
    state: State<'_, AppState>,
    request_id: String,
) -> AppResult<AiGridWorkspaceDto> {
    let connection = state.render_connection()?;
    ai_grid_repository::get_ai_grid_workspace(&connection, &request_id)
}

#[tauri::command]
pub fn get_latest_ai_grid_workspace(
    state: State<'_, AppState>,
    collection_id: String,
) -> AppResult<Option<AiGridWorkspaceDto>> {
    let connection = state.render_connection()?;
    ai_grid_repository::get_latest_ai_grid_workspace(&connection, &collection_id)
}

#[tauri::command]
pub fn mark_ai_grid_workspace_awaiting_result(
    state: State<'_, AppState>,
    request_id: String,
) -> AppResult<AiGridWorkspaceDto> {
    let connection = state.render_connection()?;
    ai_grid_repository::mark_ai_grid_awaiting_result(&connection, &request_id)?;
    ai_grid_repository::get_ai_grid_workspace(&connection, &request_id)
}

#[tauri::command]
pub fn attach_ai_grid_output(
    state: State<'_, AppState>,
    request_id: String,
    file: ImportImageFilePayload,
    manifest_json: Option<String>,
) -> AppResult<AiGridWorkspaceDto> {
    let paths = state.paths().clone();
    let mut connection = state.render_connection()?;
    ai_grid_repository::record_ai_grid_output_artifact(
        &mut connection,
        &paths,
        &request_id,
        file,
        manifest_json.as_deref().unwrap_or(DEFAULT_OUTPUT_MANIFEST),
    )?;
    ai_grid_repository::get_ai_grid_workspace(&connection, &request_id)
}

#[tauri::command]
pub fn analyze_ai_grid_output(
    state: State<'_, AppState>,
    request_id: String,
    settings: SheetGridSettings,
) -> AppResult<SheetGridAnalysis> {
    let connection = state.render_connection()?;
    ai_grid_repository::analyze_ai_grid_output(&connection, &request_id, settings)
}

#[tauri::command]
pub fn commit_ai_grid_review(
    state: State<'_, AppState>,
    request_id: String,
    decisions: Vec<ReviewedGridDecision>,
) -> AppResult<AiGridReviewCommitDto> {
    let paths = state.paths().clone();
    let mut connection = state.render_connection()?;
    let commit = ai_grid_repository::commit_ai_grid_candidates(
        &mut connection,
        &paths,
        &request_id,
        decisions,
    )?;
    let workspace = ai_grid_repository::get_ai_grid_workspace(&connection, &request_id)?;
    Ok(AiGridReviewCommitDto { commit, workspace })
}

#[tauri::command]
pub fn commit_ai_generated_icons(
    state: State<'_, AppState>,
    collection_id: String,
    request_id: String,
    finalized_items: Vec<FinalizeGeneratedIconInput>,
) -> AppResult<AiGeneratedIconsCommitDto> {
    let paths = state.paths().clone();
    let mut connection = state.render_connection()?;
    let commit = ai_grid_repository::commit_ai_generated_icons(
        &mut connection,
        &paths,
        &collection_id,
        &request_id,
        finalized_items,
    )?;
    let workspace = ai_grid_repository::get_ai_grid_workspace(&connection, &request_id)?;
    Ok(AiGeneratedIconsCommitDto { commit, workspace })
}

#[tauri::command]
pub fn cancel_ai_grid_workspace(
    state: State<'_, AppState>,
    request_id: String,
) -> AppResult<AiGridWorkspaceDto> {
    let connection = state.render_connection()?;
    ai_grid_repository::cancel_ai_grid_request(&connection, &request_id)?;
    ai_grid_repository::get_ai_grid_workspace(&connection, &request_id)
}

#[tauri::command]
pub fn reveal_ai_grid_input(state: State<'_, AppState>, request_id: String) -> AppResult<()> {
    let paths = state.paths().clone();
    let connection = state.render_connection()?;
    ai_grid_repository::reveal_ai_grid_input(&connection, &paths, &request_id)
}

#[tauri::command]
pub fn start_ai_grid_input_drag(
    window: tauri::Window,
    state: State<'_, AppState>,
    request_id: String,
) -> AppResult<AiGridInputDragResultDto> {
    let paths = state.paths().clone();
    let input_path = {
        let connection = state.render_connection()?;
        ai_grid_repository::verified_ai_grid_input_path(&connection, &paths, &request_id)?
    };
    let outcome = crate::native_drag::start_verified_file_drag(&window, &paths, &input_path)?;
    let message = match outcome {
        crate::native_drag::NativeFileDragOutcome::Dropped => {
            "입력 스프라이트를 놓았습니다. 웹 화면에 첨부됐는지 확인한 뒤 프롬프트를 붙여넣으세요."
        }
        crate::native_drag::NativeFileDragOutcome::Cancelled => {
            "파일 끌기를 취소했습니다. 다시 끌거나 탐색기에서 파일 선택을 사용하세요."
        }
    };
    Ok(AiGridInputDragResultDto {
        started: true,
        native_drag_supported: crate::native_drag::NATIVE_FILE_DRAG_SUPPORTED,
        message: message.to_string(),
    })
}
