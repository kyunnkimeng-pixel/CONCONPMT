use image::RgbaImage;
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::imaging::import_limits::{
    decode_import_image, validate_import_dimensions, MAX_IMPORT_DIMENSION,
};

use super::{image_format_for_extension, read_sheet_image_input};
use crate::models::ImportImageFilePayload;

pub const MAX_SHEET_CELLS: i64 = 10_000;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetGridAnalyzeRequest {
    pub sheet_path: Option<String>,
    pub sheet_file: Option<ImportImageFilePayload>,
    pub mode: String,
    pub rows: Option<i64>,
    pub columns: Option<i64>,
    pub cell_width: Option<i64>,
    pub cell_height: Option<i64>,
    #[serde(default)]
    pub border_left: i64,
    #[serde(default)]
    pub border_top: i64,
    #[serde(default)]
    pub border_right: i64,
    #[serde(default)]
    pub border_bottom: i64,
    #[serde(default)]
    pub gap_x: i64,
    #[serde(default)]
    pub gap_y: i64,
    #[serde(default = "default_read_order")]
    pub read_order: String,
    pub empty_cell_threshold: Option<f64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetGridSettings {
    pub mode: String,
    pub rows: Option<i64>,
    pub columns: Option<i64>,
    pub cell_width: Option<i64>,
    pub cell_height: Option<i64>,
    #[serde(default)]
    pub border_left: i64,
    #[serde(default)]
    pub border_top: i64,
    #[serde(default)]
    pub border_right: i64,
    #[serde(default)]
    pub border_bottom: i64,
    #[serde(default)]
    pub gap_x: i64,
    #[serde(default)]
    pub gap_y: i64,
    #[serde(default = "default_read_order")]
    pub read_order: String,
    pub empty_cell_threshold: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetGridAnalysis {
    pub sheet_width: i64,
    pub sheet_height: i64,
    pub computed_rows: i64,
    pub computed_columns: i64,
    pub cell_count: i64,
    pub out_of_bounds_cells: Vec<i64>,
    pub empty_cell_candidates: Vec<i64>,
    pub cells: Vec<SheetCell>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SheetCell {
    pub index: i64,
    pub page: i64,
    pub row: i64,
    pub col: i64,
    pub x: i64,
    pub y: i64,
    pub w: i64,
    pub h: i64,
    pub out_of_bounds: bool,
    pub empty_candidate: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadOrder {
    RowMajor,
    ColumnMajor,
}

#[derive(Debug, Clone, Copy)]
pub struct ResolvedGrid {
    pub rows: i64,
    pub columns: i64,
    pub cell_width: i64,
    pub cell_height: i64,
    pub border_left: i64,
    pub border_top: i64,
    pub gap_x: i64,
    pub gap_y: i64,
    pub read_order: ReadOrder,
    pub empty_cell_threshold: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageSplitSettings {
    pub cell_width: i64,
    pub cell_height: i64,
    pub columns: i64,
    pub gap_x: i64,
    pub gap_y: i64,
    pub border_x: i64,
    pub border_y: i64,
    pub max_sheet_width: i64,
    pub max_sheet_height: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageCellPlacement {
    pub item_index: usize,
    pub page_index: i64,
    pub row: i64,
    pub col: i64,
    pub x: i64,
    pub y: i64,
    pub w: i64,
    pub h: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PagePlan {
    pub page_index: i64,
    pub item_count: i64,
    pub columns: i64,
    pub rows: i64,
    pub width: i64,
    pub height: i64,
}

#[derive(Debug, Clone)]
pub struct PageSplitPlan {
    pub columns_per_page: i64,
    pub rows_per_page: i64,
    pub pages: Vec<PagePlan>,
    pub placements: Vec<PageCellPlacement>,
    pub warnings: Vec<String>,
}

pub fn analyze_sheet_grid(request: SheetGridAnalyzeRequest) -> AppResult<SheetGridAnalysis> {
    let source = read_sheet_image_input(
        request.sheet_path.as_deref(),
        request.sheet_file.as_ref(),
        false,
    )?;
    let format = image_format_for_extension(&source.extension)?;
    let image = decode_import_image(&source.bytes, format)?.to_rgba8();
    let (sheet_width, sheet_height) = (i64::from(image.width()), i64::from(image.height()));
    let settings = SheetGridSettings {
        mode: request.mode,
        rows: request.rows,
        columns: request.columns,
        cell_width: request.cell_width,
        cell_height: request.cell_height,
        border_left: request.border_left,
        border_top: request.border_top,
        border_right: request.border_right,
        border_bottom: request.border_bottom,
        gap_x: request.gap_x,
        gap_y: request.gap_y,
        read_order: request.read_order,
        empty_cell_threshold: request.empty_cell_threshold,
    };
    let mut analysis = analyze_rgba_grid(&image, &settings, sheet_width, sheet_height)?;
    if let Some(warning) = alpha_warning_for_extension(&source.extension) {
        analysis.warnings.push(warning.to_string());
    }
    Ok(analysis)
}

pub(crate) fn alpha_warning_for_extension(extension: &str) -> Option<&'static str> {
    match extension {
        "jpg" | "jpeg" => Some("JPG/JPEG 시트는 alpha 투명도를 포함하지 않습니다. 투명 배경이 필요한 작업 시트에는 PNG를 사용하세요."),
        _ => None,
    }
}

pub fn analyze_rgba_grid(
    image: &RgbaImage,
    settings: &SheetGridSettings,
    sheet_width: i64,
    sheet_height: i64,
) -> AppResult<SheetGridAnalysis> {
    let resolved = resolve_grid(settings, sheet_width, sheet_height)?;
    let cells = calculate_cells(&resolved, sheet_width, sheet_height, Some(image));
    let out_of_bounds_cells = cells
        .iter()
        .filter(|cell| cell.out_of_bounds)
        .map(|cell| cell.index)
        .collect::<Vec<_>>();
    let empty_cell_candidates = cells
        .iter()
        .filter(|cell| cell.empty_candidate)
        .map(|cell| cell.index)
        .collect::<Vec<_>>();
    let mut warnings = Vec::new();

    if !out_of_bounds_cells.is_empty() {
        warnings.push("일부 셀이 시트 이미지 밖으로 나갑니다.".to_string());
    }
    if empty_cell_candidates.is_empty() {
        warnings.push("투명 기준으로 자동 감지된 빈 셀 후보가 없습니다.".to_string());
    }

    Ok(SheetGridAnalysis {
        sheet_width,
        sheet_height,
        computed_rows: resolved.rows,
        computed_columns: resolved.columns,
        cell_count: resolved.rows * resolved.columns,
        out_of_bounds_cells,
        empty_cell_candidates,
        cells,
        warnings,
    })
}

#[allow(dead_code)]
pub fn cells_for_settings(
    settings: &SheetGridSettings,
    sheet_width: i64,
    sheet_height: i64,
) -> AppResult<Vec<SheetCell>> {
    let resolved = resolve_grid(settings, sheet_width, sheet_height)?;
    Ok(calculate_cells(&resolved, sheet_width, sheet_height, None))
}

pub fn resolve_grid(
    settings: &SheetGridSettings,
    sheet_width: i64,
    sheet_height: i64,
) -> AppResult<ResolvedGrid> {
    let sheet_width_u32 = u32::try_from(sheet_width)
        .map_err(|_| AppError::new("validation", "시트 너비가 올바르지 않습니다."))?;
    let sheet_height_u32 = u32::try_from(sheet_height)
        .map_err(|_| AppError::new("validation", "시트 높이가 올바르지 않습니다."))?;
    validate_import_dimensions(sheet_width_u32, sheet_height_u32)?;
    validate_nonnegative_extent(settings.border_left, "왼쪽 여백")?;
    validate_nonnegative_extent(settings.border_top, "위쪽 여백")?;
    validate_nonnegative_extent(settings.border_right, "오른쪽 여백")?;
    validate_nonnegative_extent(settings.border_bottom, "아래쪽 여백")?;
    validate_nonnegative_extent(settings.gap_x, "가로 간격")?;
    validate_nonnegative_extent(settings.gap_y, "세로 간격")?;

    let border_left = settings.border_left.max(0);
    let border_top = settings.border_top.max(0);
    let border_right = settings.border_right.max(0);
    let border_bottom = settings.border_bottom.max(0);
    let gap_x = settings.gap_x.max(0);
    let gap_y = settings.gap_y.max(0);
    let available_width = sheet_width
        .checked_sub(border_left)
        .and_then(|value| value.checked_sub(border_right))
        .ok_or_else(|| AppError::new("validation", "가로 여백 계산이 지원 범위를 벗어났습니다."))?;
    let available_height = sheet_height
        .checked_sub(border_top)
        .and_then(|value| value.checked_sub(border_bottom))
        .ok_or_else(|| AppError::new("validation", "세로 여백 계산이 지원 범위를 벗어났습니다."))?;

    if available_width <= 0 || available_height <= 0 {
        return Err(AppError::new(
            "validation",
            "시트 여백이 이미지 크기보다 큽니다.",
        ));
    }

    let (rows, columns, cell_width, cell_height) = match settings.mode.as_str() {
        "cell_size" => {
            let cell_width = positive_value(settings.cell_width, "셀 너비")?;
            let cell_height = positive_value(settings.cell_height, "셀 높이")?;
            let columns = settings
                .columns
                .filter(|value| *value > 0)
                .unwrap_or_else(|| infer_count(available_width, cell_width, gap_x));
            let rows = settings
                .rows
                .filter(|value| *value > 0)
                .unwrap_or_else(|| infer_count(available_height, cell_height, gap_y));
            validate_grid_cell_count(rows, columns)?;
            (rows, columns, cell_width, cell_height)
        }
        _ => {
            let rows = positive_value(settings.rows, "행")?;
            let columns = positive_value(settings.columns, "열")?;
            validate_grid_cell_count(rows, columns)?;
            let cell_width = settings
                .cell_width
                .filter(|value| *value > 0)
                .unwrap_or_else(|| divide_grid_extent(available_width, columns, gap_x));
            let cell_height = settings
                .cell_height
                .filter(|value| *value > 0)
                .unwrap_or_else(|| divide_grid_extent(available_height, rows, gap_y));
            (rows, columns, cell_width, cell_height)
        }
    };

    if rows <= 0 || columns <= 0 || cell_width <= 0 || cell_height <= 0 {
        return Err(AppError::new(
            "validation",
            "행, 열, 셀 크기는 모두 1 이상이어야 합니다.",
        ));
    }
    validate_grid_dimensions(cell_width, cell_height)?;

    Ok(ResolvedGrid {
        rows,
        columns,
        cell_width,
        cell_height,
        border_left,
        border_top,
        gap_x,
        gap_y,
        read_order: parse_read_order(&settings.read_order)?,
        empty_cell_threshold: settings
            .empty_cell_threshold
            .unwrap_or(0.98)
            .clamp(0.0, 1.0),
    })
}

pub fn calculate_cells(
    grid: &ResolvedGrid,
    sheet_width: i64,
    sheet_height: i64,
    image: Option<&RgbaImage>,
) -> Vec<SheetCell> {
    let mut cells = Vec::with_capacity((grid.rows * grid.columns).max(0) as usize);

    for logical_index in 0..(grid.rows * grid.columns) {
        let (row, col) = match grid.read_order {
            ReadOrder::RowMajor => (logical_index / grid.columns, logical_index % grid.columns),
            ReadOrder::ColumnMajor => (logical_index % grid.rows, logical_index / grid.rows),
        };
        let x = grid.border_left + col * (grid.cell_width + grid.gap_x);
        let y = grid.border_top + row * (grid.cell_height + grid.gap_y);
        let out_of_bounds = x < 0
            || y < 0
            || x + grid.cell_width > sheet_width
            || y + grid.cell_height > sheet_height;
        let empty_candidate = image.filter(|_| !out_of_bounds).is_some_and(|image| {
            is_empty_alpha_cell(
                image,
                x,
                y,
                grid.cell_width,
                grid.cell_height,
                grid.empty_cell_threshold,
            )
        });

        cells.push(SheetCell {
            index: logical_index,
            page: 0,
            row,
            col,
            x,
            y,
            w: grid.cell_width,
            h: grid.cell_height,
            out_of_bounds,
            empty_candidate,
        });
    }

    cells
}

pub fn split_pages(item_count: usize, settings: PageSplitSettings) -> AppResult<PageSplitPlan> {
    validate_page_split_settings(item_count, &settings)?;
    if item_count == 0 {
        return Ok(PageSplitPlan {
            columns_per_page: 0,
            rows_per_page: 0,
            pages: Vec::new(),
            placements: Vec::new(),
            warnings: Vec::new(),
        });
    }

    let cell_width = settings.cell_width.max(1);
    let cell_height = settings.cell_height.max(1);
    let gap_x = settings.gap_x.max(0);
    let gap_y = settings.gap_y.max(0);
    let border_x = settings.border_x.max(0);
    let border_y = settings.border_y.max(0);
    let max_sheet_width = settings.max_sheet_width.max(cell_width + border_x * 2);
    let max_sheet_height = settings.max_sheet_height.max(cell_height + border_y * 2);
    let requested_columns = settings.columns.max(1);
    let max_columns_by_width = infer_count(max_sheet_width - border_x * 2, cell_width, gap_x);
    let columns_per_page = requested_columns.min(max_columns_by_width.max(1));
    let rows_per_page = infer_count(max_sheet_height - border_y * 2, cell_height, gap_y).max(1);
    let items_per_page = (columns_per_page * rows_per_page).max(1) as usize;
    let mut warnings = Vec::new();

    if columns_per_page < requested_columns {
        warnings.push("최대 시트 너비에 맞추기 위해 페이지당 열 수를 줄였습니다.".to_string());
    }

    let mut pages = Vec::new();
    let mut placements = Vec::new();
    for (page_index, page_items) in (0..item_count)
        .collect::<Vec<_>>()
        .chunks(items_per_page)
        .enumerate()
    {
        let page_index = page_index as i64;
        let item_count_on_page = page_items.len() as i64;
        let rows = ((item_count_on_page + columns_per_page - 1) / columns_per_page).max(1);
        let width = sheet_extent(columns_per_page, cell_width, gap_x, border_x);
        let height = sheet_extent(rows, cell_height, gap_y, border_y);
        let width_u32 = u32::try_from(width)
            .map_err(|_| AppError::new("validation", "시트 페이지 너비가 올바르지 않습니다."))?;
        let height_u32 = u32::try_from(height)
            .map_err(|_| AppError::new("validation", "시트 페이지 높이가 올바르지 않습니다."))?;
        validate_import_dimensions(width_u32, height_u32)?;

        pages.push(PagePlan {
            page_index,
            item_count: item_count_on_page,
            columns: columns_per_page,
            rows,
            width,
            height,
        });

        for (local_index, item_index) in page_items.iter().enumerate() {
            let local_index = local_index as i64;
            let row = local_index / columns_per_page;
            let col = local_index % columns_per_page;
            placements.push(PageCellPlacement {
                item_index: *item_index,
                page_index,
                row,
                col,
                x: border_x + col * (cell_width + gap_x),
                y: border_y + row * (cell_height + gap_y),
                w: cell_width,
                h: cell_height,
            });
        }
    }

    Ok(PageSplitPlan {
        columns_per_page,
        rows_per_page,
        pages,
        placements,
        warnings,
    })
}

fn validate_grid_dimensions(width: i64, height: i64) -> AppResult<()> {
    let width = u32::try_from(width)
        .map_err(|_| AppError::new("validation", "셀 너비가 올바르지 않습니다."))?;
    let height = u32::try_from(height)
        .map_err(|_| AppError::new("validation", "셀 높이가 올바르지 않습니다."))?;
    validate_import_dimensions(width, height)
}

fn validate_nonnegative_extent(value: i64, label: &str) -> AppResult<()> {
    if !(0..=i64::from(MAX_IMPORT_DIMENSION)).contains(&value) {
        return Err(AppError::new(
            "validation",
            format!("{label}은 0~{MAX_IMPORT_DIMENSION}px 범위여야 합니다."),
        ));
    }
    Ok(())
}

fn validate_page_split_settings(item_count: usize, settings: &PageSplitSettings) -> AppResult<()> {
    if item_count > usize::try_from(MAX_SHEET_CELLS).unwrap_or(usize::MAX) {
        return Err(AppError::new(
            "validation",
            format!("시트는 최대 {MAX_SHEET_CELLS}개 셀까지 분할할 수 있습니다."),
        ));
    }
    validate_grid_dimensions(settings.cell_width, settings.cell_height)?;
    if !(1..=MAX_SHEET_CELLS).contains(&settings.columns) {
        return Err(AppError::new(
            "validation",
            format!("시트 열 수는 1 이상 {MAX_SHEET_CELLS} 이하여야 합니다."),
        ));
    }
    validate_nonnegative_extent(settings.gap_x, "가로 간격")?;
    validate_nonnegative_extent(settings.gap_y, "세로 간격")?;
    validate_nonnegative_extent(settings.border_x, "가로 테두리")?;
    validate_nonnegative_extent(settings.border_y, "세로 테두리")?;
    let max_width = u32::try_from(settings.max_sheet_width)
        .map_err(|_| AppError::new("validation", "최대 시트 너비가 올바르지 않습니다."))?;
    let max_height = u32::try_from(settings.max_sheet_height)
        .map_err(|_| AppError::new("validation", "최대 시트 높이가 올바르지 않습니다."))?;
    if max_width == 0
        || max_height == 0
        || max_width > MAX_IMPORT_DIMENSION
        || max_height > MAX_IMPORT_DIMENSION
    {
        return Err(AppError::new(
            "validation",
            "최대 시트 크기는 한 변 1~12,000px 범위여야 합니다.",
        ));
    }
    Ok(())
}

fn positive_value(value: Option<i64>, label: &str) -> AppResult<i64> {
    value
        .filter(|value| *value > 0)
        .ok_or_else(|| AppError::new("validation", format!("{label} 값이 필요합니다.")))
}

fn infer_count(available: i64, cell: i64, gap: i64) -> i64 {
    if available <= 0 || cell <= 0 {
        return 0;
    }
    (available.saturating_add(gap) / cell.saturating_add(gap).max(1)).max(1)
}

fn divide_grid_extent(available: i64, count: i64, gap: i64) -> i64 {
    (available.saturating_sub(gap.saturating_mul((count - 1).max(0))) / count.max(1)).max(1)
}

fn validate_grid_cell_count(rows: i64, columns: i64) -> AppResult<()> {
    let cell_count = rows.checked_mul(columns).ok_or_else(|| {
        AppError::new(
            "validation",
            "시트 행·열 조합이 너무 큽니다. 분할 설정을 줄여 주세요.",
        )
    })?;
    if cell_count > MAX_SHEET_CELLS {
        return Err(AppError::new(
            "validation",
            format!("시트는 최대 {MAX_SHEET_CELLS}개 셀까지 분석할 수 있습니다."),
        ));
    }
    Ok(())
}

fn sheet_extent(count: i64, cell: i64, gap: i64, border: i64) -> i64 {
    border * 2 + count * cell + (count - 1).max(0) * gap
}

fn parse_read_order(value: &str) -> AppResult<ReadOrder> {
    match value {
        "column_major" => Ok(ReadOrder::ColumnMajor),
        "row_major" | "" => Ok(ReadOrder::RowMajor),
        _ => Err(AppError::new(
            "validation",
            "지원하지 않는 시트 읽기 순서입니다.",
        )),
    }
}

fn is_empty_alpha_cell(
    image: &RgbaImage,
    x: i64,
    y: i64,
    width: i64,
    height: i64,
    threshold: f64,
) -> bool {
    let width = width.max(1) as u32;
    let height = height.max(1) as u32;
    let mut transparent = 0_u64;
    let mut has_alpha = false;

    for yy in 0..height {
        for xx in 0..width {
            let pixel = image.get_pixel((x as u32) + xx, (y as u32) + yy);
            if pixel.0[3] < 255 {
                has_alpha = true;
            }
            if pixel.0[3] <= 4 {
                transparent += 1;
            }
        }
    }

    has_alpha && (transparent as f64 / f64::from(width * height)) >= threshold
}

fn default_read_order() -> String {
    "row_major".to_string()
}

#[cfg(test)]
mod tests {
    use image::{ImageBuffer, Rgba};

    use super::{
        analyze_rgba_grid, resolve_grid, split_pages, PageSplitSettings, SheetGridSettings,
    };

    #[test]
    fn rows_columns_grid_uses_border_gap_and_row_major_order() {
        let settings = SheetGridSettings {
            mode: "rows_columns".to_string(),
            rows: Some(2),
            columns: Some(3),
            cell_width: Some(10),
            cell_height: Some(8),
            border_left: 2,
            border_top: 3,
            border_right: 0,
            border_bottom: 0,
            gap_x: 1,
            gap_y: 2,
            read_order: "row_major".to_string(),
            empty_cell_threshold: None,
        };
        let cells = super::cells_for_settings(&settings, 40, 30).unwrap();

        assert_eq!((cells[0].x, cells[0].y), (2, 3));
        assert_eq!((cells[1].x, cells[1].y), (13, 3));
        assert_eq!((cells[3].x, cells[3].y), (2, 13));
    }

    #[test]
    fn cell_size_mode_infers_rows_and_columns() {
        let settings = SheetGridSettings {
            mode: "cell_size".to_string(),
            rows: None,
            columns: None,
            cell_width: Some(10),
            cell_height: Some(10),
            border_left: 1,
            border_top: 1,
            border_right: 1,
            border_bottom: 1,
            gap_x: 2,
            gap_y: 2,
            read_order: "row_major".to_string(),
            empty_cell_threshold: None,
        };
        let grid = resolve_grid(&settings, 38, 26).unwrap();

        assert_eq!(grid.columns, 3);
        assert_eq!(grid.rows, 2);
    }

    #[test]
    fn cell_size_mode_keeps_one_preview_cell_when_extent_is_smaller_than_cell() {
        let settings = SheetGridSettings {
            mode: "cell_size".to_string(),
            rows: None,
            columns: Some(5),
            cell_width: Some(200),
            cell_height: Some(200),
            border_left: 16,
            border_top: 16,
            border_right: 16,
            border_bottom: 16,
            gap_x: 8,
            gap_y: 8,
            read_order: "row_major".to_string(),
            empty_cell_threshold: None,
        };
        let grid = resolve_grid(&settings, 600, 200).unwrap();

        assert_eq!(grid.columns, 5);
        assert_eq!(grid.rows, 1);
    }

    #[test]
    fn column_major_order_calculates_top_to_bottom_first() {
        let settings = SheetGridSettings {
            mode: "rows_columns".to_string(),
            rows: Some(2),
            columns: Some(2),
            cell_width: Some(10),
            cell_height: Some(10),
            border_left: 0,
            border_top: 0,
            border_right: 0,
            border_bottom: 0,
            gap_x: 0,
            gap_y: 0,
            read_order: "column_major".to_string(),
            empty_cell_threshold: None,
        };
        let cells = super::cells_for_settings(&settings, 20, 20).unwrap();

        assert_eq!((cells[1].row, cells[1].col), (1, 0));
        assert_eq!((cells[2].row, cells[2].col), (0, 1));
    }

    #[test]
    fn grid_rejects_excessive_or_overflowing_cell_counts() {
        let settings = SheetGridSettings {
            mode: "rows_columns".to_string(),
            rows: Some(101),
            columns: Some(100),
            cell_width: Some(1),
            cell_height: Some(1),
            border_left: 0,
            border_top: 0,
            border_right: 0,
            border_bottom: 0,
            gap_x: 0,
            gap_y: 0,
            read_order: "row_major".to_string(),
            empty_cell_threshold: None,
        };
        assert!(resolve_grid(&settings, 200, 200).is_err());

        let overflowing = SheetGridSettings {
            rows: Some(i64::MAX),
            columns: Some(i64::MAX),
            ..settings.clone()
        };
        assert!(resolve_grid(&overflowing, 200, 200).is_err());

        let extreme_layout = SheetGridSettings {
            rows: Some(1),
            columns: Some(1),
            border_left: i64::MAX,
            gap_x: i64::MAX,
            ..settings
        };
        assert!(resolve_grid(&extreme_layout, 200, 200).is_err());
    }

    #[test]
    fn alpha_empty_cells_are_detected() {
        let mut image = ImageBuffer::from_pixel(20, 10, Rgba([255, 0, 0, 255]));
        for y in 0..10 {
            for x in 10..20 {
                image.put_pixel(x, y, Rgba([0, 0, 0, 0]));
            }
        }
        let settings = SheetGridSettings {
            mode: "rows_columns".to_string(),
            rows: Some(1),
            columns: Some(2),
            cell_width: Some(10),
            cell_height: Some(10),
            border_left: 0,
            border_top: 0,
            border_right: 0,
            border_bottom: 0,
            gap_x: 0,
            gap_y: 0,
            read_order: "row_major".to_string(),
            empty_cell_threshold: Some(0.98),
        };
        let analysis = analyze_rgba_grid(&image, &settings, 20, 10).unwrap();

        assert_eq!(analysis.empty_cell_candidates, vec![1]);
    }

    #[test]
    fn page_splitting_rejects_extreme_extents_before_planning() {
        assert!(split_pages(
            1,
            PageSplitSettings {
                cell_width: 1,
                cell_height: 1,
                columns: 1,
                gap_x: i64::MAX,
                gap_y: 0,
                border_x: 0,
                border_y: 0,
                max_sheet_width: 1,
                max_sheet_height: 1,
            },
        )
        .is_err());
    }

    #[test]
    fn page_splitting_caps_columns_and_maps_cells() {
        let plan = split_pages(
            10,
            PageSplitSettings {
                cell_width: 100,
                cell_height: 100,
                columns: 5,
                gap_x: 10,
                gap_y: 10,
                border_x: 10,
                border_y: 10,
                max_sheet_width: 350,
                max_sheet_height: 250,
            },
        )
        .unwrap();

        assert_eq!(plan.columns_per_page, 3);
        assert_eq!(plan.rows_per_page, 2);
        assert_eq!(plan.pages.len(), 2);
        assert_eq!(plan.placements[3].page_index, 0);
        assert_eq!((plan.placements[3].row, plan.placements[3].col), (1, 0));
        assert_eq!(plan.placements[6].page_index, 1);
    }
}
