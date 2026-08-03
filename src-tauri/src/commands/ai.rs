use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use crate::ai_provider::provider;
use crate::app_state::AppState;
use crate::db::connection::open_existing_database;
use crate::db::repositories::ai as ai_repository;
use crate::error::{AppError, AppResult};
use crate::models::{
    ActivateAiCandidatePayload, AiNormalizationPreviewDto, AiProviderSessionStatusDto,
    AiReviewStateDto, AiSourceMutationResultDto, CreateAiIconRootPayload,
    CreateAiIconRootResultDto, ExecuteAiImageEditPayload, ImportAiCandidatePayload,
    PreviewAiCandidateNormalizationPayload, RepairAiToOriginalPayload, RestoreAiVersionPayload,
    SetAiSessionCredentialPayload,
};

#[tauri::command]
pub fn get_ai_provider_session_status(
    state: State<'_, AppState>,
) -> AppResult<AiProviderSessionStatusDto> {
    state.ai_credentials().status()
}

#[tauri::command]
pub fn set_ai_session_credential(
    state: State<'_, AppState>,
    payload: SetAiSessionCredentialPayload,
) -> AppResult<AiProviderSessionStatusDto> {
    state.ai_credentials().set(payload)
}

#[tauri::command]
pub fn clear_ai_session_credential(
    state: State<'_, AppState>,
    provider: String,
) -> AppResult<AiProviderSessionStatusDto> {
    state.ai_credentials().clear(&provider)
}

#[tauri::command]
pub async fn execute_ai_image_edit(
    state: State<'_, AppState>,
    collection_id: String,
    payload: ExecuteAiImageEditPayload,
) -> AppResult<AiReviewStateDto> {
    let credential = state.ai_credentials().credential(&payload.provider)?;
    let paths = state.paths().clone();
    let prepared = {
        let mut connection = state.render_connection()?;
        provider::start_image_edit(&mut connection, &paths, &collection_id, payload)?
    };
    let request_id = prepared.request_id.clone();
    let worker_paths = paths.clone();

    let worker = tauri::async_runtime::spawn_blocking(move || {
        let mut connection = open_existing_database(&worker_paths.database_path)?;
        provider::execute_started_http(
            &mut connection,
            &worker_paths,
            &prepared,
            credential.as_str(),
        )
    })
    .await;

    match worker {
        Ok(Ok(review_state)) => Ok(review_state),
        Ok(Err(error)) => {
            if let Ok(connection) = state.connection() {
                let _ = provider::record_started_request_failure(&connection, &request_id, &error);
            }
            Err(error)
        }
        Err(_) => {
            let error = AppError::new(
                "ai_provider_worker",
                "AI 공급자 작업이 예기치 않게 중단되어 결과를 저장하지 않았습니다.",
            );
            if let Ok(connection) = state.connection() {
                let _ = provider::record_started_request_failure(&connection, &request_id, &error);
            }
            Err(error)
        }
    }
}
#[tauri::command]
pub fn open_ai_official_resource(app: AppHandle, resource: String) -> AppResult<()> {
    let url = match resource.as_str() {
        "user_manual" => "https://github.com/kyunnkimeng-pixel/CONCONPMT",
        "novelai_app" => "https://novelai.net/image",
        "novelai_pat" => "https://novelai.net/account",
        "novelai_docs" => "https://image.novelai.net/docs/index.html",
        "novelai_terms" => "https://novelai.net/terms",
        "gemini_ai_studio" => "https://aistudio.google.com/",
        "gemini_image_docs" => "https://ai.google.dev/gemini-api/docs/image-generation",
        "gemini_pricing" => "https://ai.google.dev/gemini-api/docs/pricing",
        "gemini_terms" => "https://ai.google.dev/gemini-api/terms",
        _ => {
            return Err(AppError::new(
                "ai_official_resource_invalid",
                "허용되지 않은 외부 주소입니다.",
            ));
        }
    };
    app.opener().open_url(url, None::<&str>).map_err(|_| {
        AppError::new(
            "ai_official_resource_open",
            "공식 페이지를 열지 못했습니다.",
        )
    })
}

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
