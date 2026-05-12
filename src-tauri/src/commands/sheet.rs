use tauri::State;

use crate::app_state::AppState;
use crate::error::AppResult;
use crate::sheet::exporter::{ExportEditSheetRequest, ExportEditSheetResult};
use crate::sheet::grid::{SheetGridAnalysis, SheetGridAnalyzeRequest};
use crate::sheet::importer::{ImportSheetCellsRequest, ImportSheetCellsResult};
use crate::sheet::preview::{SheetPreviewRequest, SheetPreviewResult};
use crate::sheet::reimport::{ReimportEditSheetRequest, ReimportEditSheetResult};

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
pub fn import_sheet_cells(
    state: State<'_, AppState>,
    request: ImportSheetCellsRequest,
) -> AppResult<ImportSheetCellsResult> {
    let paths = state.paths().clone();
    let mut connection = state.connection()?;
    crate::sheet::importer::import_sheet_cells(&mut connection, &paths, request)
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
