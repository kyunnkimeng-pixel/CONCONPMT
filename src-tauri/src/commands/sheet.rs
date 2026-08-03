use serde::Serialize;
use tauri::State;

use crate::app_state::AppState;
use crate::error::AppResult;
use crate::sheet::auto_detect::{AutoDetectSheetGridRequest, AutoDetectSheetGridResult};
use crate::sheet::exporter::{ExportEditSheetRequest, ExportEditSheetResult};
use crate::sheet::frame_sheet_gif::{
    FrameSheetGifCreateResult, FrameSheetGifMeasurement, FrameSheetGifRequest,
};
use crate::sheet::gif_frames::{
    AnalyzeGifFrameSheetExportRequest, GifFrameSheetExportAnalysis, GifFrameSheetExportRequest,
    GifFrameSheetExportResult, GifFrameSheetReimportRequest, GifFrameSheetReimportResult,
    GifFrameSheetReimportValidation, ValidateGifFrameSheetReimportRequest,
};
use crate::sheet::grid::{SheetGridAnalysis, SheetGridAnalyzeRequest};
use crate::sheet::importer::{ImportSheetCellsRequest, ImportSheetCellsResult};
use crate::sheet::presets::{SheetGridPresetDto, SheetGridPresetInput};
use crate::sheet::preview::{SheetPreviewRequest, SheetPreviewResult};
use crate::sheet::reimport::{ReimportEditSheetRequest, ReimportEditSheetResult};
use crate::sheet::slices::{
    AnalyzeManualSlicesRequest, ImportManualSlicesRequest, ImportManualSlicesResult,
    ManualSliceAnalysis, ManualSliceLoadResult, ManualSliceSaveRequest, ManualSliceSaveResult,
};

#[tauri::command]
pub fn analyze_sheet_grid(
    _state: State<'_, AppState>,
    request: SheetGridAnalyzeRequest,
) -> AppResult<SheetGridAnalysis> {
    crate::sheet::grid::analyze_sheet_grid(request)
}

#[tauri::command]
pub fn preview_sheet_slices(
    _state: State<'_, AppState>,
    request: SheetPreviewRequest,
) -> AppResult<SheetPreviewResult> {
    crate::sheet::preview::preview_sheet_slices(request)
}

#[tauri::command]
pub fn auto_detect_sheet_grid(
    _state: State<'_, AppState>,
    request: AutoDetectSheetGridRequest,
) -> AppResult<AutoDetectSheetGridResult> {
    crate::sheet::auto_detect::auto_detect_sheet_grid(request)
}

#[tauri::command]
pub fn import_sheet_cells(
    state: State<'_, AppState>,
    request: ImportSheetCellsRequest,
) -> AppResult<ImportSheetCellsResult> {
    let paths = state.paths().clone();
    let mut connection = state.render_connection()?;
    crate::sheet::importer::import_sheet_cells(&mut connection, &paths, request)
}

#[tauri::command]
pub fn measure_frame_sheet_gif(
    state: State<'_, AppState>,
    request: FrameSheetGifRequest,
) -> AppResult<FrameSheetGifMeasurement> {
    let paths = state.paths().clone();
    let connection = state.render_connection()?;
    crate::sheet::frame_sheet_gif::measure_frame_sheet_gif(&connection, &paths, request)
}

#[tauri::command]
pub fn create_frame_sheet_gif(
    state: State<'_, AppState>,
    request: FrameSheetGifRequest,
) -> AppResult<FrameSheetGifCreateResult> {
    let paths = state.paths().clone();
    let mut connection = state.render_connection()?;
    crate::sheet::frame_sheet_gif::create_frame_sheet_gif(&mut connection, &paths, request)
}

#[tauri::command]
pub fn analyze_manual_slices(
    _state: State<'_, AppState>,
    request: AnalyzeManualSlicesRequest,
) -> AppResult<ManualSliceAnalysis> {
    crate::sheet::slices::analyze_manual_slices(request)
}

#[tauri::command]
pub fn import_manual_slices(
    state: State<'_, AppState>,
    request: ImportManualSlicesRequest,
) -> AppResult<ImportManualSlicesResult> {
    let paths = state.paths().clone();
    let mut connection = state.render_connection()?;
    crate::sheet::slices::import_manual_slices(&mut connection, &paths, request)
}

#[tauri::command]
pub fn save_manual_slices(
    state: State<'_, AppState>,
    request: ManualSliceSaveRequest,
) -> AppResult<ManualSliceSaveResult> {
    let paths = state.paths().clone();
    crate::sheet::slices::save_manual_slices(&paths, request)
}

#[tauri::command]
pub fn load_manual_slices(
    state: State<'_, AppState>,
    sheet_id: String,
) -> AppResult<ManualSliceLoadResult> {
    let paths = state.paths().clone();
    crate::sheet::slices::load_manual_slices(&paths, sheet_id)
}

#[tauri::command]
pub fn export_edit_sheet(
    state: State<'_, AppState>,
    request: ExportEditSheetRequest,
) -> AppResult<ExportEditSheetResult> {
    let paths = state.paths().clone();
    let connection = state.render_connection()?;
    crate::sheet::exporter::export_edit_sheet(&connection, &paths, request)
}

#[tauri::command]
pub fn reimport_edit_sheet(
    state: State<'_, AppState>,
    request: ReimportEditSheetRequest,
) -> AppResult<ReimportEditSheetResult> {
    let paths = state.paths().clone();
    let mut connection = state.render_connection()?;
    crate::sheet::reimport::reimport_edit_sheet(&mut connection, &paths, request)
}

#[tauri::command]
pub fn analyze_gif_frame_sheet_export(
    state: State<'_, AppState>,
    request: AnalyzeGifFrameSheetExportRequest,
) -> AppResult<GifFrameSheetExportAnalysis> {
    let connection = state.render_connection()?;
    crate::sheet::gif_frames::analyze_gif_frame_sheet_export(&connection, request)
}

#[tauri::command]
pub fn export_gif_frame_sheet(
    state: State<'_, AppState>,
    request: GifFrameSheetExportRequest,
) -> AppResult<GifFrameSheetExportResult> {
    let paths = state.paths().clone();
    let connection = state.render_connection()?;
    crate::sheet::gif_frames::export_gif_frame_sheet(&connection, &paths, request)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GifFrameSheetPageDragResultDto {
    pub started: bool,
    pub native_drag_supported: bool,
    pub message: String,
}

#[tauri::command]
pub fn start_gif_frame_sheet_page_drag(
    window: tauri::Window,
    state: State<'_, AppState>,
    manifest_path: String,
    page_index: i64,
) -> AppResult<GifFrameSheetPageDragResultDto> {
    let paths = state.paths().clone();
    let prepared = crate::sheet::gif_frames::prepare_gif_frame_sheet_page_handoff(
        &paths,
        &manifest_path,
        page_index,
    )?;
    let outcome =
        crate::native_drag::start_verified_file_drag(&window, &paths, &prepared.staged_path)?;
    let message = match outcome {
        crate::native_drag::NativeFileDragOutcome::Dropped => format!(
            "{} 페이지 clean PNG({})를 놓았습니다. 웹 화면에 첨부됐는지 확인하세요.",
            prepared.page_index + 1,
            prepared.file_name
        ),
        crate::native_drag::NativeFileDragOutcome::Cancelled => format!(
            "{} 페이지 파일 끌기를 취소했습니다. 다시 끌거나 ‘파일 위치 열기’를 사용하세요.",
            prepared.page_index + 1
        ),
    };
    Ok(GifFrameSheetPageDragResultDto {
        started: true,
        native_drag_supported: crate::native_drag::NATIVE_FILE_DRAG_SUPPORTED,
        message,
    })
}

#[tauri::command]
pub fn reveal_gif_frame_sheet_page(
    state: State<'_, AppState>,
    manifest_path: String,
    page_index: i64,
) -> AppResult<()> {
    let paths = state.paths().clone();
    let prepared = crate::sheet::gif_frames::prepare_gif_frame_sheet_page_handoff(
        &paths,
        &manifest_path,
        page_index,
    )?;
    crate::export::open_export_path(&prepared.staged_path.to_string_lossy())
}

#[tauri::command]
pub fn validate_gif_frame_sheet_reimport(
    _state: State<'_, AppState>,
    request: ValidateGifFrameSheetReimportRequest,
) -> AppResult<GifFrameSheetReimportValidation> {
    crate::sheet::gif_frames::validate_gif_frame_sheet_reimport(request)
}

#[tauri::command]
pub fn reimport_gif_frame_sheet(
    state: State<'_, AppState>,
    request: GifFrameSheetReimportRequest,
) -> AppResult<GifFrameSheetReimportResult> {
    let paths = state.paths().clone();
    let connection = state.render_connection()?;
    crate::sheet::gif_frames::reimport_gif_frame_sheet(&connection, &paths, request)
}

#[tauri::command]
pub fn list_sheet_grid_presets(
    state: State<'_, AppState>,
    collection_id: Option<String>,
) -> AppResult<Vec<SheetGridPresetDto>> {
    let connection = state.render_connection()?;
    crate::sheet::presets::list_sheet_grid_presets(&connection, collection_id)
}

#[tauri::command]
pub fn create_sheet_grid_preset(
    state: State<'_, AppState>,
    input: SheetGridPresetInput,
) -> AppResult<SheetGridPresetDto> {
    let connection = state.render_connection()?;
    crate::sheet::presets::create_sheet_grid_preset(&connection, input)
}

#[tauri::command]
pub fn update_sheet_grid_preset(
    state: State<'_, AppState>,
    id: String,
    input: SheetGridPresetInput,
) -> AppResult<SheetGridPresetDto> {
    let connection = state.render_connection()?;
    crate::sheet::presets::update_sheet_grid_preset(&connection, id, input)
}

#[tauri::command]
pub fn delete_sheet_grid_preset(state: State<'_, AppState>, id: String) -> AppResult<()> {
    let connection = state.render_connection()?;
    crate::sheet::presets::delete_sheet_grid_preset(&connection, id)
}

#[tauri::command]
pub fn duplicate_sheet_grid_preset(
    state: State<'_, AppState>,
    id: String,
) -> AppResult<SheetGridPresetDto> {
    let connection = state.render_connection()?;
    crate::sheet::presets::duplicate_sheet_grid_preset(&connection, id)
}

#[tauri::command]
pub fn set_default_sheet_grid_preset(
    state: State<'_, AppState>,
    id: String,
    target: String,
    collection_id: Option<String>,
) -> AppResult<SheetGridPresetDto> {
    let connection = state.render_connection()?;
    crate::sheet::presets::set_default_sheet_grid_preset(&connection, id, target, collection_id)
}

#[tauri::command]
pub fn get_default_sheet_grid_preset(
    state: State<'_, AppState>,
    target: String,
    collection_id: Option<String>,
) -> AppResult<Option<SheetGridPresetDto>> {
    let connection = state.render_connection()?;
    crate::sheet::presets::get_default_sheet_grid_preset(&connection, target, collection_id)
}
