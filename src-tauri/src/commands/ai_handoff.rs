use tauri::State;

use crate::app_state::AppState;
use crate::db::repositories::ai_handoff::{
    self as ai_handoff_repository, AiWebHandoffDeleteResultDto, AiWebHandoffDragResultDto,
    AiWebHandoffResultInspectionDto, AiWebHandoffSessionDto, PrepareAiWebHandoffPayload,
};
use crate::db::repositories::ai_handoff::{
    AiWebHandoffHistoryItemDto, AiWebHandoffMaintenanceReportDto, AiWebHandoffStorageStatusDto,
};
use crate::error::AppResult;
use crate::models::ImportImageFilePayload;

#[tauri::command]
pub fn prepare_ai_web_handoff(
    state: State<'_, AppState>,
    collection_id: String,
    payload: PrepareAiWebHandoffPayload,
) -> AppResult<AiWebHandoffSessionDto> {
    let paths = state.paths().clone();
    let mut connection = state.render_connection()?;
    ai_handoff_repository::prepare_ai_web_handoff(&mut connection, &paths, &collection_id, payload)
}

#[tauri::command]
pub fn get_ai_web_handoff(
    state: State<'_, AppState>,
    request_id: String,
) -> AppResult<AiWebHandoffSessionDto> {
    let paths = state.paths().clone();
    let mut connection = state.render_connection()?;
    ai_handoff_repository::get_ai_web_handoff(&mut connection, &paths, &request_id)
}

#[tauri::command]
pub fn get_latest_ai_web_handoff_for_icon(
    state: State<'_, AppState>,
    collection_id: String,
    icon_id: String,
) -> AppResult<Option<AiWebHandoffSessionDto>> {
    let paths = state.paths().clone();
    let mut connection = state.render_connection()?;
    ai_handoff_repository::get_latest_ai_web_handoff_for_icon(
        &mut connection,
        &paths,
        &collection_id,
        &icon_id,
    )
}

#[tauri::command]
pub fn reveal_ai_web_handoff_upload(
    state: State<'_, AppState>,
    request_id: String,
) -> AppResult<()> {
    let paths = state.paths().clone();
    let mut connection = state.render_connection()?;
    ai_handoff_repository::reveal_ai_web_handoff_upload(&mut connection, &paths, &request_id)
}

#[tauri::command]
pub fn start_ai_web_handoff_drag(
    window: tauri::Window,
    state: State<'_, AppState>,
    request_id: String,
) -> AppResult<AiWebHandoffDragResultDto> {
    let paths = state.paths().clone();
    let upload_path = {
        let mut connection = state.render_connection()?;
        ai_handoff_repository::verified_ai_web_handoff_upload_path(
            &mut connection,
            &paths,
            &request_id,
        )?
    };
    let outcome = crate::native_drag::start_verified_file_drag(&window, &paths, &upload_path)?;
    Ok(ai_web_handoff_drag_result(outcome))
}

fn ai_web_handoff_drag_result(
    outcome: crate::native_drag::NativeFileDragOutcome,
) -> AiWebHandoffDragResultDto {
    let message = match outcome {
        crate::native_drag::NativeFileDragOutcome::Dropped => {
            "upload.png를 놓았습니다. 웹 화면에 첨부됐는지 확인한 뒤 프롬프트를 붙여넣으세요."
        }
        crate::native_drag::NativeFileDragOutcome::Cancelled => {
            "파일 끌기를 취소했습니다. 다시 끌거나 탐색기에서 파일 선택을 사용하세요."
        }
    };
    AiWebHandoffDragResultDto {
        started: true,
        native_drag_supported: crate::native_drag::NATIVE_FILE_DRAG_SUPPORTED,
        message: message.to_string(),
    }
}

#[tauri::command]
pub fn validate_ai_web_handoff_result(
    state: State<'_, AppState>,
    request_id: String,
    file: ImportImageFilePayload,
) -> AppResult<AiWebHandoffResultInspectionDto> {
    let paths = state.paths().clone();
    let mut connection = state.render_connection()?;
    ai_handoff_repository::validate_ai_web_handoff_result(
        &mut connection,
        &paths,
        &request_id,
        &file,
    )
}

#[tauri::command]
pub fn commit_ai_web_handoff_result(
    state: State<'_, AppState>,
    request_id: String,
    file: ImportImageFilePayload,
    expected_validation_signature: String,
) -> AppResult<AiWebHandoffResultInspectionDto> {
    let paths = state.paths().clone();
    let mut connection = state.render_connection()?;
    ai_handoff_repository::commit_ai_web_handoff_result(
        &mut connection,
        &paths,
        &request_id,
        file,
        &expected_validation_signature,
    )
}

#[tauri::command]
pub fn extend_ai_web_handoff_retention(
    state: State<'_, AppState>,
    request_id: String,
) -> AppResult<AiWebHandoffSessionDto> {
    let paths = state.paths().clone();
    let mut connection = state.render_connection()?;
    ai_handoff_repository::extend_ai_web_handoff_retention(&mut connection, &paths, &request_id)
}

#[tauri::command]
pub fn delete_ai_web_handoff_payload(
    state: State<'_, AppState>,
    request_id: String,
) -> AppResult<AiWebHandoffDeleteResultDto> {
    let paths = state.paths().clone();
    let mut connection = state.render_connection()?;
    ai_handoff_repository::delete_ai_web_handoff_payload(&mut connection, &paths, &request_id)
}

#[tauri::command]
pub fn list_recent_ai_web_handoffs(
    state: State<'_, AppState>,
    limit: Option<u32>,
) -> AppResult<Vec<AiWebHandoffHistoryItemDto>> {
    let connection = state.render_connection()?;
    ai_handoff_repository::list_recent_ai_web_handoffs(&connection, limit)
}

#[tauri::command]
pub fn get_ai_web_handoff_storage_status(
    state: State<'_, AppState>,
) -> AppResult<AiWebHandoffStorageStatusDto> {
    let paths = state.paths().clone();
    let connection = state.render_connection()?;
    ai_handoff_repository::get_ai_web_handoff_storage_status(&connection, &paths)
}

#[tauri::command]
pub fn run_ai_web_handoff_maintenance(
    state: State<'_, AppState>,
) -> AppResult<AiWebHandoffMaintenanceReportDto> {
    let paths = state.paths().clone();
    let connection = state.render_connection()?;
    ai_handoff_repository::run_ai_web_handoff_maintenance(&connection, &paths)
}
#[cfg(test)]
mod tests {
    use crate::native_drag::NativeFileDragOutcome;

    use super::ai_web_handoff_drag_result;

    #[test]
    fn native_drag_result_distinguishes_drop_and_cancel() {
        let dropped = ai_web_handoff_drag_result(NativeFileDragOutcome::Dropped);
        assert!(dropped.started);
        assert_eq!(dropped.native_drag_supported, cfg!(windows));
        assert!(dropped.message.contains("놓았습니다"));

        let cancelled = ai_web_handoff_drag_result(NativeFileDragOutcome::Cancelled);
        assert!(cancelled.started);
        assert_eq!(cancelled.native_drag_supported, cfg!(windows));
        assert!(cancelled.message.contains("취소"));
    }
}
