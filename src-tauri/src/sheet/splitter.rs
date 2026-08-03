use std::collections::HashSet;

use image::{imageops, ImageFormat};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::imaging::import_limits::{decode_import_image, MAX_IMPORT_FILE_BYTES};

use super::composer::{
    sha256_hex, AiGridRect, MAX_AI_GRID_CANVAS_PIXELS, MAX_AI_GRID_CANVAS_SIDE,
    MAX_AI_GRID_INPUT_BYTES, MAX_AI_GRID_ITEMS,
};
use super::importer::png_bytes_from_rgba;

const MIN_REVIEWED_GRID_ITEMS: usize = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StaticGridImageFormat {
    Png,
    Jpeg,
}

impl StaticGridImageFormat {
    fn image_format(self) -> ImageFormat {
        match self {
            Self::Png => ImageFormat::Png,
            Self::Jpeg => ImageFormat::Jpeg,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReviewedGridDecision {
    pub result_cell_index: i64,
    pub target_item_index: i64,
    pub include: bool,
    pub crop: Option<AiGridRect>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SplitReviewedGridRequest<'a> {
    pub encoded_sheet: &'a [u8],
    pub format: StaticGridImageFormat,
    pub request_item_indexes: &'a [i64],
    pub decisions: &'a [ReviewedGridDecision],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SplitGridCell {
    pub result_cell_index: i64,
    pub target_item_index: i64,
    pub png_bytes: Vec<u8>,
    pub png_sha256: String,
    pub width: i64,
    pub height: i64,
    pub has_alpha: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReviewedGridSplit {
    pub output_sheet_sha256: String,
    pub review_signature: String,
    pub cells: Vec<SplitGridCell>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReviewSignaturePayload<'a> {
    schema: &'static str,
    output_sheet_sha256: &'a str,
    sheet_width: i64,
    sheet_height: i64,
    decisions: &'a [ReviewedGridDecision],
    cells: Vec<ReviewSignatureCell<'a>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReviewSignatureCell<'a> {
    result_cell_index: i64,
    target_item_index: i64,
    png_sha256: &'a str,
    width: i64,
    height: i64,
    has_alpha: bool,
}

pub(crate) fn split_reviewed_grid(
    request: SplitReviewedGridRequest<'_>,
) -> AppResult<ReviewedGridSplit> {
    validate_request_shape(&request)?;
    if request.encoded_sheet.is_empty() || request.encoded_sheet.len() > MAX_AI_GRID_INPUT_BYTES {
        return Err(AppError::new(
            "ai_grid_result_too_large",
            "AI grid 결과 이미지는 비어 있지 않은 16MB 이하 JPG 또는 PNG여야 합니다.",
        ));
    }
    let image =
        decode_import_image(request.encoded_sheet, request.format.image_format())?.to_rgba8();
    let sheet_width = i64::from(image.width());
    let sheet_height = i64::from(image.height());
    if image.width() > MAX_AI_GRID_CANVAS_SIDE || image.height() > MAX_AI_GRID_CANVAS_SIDE {
        return Err(AppError::new(
            "ai_grid_result_dimensions",
            "첫 AI grid 단계에서는 결과 이미지 한 변이 2048px 이하여야 합니다.",
        ));
    }
    let sheet_pixels = u64::from(image.width())
        .checked_mul(u64::from(image.height()))
        .ok_or_else(|| {
            AppError::new(
                "ai_grid_result_dimensions",
                "AI grid 결과 픽셀 수가 너무 큽니다.",
            )
        })?;
    if sheet_pixels > MAX_AI_GRID_CANVAS_PIXELS {
        return Err(AppError::new(
            "ai_grid_result_dimensions",
            "AI grid 결과 이미지의 전체 픽셀 수가 지원 범위를 벗어났습니다.",
        ));
    }

    let output_sheet_sha256 = sha256_hex(request.encoded_sheet);
    let mut decisions = request.decisions.to_vec();
    decisions.sort_by_key(|decision| decision.target_item_index);
    let mut expected_cell_size: Option<(i64, i64)> = None;
    let mut total_cell_bytes = 0_usize;
    let mut cells = Vec::new();
    for decision in &decisions {
        if !decision.include {
            continue;
        }
        let crop = decision.crop.ok_or_else(|| {
            AppError::new(
                "ai_grid_review_mapping",
                "포함한 AI grid cell에는 검토한 crop 영역이 필요합니다.",
            )
        })?;
        validate_reviewed_crop(crop, sheet_width, sheet_height)?;
        if crop.width != crop.height {
            return Err(AppError::new(
                "ai_grid_cell_geometry",
                "첫 AI grid 단계의 결과 cell은 정사각형이어야 합니다.",
            ));
        }
        match expected_cell_size {
            Some(expected) if expected != (crop.width, crop.height) => {
                return Err(AppError::new(
                    "ai_grid_cell_geometry",
                    "검토한 AI grid 결과 cell의 크기가 서로 다릅니다.",
                ));
            }
            None => expected_cell_size = Some((crop.width, crop.height)),
            _ => {}
        }
        let x = u32::try_from(crop.x).map_err(|_| cell_bounds_error())?;
        let y = u32::try_from(crop.y).map_err(|_| cell_bounds_error())?;
        let width = u32::try_from(crop.width).map_err(|_| cell_bounds_error())?;
        let height = u32::try_from(crop.height).map_err(|_| cell_bounds_error())?;
        let cell = imageops::crop_imm(&image, x, y, width, height).to_image();
        if !cell.pixels().any(|pixel| pixel.0[3] > 0) {
            return Err(AppError::new(
                "ai_grid_cell_empty",
                "선택한 AI grid 결과 cell이 완전히 투명합니다. 배치를 다시 확인해 주세요.",
            ));
        }
        let has_alpha = cell.pixels().any(|pixel| pixel.0[3] < 255);
        let png_bytes = png_bytes_from_rgba(&cell)?;
        if png_bytes.len() > MAX_AI_GRID_INPUT_BYTES {
            return Err(AppError::new(
                "ai_grid_cell_too_large",
                "AI grid 결과 cell 한 장은 최대 16MB까지 저장할 수 있습니다.",
            ));
        }
        total_cell_bytes = total_cell_bytes
            .checked_add(png_bytes.len())
            .ok_or_else(|| {
                AppError::new(
                    "ai_grid_cell_workload",
                    "AI grid 결과 cell의 전체 파일 크기가 너무 큽니다.",
                )
            })?;
        if total_cell_bytes > MAX_IMPORT_FILE_BYTES {
            return Err(AppError::new(
                "ai_grid_cell_workload",
                "AI grid 결과 cell PNG는 합계 64MB까지 처리할 수 있습니다.",
            ));
        }
        let png_sha256 = sha256_hex(&png_bytes);
        cells.push(SplitGridCell {
            result_cell_index: decision.result_cell_index,
            target_item_index: decision.target_item_index,
            png_bytes,
            png_sha256,
            width: crop.width,
            height: crop.height,
            has_alpha,
        });
    }
    if cells.is_empty() {
        return Err(AppError::new(
            "ai_grid_review_empty",
            "저장할 AI grid 결과 cell을 하나 이상 포함해 주세요.",
        ));
    }
    cells.sort_by_key(|cell| cell.target_item_index);
    let signature_cells = cells
        .iter()
        .map(|cell| ReviewSignatureCell {
            result_cell_index: cell.result_cell_index,
            target_item_index: cell.target_item_index,
            png_sha256: &cell.png_sha256,
            width: cell.width,
            height: cell.height,
            has_alpha: cell.has_alpha,
        })
        .collect::<Vec<_>>();
    let signature_payload = serde_json::to_vec(&ReviewSignaturePayload {
        schema: "pmtcon-ai-grid-review-v1",
        output_sheet_sha256: &output_sheet_sha256,
        sheet_width,
        sheet_height,
        decisions: &decisions,
        cells: signature_cells,
    })
    .map_err(|error| AppError::new("ai_grid_review_signature", error.to_string()))?;
    let review_signature = sha256_hex(&signature_payload);

    Ok(ReviewedGridSplit {
        output_sheet_sha256,
        review_signature,
        cells,
    })
}

fn validate_request_shape(request: &SplitReviewedGridRequest<'_>) -> AppResult<()> {
    if !(MIN_REVIEWED_GRID_ITEMS..=MAX_AI_GRID_ITEMS).contains(&request.request_item_indexes.len())
    {
        return Err(AppError::new(
            "ai_grid_item_count",
            "AI 결과 검토 항목은 1~16개여야 합니다.",
        ));
    }
    if request.decisions.len() != request.request_item_indexes.len() {
        return Err(AppError::new(
            "ai_grid_review_mapping",
            "모든 AI grid 요청 항목에 포함 또는 제외 결정을 내려야 합니다.",
        ));
    }
    let request_items = request
        .request_item_indexes
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    if request_items.len() != request.request_item_indexes.len()
        || request_items.iter().any(|index| *index < 0)
    {
        return Err(AppError::new(
            "ai_grid_review_mapping",
            "AI grid 요청 항목 번호가 중복되었거나 올바르지 않습니다.",
        ));
    }
    let mut targets = HashSet::with_capacity(request.decisions.len());
    let mut included_results = HashSet::with_capacity(request.decisions.len());
    for decision in request.decisions {
        if decision.target_item_index < 0
            || !request_items.contains(&decision.target_item_index)
            || !targets.insert(decision.target_item_index)
        {
            return Err(AppError::new(
                "ai_grid_review_mapping",
                "AI grid target mapping이 중복되었거나 올바르지 않습니다.",
            ));
        }
        if decision.include {
            if decision.result_cell_index < 0
                || !included_results.insert(decision.result_cell_index)
            {
                return Err(AppError::new(
                    "ai_grid_review_mapping",
                    "포함한 AI grid 결과 cell 번호가 중복되었거나 올바르지 않습니다.",
                ));
            }
            if decision.crop.is_none() {
                return Err(AppError::new(
                    "ai_grid_review_mapping",
                    "포함한 AI grid 결과 cell에는 crop 영역이 필요합니다.",
                ));
            }
        } else if decision.crop.is_some() {
            return Err(AppError::new(
                "ai_grid_review_mapping",
                "제외한 AI grid 결과 cell에는 crop 영역을 저장하지 않습니다.",
            ));
        }
    }
    if targets != request_items {
        return Err(AppError::new(
            "ai_grid_review_mapping",
            "AI grid 요청 항목과 검토 mapping이 일치하지 않습니다.",
        ));
    }
    Ok(())
}

fn validate_reviewed_crop(crop: AiGridRect, sheet_width: i64, sheet_height: i64) -> AppResult<()> {
    if crop.x < 0 || crop.y < 0 || crop.width <= 0 || crop.height <= 0 {
        return Err(cell_bounds_error());
    }
    let right = crop
        .x
        .checked_add(crop.width)
        .ok_or_else(cell_bounds_error)?;
    let bottom = crop
        .y
        .checked_add(crop.height)
        .ok_or_else(cell_bounds_error)?;
    if right > sheet_width || bottom > sheet_height {
        return Err(cell_bounds_error());
    }
    Ok(())
}

fn cell_bounds_error() -> AppError {
    AppError::new(
        "ai_grid_cell_out_of_bounds",
        "검토한 AI grid 결과 cell이 이미지 범위를 벗어납니다.",
    )
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use image::{DynamicImage, ImageBuffer, Rgba};

    use super::*;

    fn encoded_fixture() -> Vec<u8> {
        let image = ImageBuffer::from_fn(20, 10, |x, y| {
            if x < 10 {
                Rgba([240, x as u8, y as u8, 128])
            } else {
                Rgba([x as u8, 80, 220, 255])
            }
        });
        let mut cursor = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image)
            .write_to(&mut cursor, ImageFormat::Png)
            .unwrap();
        cursor.into_inner()
    }

    fn included_decisions() -> Vec<ReviewedGridDecision> {
        vec![
            ReviewedGridDecision {
                result_cell_index: 1,
                target_item_index: 1,
                include: true,
                crop: Some(AiGridRect {
                    x: 10,
                    y: 0,
                    width: 10,
                    height: 10,
                }),
            },
            ReviewedGridDecision {
                result_cell_index: 0,
                target_item_index: 0,
                include: true,
                crop: Some(AiGridRect {
                    x: 0,
                    y: 0,
                    width: 10,
                    height: 10,
                }),
            },
        ]
    }

    fn split<'a>(
        encoded_sheet: &'a [u8],
        request_item_indexes: &'a [i64],
        decisions: &'a [ReviewedGridDecision],
    ) -> AppResult<ReviewedGridSplit> {
        split_reviewed_grid(SplitReviewedGridRequest {
            encoded_sheet,
            format: StaticGridImageFormat::Png,
            request_item_indexes,
            decisions,
        })
    }

    #[test]
    fn split_is_deterministic_sorted_and_preserves_alpha() {
        let encoded = encoded_fixture();
        let indexes = [0, 1];
        let decisions = included_decisions();
        let first = split(&encoded, &indexes, &decisions).unwrap();
        let second = split(&encoded, &indexes, &decisions).unwrap();

        assert_eq!(first, second);
        assert_eq!(first.cells.len(), 2);
        assert_eq!(first.cells[0].target_item_index, 0);
        assert_eq!(first.cells[1].target_item_index, 1);
        assert!(first.cells[0].has_alpha);
        assert!(!first.cells[1].has_alpha);
        let decoded =
            image::load_from_memory_with_format(&first.cells[0].png_bytes, ImageFormat::Png)
                .unwrap()
                .to_rgba8();
        assert_eq!(decoded.get_pixel(0, 0).0[3], 128);
        assert_eq!(first.output_sheet_sha256, sha256_hex(&encoded));
    }

    #[test]
    fn excluded_target_is_not_emitted_but_remains_mapped() {
        let encoded = encoded_fixture();
        let indexes = [0, 1];
        let decisions = vec![
            ReviewedGridDecision {
                result_cell_index: 0,
                target_item_index: 0,
                include: false,
                crop: None,
            },
            ReviewedGridDecision {
                result_cell_index: 1,
                target_item_index: 1,
                include: true,
                crop: Some(AiGridRect {
                    x: 10,
                    y: 0,
                    width: 10,
                    height: 10,
                }),
            },
        ];
        let result = split(&encoded, &indexes, &decisions).unwrap();
        assert_eq!(result.cells.len(), 1);
        assert_eq!(result.cells[0].target_item_index, 1);
    }

    #[test]
    fn invalid_mapping_and_bounds_are_all_or_none_errors() {
        let encoded = encoded_fixture();
        let indexes = [0, 1];

        let duplicate_target = vec![
            ReviewedGridDecision {
                result_cell_index: 0,
                target_item_index: 0,
                include: true,
                crop: Some(AiGridRect {
                    x: 0,
                    y: 0,
                    width: 10,
                    height: 10,
                }),
            },
            ReviewedGridDecision {
                result_cell_index: 1,
                target_item_index: 0,
                include: false,
                crop: None,
            },
        ];
        assert_eq!(
            split(&encoded, &indexes, &duplicate_target)
                .unwrap_err()
                .code,
            "ai_grid_review_mapping"
        );

        let mut duplicate_result = included_decisions();
        duplicate_result[1].result_cell_index = 1;
        assert_eq!(
            split(&encoded, &indexes, &duplicate_result)
                .unwrap_err()
                .code,
            "ai_grid_review_mapping"
        );

        let missing_decisions = included_decisions();
        assert_eq!(
            split(&encoded, &indexes, &missing_decisions[..1])
                .unwrap_err()
                .code,
            "ai_grid_review_mapping"
        );

        let mut out_of_bounds = included_decisions();
        out_of_bounds[0].crop = Some(AiGridRect {
            x: 11,
            y: 0,
            width: 10,
            height: 10,
        });
        assert_eq!(
            split(&encoded, &indexes, &out_of_bounds).unwrap_err().code,
            "ai_grid_cell_out_of_bounds"
        );
    }

    #[test]
    fn transparent_and_inconsistent_cells_are_rejected() {
        let mut transparent = ImageBuffer::from_pixel(20, 10, Rgba([0, 0, 0, 0]));
        for y in 0..10 {
            for x in 10..20 {
                transparent.put_pixel(x, y, Rgba([20, 30, 40, 255]));
            }
        }
        let mut cursor = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(transparent)
            .write_to(&mut cursor, ImageFormat::Png)
            .unwrap();
        let encoded = cursor.into_inner();
        let indexes = [0, 1];
        let decisions = included_decisions();
        assert_eq!(
            split(&encoded, &indexes, &decisions).unwrap_err().code,
            "ai_grid_cell_empty"
        );

        let encoded = encoded_fixture();
        let mut non_square = included_decisions();
        non_square[0].crop = Some(AiGridRect {
            x: 10,
            y: 0,
            width: 9,
            height: 10,
        });
        assert_eq!(
            split(&encoded, &indexes, &non_square).unwrap_err().code,
            "ai_grid_cell_geometry"
        );

        let mut mismatched = included_decisions();
        mismatched[0].crop = Some(AiGridRect {
            x: 10,
            y: 0,
            width: 9,
            height: 9,
        });
        assert_eq!(
            split(&encoded, &indexes, &mismatched).unwrap_err().code,
            "ai_grid_cell_geometry"
        );
    }

    #[test]
    fn jpeg_sheet_is_supported_without_claiming_alpha() {
        let image = ImageBuffer::from_fn(20, 10, |x, y| image::Rgb([x as u8, y as u8, 140_u8]));
        let mut cursor = Cursor::new(Vec::new());
        DynamicImage::ImageRgb8(image)
            .write_to(&mut cursor, ImageFormat::Jpeg)
            .unwrap();
        let encoded = cursor.into_inner();
        let indexes = [0, 1];
        let decisions = included_decisions();
        let result = split_reviewed_grid(SplitReviewedGridRequest {
            encoded_sheet: &encoded,
            format: StaticGridImageFormat::Jpeg,
            request_item_indexes: &indexes,
            decisions: &decisions,
        })
        .unwrap();
        assert_eq!(result.cells.len(), 2);
        assert!(result.cells.iter().all(|cell| !cell.has_alpha));
    }

    #[test]
    fn single_generation_result_is_supported_but_empty_request_is_rejected() {
        let encoded = encoded_fixture();
        let indexes = [0];
        let decisions = [ReviewedGridDecision {
            result_cell_index: 0,
            target_item_index: 0,
            include: true,
            crop: Some(AiGridRect {
                x: 0,
                y: 0,
                width: 10,
                height: 10,
            }),
        }];
        let result = split(&encoded, &indexes, &decisions).unwrap();
        assert_eq!(result.cells.len(), 1);
        assert_eq!(result.cells[0].target_item_index, 0);
        assert!(result.cells[0].has_alpha);

        assert_eq!(
            split(&encoded, &[], &[]).unwrap_err().code,
            "ai_grid_item_count"
        );
    }
}
