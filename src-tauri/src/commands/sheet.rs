use tauri::State;

use crate::app_state::AppState;
use crate::error::AppResult;
use crate::sheet::auto_detect::{AutoDetectSheetGridRequest, AutoDetectSheetGridResult};
use crate::sheet::exporter::{ExportEditSheetRequest, ExportEditSheetResult};
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
    let mut connection = state.connection()?;
    crate::sheet::importer::import_sheet_cells(&mut connection, &paths, request)
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
    let mut connection = state.connection()?;
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
    let connection = state.connection()?;
    crate::sheet::exporter::export_edit_sheet(&connection, &paths, request)
}

#[tauri::command]
pub fn reimport_edit_sheet(
    state: State<'_, AppState>,
    request: ReimportEditSheetRequest,
) -> AppResult<ReimportEditSheetResult> {
    let paths = state.paths().clone();
    let mut connection = state.connection()?;
    crate::sheet::reimport::reimport_edit_sheet(&mut connection, &paths, request)
}

#[tauri::command]
pub fn analyze_gif_frame_sheet_export(
    state: State<'_, AppState>,
    request: AnalyzeGifFrameSheetExportRequest,
) -> AppResult<GifFrameSheetExportAnalysis> {
    let connection = state.connection()?;
    crate::sheet::gif_frames::analyze_gif_frame_sheet_export(&connection, request)
}

#[tauri::command]
pub fn export_gif_frame_sheet(
    state: State<'_, AppState>,
    request: GifFrameSheetExportRequest,
) -> AppResult<GifFrameSheetExportResult> {
    let paths = state.paths().clone();
    let connection = state.connection()?;
    crate::sheet::gif_frames::export_gif_frame_sheet(&connection, &paths, request)
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
    let connection = state.connection()?;
    crate::sheet::gif_frames::reimport_gif_frame_sheet(&connection, &paths, request)
}

#[tauri::command]
pub fn list_sheet_grid_presets(
    state: State<'_, AppState>,
    collection_id: Option<String>,
) -> AppResult<Vec<SheetGridPresetDto>> {
    let connection = state.connection()?;
    crate::sheet::presets::list_sheet_grid_presets(&connection, collection_id)
}

#[tauri::command]
pub fn create_sheet_grid_preset(
    state: State<'_, AppState>,
    input: SheetGridPresetInput,
) -> AppResult<SheetGridPresetDto> {
    let connection = state.connection()?;
    crate::sheet::presets::create_sheet_grid_preset(&connection, input)
}

#[tauri::command]
pub fn update_sheet_grid_preset(
    state: State<'_, AppState>,
    id: String,
    input: SheetGridPresetInput,
) -> AppResult<SheetGridPresetDto> {
    let connection = state.connection()?;
    crate::sheet::presets::update_sheet_grid_preset(&connection, id, input)
}

#[tauri::command]
pub fn delete_sheet_grid_preset(state: State<'_, AppState>, id: String) -> AppResult<()> {
    let connection = state.connection()?;
    crate::sheet::presets::delete_sheet_grid_preset(&connection, id)
}

#[tauri::command]
pub fn duplicate_sheet_grid_preset(
    state: State<'_, AppState>,
    id: String,
) -> AppResult<SheetGridPresetDto> {
    let connection = state.connection()?;
    crate::sheet::presets::duplicate_sheet_grid_preset(&connection, id)
}

#[tauri::command]
pub fn set_default_sheet_grid_preset(
    state: State<'_, AppState>,
    id: String,
    target: String,
    collection_id: Option<String>,
) -> AppResult<SheetGridPresetDto> {
    let connection = state.connection()?;
    crate::sheet::presets::set_default_sheet_grid_preset(&connection, id, target, collection_id)
}

#[tauri::command]
pub fn get_default_sheet_grid_preset(
    state: State<'_, AppState>,
    target: String,
    collection_id: Option<String>,
) -> AppResult<Option<SheetGridPresetDto>> {
    let connection = state.connection()?;
    crate::sheet::presets::get_default_sheet_grid_preset(&connection, target, collection_id)
}
