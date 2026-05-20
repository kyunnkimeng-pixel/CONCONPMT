use image::RgbaImage;
use serde::{Deserialize, Serialize};

use crate::error::AppResult;
use crate::models::ImportImageFilePayload;

use super::grid::{analyze_rgba_grid, SheetGridSettings};
use super::{image_format_for_extension, read_sheet_image_input};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoDetectSheetGridRequest {
    pub sheet_path: Option<String>,
    pub sheet_file: Option<ImportImageFilePayload>,
    pub alpha_separator_threshold: Option<f64>,
    pub background_separator_threshold: Option<f64>,
    pub background_tolerance: Option<u8>,
    pub min_cell_width: Option<i64>,
    pub min_cell_height: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoDetectSheetGridResult {
    pub sheet_width: i64,
    pub sheet_height: i64,
    pub has_alpha: bool,
    pub proposals: Vec<AutoDetectSheetGridProposal>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoDetectSheetGridProposal {
    pub id: String,
    pub label: String,
    pub method: String,
    pub confidence: String,
    pub confidence_score: f64,
    pub grid_settings: SheetGridSettings,
    pub computed_rows: i64,
    pub computed_columns: i64,
    pub cell_count: i64,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
struct Band {
    start: i64,
    end: i64,
}

pub fn auto_detect_sheet_grid(
    request: AutoDetectSheetGridRequest,
) -> AppResult<AutoDetectSheetGridResult> {
    let source = read_sheet_image_input(
        request.sheet_path.as_deref(),
        request.sheet_file.as_ref(),
        false,
    )?;
    let format = image_format_for_extension(&source.extension)?;
    let image = image::load_from_memory_with_format(&source.bytes, format)?.to_rgba8();
    let sheet_width = i64::from(image.width());
    let sheet_height = i64::from(image.height());
    let has_alpha = image.pixels().any(|pixel| pixel.0[3] < 255);
    let min_cell_width = request.min_cell_width.unwrap_or(16).max(1);
    let min_cell_height = request.min_cell_height.unwrap_or(16).max(1);
    let alpha_threshold = request
        .alpha_separator_threshold
        .unwrap_or(0.98)
        .clamp(0.5, 1.0);
    let background_threshold = request
        .background_separator_threshold
        .unwrap_or(0.98)
        .clamp(0.5, 1.0);
    let background_tolerance = request.background_tolerance.unwrap_or(8);

    let mut proposals = Vec::new();
    let mut warnings = Vec::new();

    if has_alpha {
        let column_separators = alpha_axis_separators(&image, Axis::Column, alpha_threshold);
        let row_separators = alpha_axis_separators(&image, Axis::Row, alpha_threshold);
        maybe_add_proposal(
            &mut proposals,
            &image,
            "alpha",
            "투명 여백 감지",
            &column_separators,
            &row_separators,
            min_cell_width,
            min_cell_height,
        )?;
    } else {
        warnings.push("alpha 채널이 없어 투명 separator 기반 감지는 건너뛰었습니다.".to_string());
    }

    let background = estimate_background_color(&image);
    let column_separators = background_axis_separators(
        &image,
        Axis::Column,
        background,
        background_tolerance,
        background_threshold,
    );
    let row_separators = background_axis_separators(
        &image,
        Axis::Row,
        background,
        background_tolerance,
        background_threshold,
    );
    maybe_add_proposal(
        &mut proposals,
        &image,
        "solid_background",
        "단색 배경 separator 감지",
        &column_separators,
        &row_separators,
        min_cell_width,
        min_cell_height,
    )?;

    if proposals.is_empty() {
        warnings.push(
            "신뢰할 수 있는 grid 제안을 찾지 못했습니다. Grid/셀 크기/직접 Slice를 사용하세요."
                .to_string(),
        );
    } else {
        warnings.push(
            "자동 감지는 실험 기능입니다. 제안값을 적용한 뒤 grid overlay와 셀 검토에서 반드시 확인하세요."
                .to_string(),
        );
    }

    Ok(AutoDetectSheetGridResult {
        sheet_width,
        sheet_height,
        has_alpha,
        proposals,
        warnings,
    })
}

fn maybe_add_proposal(
    proposals: &mut Vec<AutoDetectSheetGridProposal>,
    image: &RgbaImage,
    method: &str,
    label: &str,
    column_separators: &[bool],
    row_separators: &[bool],
    min_cell_width: i64,
    min_cell_height: i64,
) -> AppResult<()> {
    let columns = bands_from_separators(column_separators, min_cell_width);
    let rows = bands_from_separators(row_separators, min_cell_height);
    if columns.is_empty() || rows.is_empty() || columns.len() * rows.len() < 2 {
        return Ok(());
    }

    let column_widths = columns.iter().map(Band::len).collect::<Vec<_>>();
    let row_heights = rows.iter().map(Band::len).collect::<Vec<_>>();
    let gap_x_values = gaps_between_bands(&columns);
    let gap_y_values = gaps_between_bands(&rows);
    let cell_width = median_i64(&column_widths);
    let cell_height = median_i64(&row_heights);
    let gap_x = median_i64_or_zero(&gap_x_values);
    let gap_y = median_i64_or_zero(&gap_y_values);
    let border_left = columns.first().map(|band| band.start).unwrap_or(0).max(0);
    let border_top = rows.first().map(|band| band.start).unwrap_or(0).max(0);
    let border_right =
        i64::from(image.width()) - columns.last().map(|band| band.end + 1).unwrap_or(0);
    let border_bottom =
        i64::from(image.height()) - rows.last().map(|band| band.end + 1).unwrap_or(0);

    let settings = SheetGridSettings {
        mode: "rows_columns".to_string(),
        rows: Some(rows.len() as i64),
        columns: Some(columns.len() as i64),
        cell_width: Some(cell_width),
        cell_height: Some(cell_height),
        border_left,
        border_top,
        border_right: border_right.max(0),
        border_bottom: border_bottom.max(0),
        gap_x,
        gap_y,
        read_order: "row_major".to_string(),
        empty_cell_threshold: Some(0.98),
    };

    if proposals
        .iter()
        .any(|proposal| same_grid_settings(&proposal.grid_settings, &settings))
    {
        return Ok(());
    }

    let analysis = analyze_rgba_grid(
        image,
        &settings,
        i64::from(image.width()),
        i64::from(image.height()),
    )?;
    let score = confidence_score(
        &column_widths,
        &row_heights,
        &gap_x_values,
        &gap_y_values,
        analysis.out_of_bounds_cells.len(),
    );
    let confidence = if score >= 0.8 {
        "high"
    } else if score >= 0.55 {
        "medium"
    } else {
        "low"
    };

    let mut warnings = analysis.warnings;
    if confidence != "high" {
        warnings.push("제안 신뢰도가 높지 않습니다. overlay에서 위치를 조정하세요.".to_string());
    }

    proposals.push(AutoDetectSheetGridProposal {
        id: format!("{}_{}x{}", method, rows.len(), columns.len()),
        label: label.to_string(),
        method: method.to_string(),
        confidence: confidence.to_string(),
        confidence_score: score,
        grid_settings: settings,
        computed_rows: rows.len() as i64,
        computed_columns: columns.len() as i64,
        cell_count: (rows.len() * columns.len()) as i64,
        warnings,
    });

    proposals.sort_by(|a, b| {
        b.confidence_score
            .partial_cmp(&a.confidence_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok(())
}

#[derive(Debug, Clone, Copy)]
enum Axis {
    Column,
    Row,
}

fn alpha_axis_separators(image: &RgbaImage, axis: Axis, threshold: f64) -> Vec<bool> {
    axis_ratios(image, axis, |pixel| pixel.0[3] <= 4)
        .into_iter()
        .map(|ratio| ratio >= threshold)
        .collect()
}

fn background_axis_separators(
    image: &RgbaImage,
    axis: Axis,
    background: [u8; 4],
    tolerance: u8,
    threshold: f64,
) -> Vec<bool> {
    axis_ratios(image, axis, |pixel| {
        color_within_tolerance(pixel.0, background, tolerance)
    })
    .into_iter()
    .map(|ratio| ratio >= threshold)
    .collect()
}

fn axis_ratios(
    image: &RgbaImage,
    axis: Axis,
    matches: impl Fn(&image::Rgba<u8>) -> bool,
) -> Vec<f64> {
    let (width, height) = (image.width(), image.height());
    let count = match axis {
        Axis::Column => width,
        Axis::Row => height,
    };
    let span = match axis {
        Axis::Column => height,
        Axis::Row => width,
    }
    .max(1);

    (0..count)
        .map(|primary| {
            let mut matched = 0_u32;
            for secondary in 0..span {
                let pixel = match axis {
                    Axis::Column => image.get_pixel(primary, secondary),
                    Axis::Row => image.get_pixel(secondary, primary),
                };
                if matches(pixel) {
                    matched += 1;
                }
            }
            f64::from(matched) / f64::from(span)
        })
        .collect()
}

fn bands_from_separators(separators: &[bool], min_len: i64) -> Vec<Band> {
    let mut bands = Vec::new();
    let mut start: Option<i64> = None;
    for (index, separator) in separators.iter().enumerate() {
        if !separator && start.is_none() {
            start = Some(index as i64);
        }
        if *separator {
            if let Some(band_start) = start.take() {
                let end = index as i64 - 1;
                if end - band_start + 1 >= min_len {
                    bands.push(Band {
                        start: band_start,
                        end,
                    });
                }
            }
        }
    }
    if let Some(band_start) = start {
        let end = separators.len() as i64 - 1;
        if end - band_start + 1 >= min_len {
            bands.push(Band {
                start: band_start,
                end,
            });
        }
    }
    bands
}

fn gaps_between_bands(bands: &[Band]) -> Vec<i64> {
    bands
        .windows(2)
        .map(|pair| (pair[1].start - pair[0].end - 1).max(0))
        .collect()
}

impl Band {
    fn len(&self) -> i64 {
        self.end - self.start + 1
    }
}

fn median_i64(values: &[i64]) -> i64 {
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    sorted[sorted.len() / 2]
}

fn median_i64_or_zero(values: &[i64]) -> i64 {
    if values.is_empty() {
        0
    } else {
        median_i64(values)
    }
}

fn confidence_score(
    column_widths: &[i64],
    row_heights: &[i64],
    gap_x_values: &[i64],
    gap_y_values: &[i64],
    out_of_bounds_count: usize,
) -> f64 {
    let width_score = uniformity_score(column_widths);
    let height_score = uniformity_score(row_heights);
    let gap_x_score = if gap_x_values.is_empty() {
        0.8
    } else {
        uniformity_score(gap_x_values)
    };
    let gap_y_score = if gap_y_values.is_empty() {
        0.8
    } else {
        uniformity_score(gap_y_values)
    };
    let bounds_score = if out_of_bounds_count == 0 { 1.0 } else { 0.2 };
    ((width_score + height_score) * 0.3 + (gap_x_score + gap_y_score) * 0.15 + bounds_score * 0.1)
        .clamp(0.0, 1.0)
}

fn uniformity_score(values: &[i64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    if values.len() == 1 {
        return 0.85;
    }
    let median = median_i64(values).max(1) as f64;
    let max_deviation = values
        .iter()
        .map(|value| ((*value as f64 - median).abs()) / median)
        .fold(0.0, f64::max);
    (1.0 - max_deviation.min(1.0)).clamp(0.0, 1.0)
}

fn estimate_background_color(image: &RgbaImage) -> [u8; 4] {
    let width = image.width().saturating_sub(1);
    let height = image.height().saturating_sub(1);
    let samples = [
        image.get_pixel(0, 0).0,
        image.get_pixel(width, 0).0,
        image.get_pixel(0, height).0,
        image.get_pixel(width, height).0,
    ];
    let mut result = [0_u8; 4];
    for channel in 0..4 {
        let mut values = samples
            .iter()
            .map(|sample| sample[channel])
            .collect::<Vec<_>>();
        values.sort_unstable();
        result[channel] = values[values.len() / 2];
    }
    result
}

fn color_within_tolerance(actual: [u8; 4], expected: [u8; 4], tolerance: u8) -> bool {
    actual
        .iter()
        .zip(expected.iter())
        .all(|(actual, expected)| actual.abs_diff(*expected) <= tolerance)
}

fn same_grid_settings(left: &SheetGridSettings, right: &SheetGridSettings) -> bool {
    left.rows == right.rows
        && left.columns == right.columns
        && left.cell_width == right.cell_width
        && left.cell_height == right.cell_height
        && left.border_left == right.border_left
        && left.border_top == right.border_top
        && left.border_right == right.border_right
        && left.border_bottom == right.border_bottom
        && left.gap_x == right.gap_x
        && left.gap_y == right.gap_y
}

#[cfg(test)]
mod tests {
    use image::{ImageBuffer, Rgba};

    use super::{auto_detect_sheet_grid, AutoDetectSheetGridRequest};
    use crate::models::ImportImageFilePayload;

    #[test]
    fn detects_transparent_separator_grid() {
        let mut image = ImageBuffer::from_pixel(34, 22, Rgba([0, 0, 0, 0]));
        fill_rect(&mut image, 2, 3, 10, 8, Rgba([255, 0, 0, 255]));
        fill_rect(&mut image, 13, 3, 10, 8, Rgba([0, 255, 0, 255]));
        fill_rect(&mut image, 24, 3, 8, 8, Rgba([0, 0, 255, 255]));
        fill_rect(&mut image, 2, 12, 10, 7, Rgba([255, 255, 0, 255]));
        fill_rect(&mut image, 13, 12, 10, 7, Rgba([0, 255, 255, 255]));
        fill_rect(&mut image, 24, 12, 8, 7, Rgba([255, 0, 255, 255]));

        let result = auto_detect_sheet_grid(request_from_png(image)).unwrap();
        let proposal = result.proposals.first().unwrap();

        assert_eq!(proposal.method, "alpha");
        assert_eq!(proposal.computed_rows, 2);
        assert_eq!(proposal.computed_columns, 3);
        assert_eq!(proposal.grid_settings.border_left, 2);
        assert_eq!(proposal.grid_settings.border_top, 3);
        assert_eq!(proposal.grid_settings.gap_x, 1);
        assert_eq!(proposal.grid_settings.gap_y, 1);
    }

    #[test]
    fn detects_solid_background_separator_grid() {
        let mut image = ImageBuffer::from_pixel(25, 12, Rgba([240, 240, 240, 255]));
        fill_rect(&mut image, 1, 1, 10, 10, Rgba([20, 20, 20, 255]));
        fill_rect(&mut image, 13, 1, 10, 10, Rgba([40, 40, 40, 255]));

        let result = auto_detect_sheet_grid(request_from_png(image)).unwrap();
        let proposal = result
            .proposals
            .iter()
            .find(|proposal| proposal.method == "solid_background")
            .unwrap();

        assert_eq!(proposal.computed_rows, 1);
        assert_eq!(proposal.computed_columns, 2);
        assert_eq!(proposal.grid_settings.gap_x, 2);
    }

    #[test]
    fn reports_no_reliable_grid_for_single_flat_image() {
        let image = ImageBuffer::from_pixel(32, 32, Rgba([10, 20, 30, 255]));
        let result = auto_detect_sheet_grid(request_from_png(image)).unwrap();

        assert!(result.proposals.is_empty());
        assert!(result
            .warnings
            .iter()
            .any(|warning| warning.contains("찾지 못했습니다")));
    }

    fn request_from_png(image: ImageBuffer<Rgba<u8>, Vec<u8>>) -> AutoDetectSheetGridRequest {
        let mut cursor = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut cursor, image::ImageFormat::Png)
            .unwrap();
        AutoDetectSheetGridRequest {
            sheet_path: None,
            sheet_file: Some(ImportImageFilePayload {
                original_filename: "sheet.png".to_string(),
                bytes: cursor.into_inner(),
            }),
            alpha_separator_threshold: Some(0.98),
            background_separator_threshold: Some(0.98),
            background_tolerance: Some(4),
            min_cell_width: Some(4),
            min_cell_height: Some(4),
        }
    }

    fn fill_rect(
        image: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        color: Rgba<u8>,
    ) {
        for yy in y..(y + height) {
            for xx in x..(x + width) {
                image.put_pixel(xx, yy, color);
            }
        }
    }
}
