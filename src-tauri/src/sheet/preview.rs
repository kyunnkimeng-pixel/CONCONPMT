use serde::{Deserialize, Serialize};

use crate::error::AppResult;
use crate::models::ImportImageFilePayload;

use super::grid::{analyze_sheet_grid, SheetCell, SheetGridAnalyzeRequest, SheetGridSettings};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetPreviewRequest {
    pub sheet_path: Option<String>,
    pub sheet_file: Option<ImportImageFilePayload>,
    pub grid_settings: SheetGridSettings,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetPreviewResult {
    pub cells: Vec<SheetCell>,
    pub empty_cell_candidates: Vec<i64>,
    pub warnings: Vec<String>,
}

pub fn preview_sheet_slices(request: SheetPreviewRequest) -> AppResult<SheetPreviewResult> {
    let analysis = analyze_sheet_grid(SheetGridAnalyzeRequest {
        sheet_path: request.sheet_path,
        sheet_file: request.sheet_file,
        mode: request.grid_settings.mode,
        rows: request.grid_settings.rows,
        columns: request.grid_settings.columns,
        cell_width: request.grid_settings.cell_width,
        cell_height: request.grid_settings.cell_height,
        border_left: request.grid_settings.border_left,
        border_top: request.grid_settings.border_top,
        border_right: request.grid_settings.border_right,
        border_bottom: request.grid_settings.border_bottom,
        gap_x: request.grid_settings.gap_x,
        gap_y: request.grid_settings.gap_y,
        read_order: request.grid_settings.read_order,
        empty_cell_threshold: request.grid_settings.empty_cell_threshold,
    })?;

    Ok(SheetPreviewResult {
        cells: analysis.cells,
        empty_cell_candidates: analysis.empty_cell_candidates,
        warnings: analysis.warnings,
    })
}
