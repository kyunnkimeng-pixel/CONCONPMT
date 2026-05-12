#![allow(dead_code)]

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

use super::grid::{split_pages, PageSplitSettings};
use super::manifest::{
    GifFrameManifestItem, GifFrameSheetManifest, GifFrameSheetPage, APP_NAME,
    GIF_FRAME_SHEET_SCHEMA,
};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GifFrameSheetExportRequest {
    pub icon_id: String,
    pub frame_cell_width: i64,
    pub frame_cell_height: i64,
    pub columns: i64,
    pub frames_per_page: Option<i64>,
    pub gap_x: i64,
    pub gap_y: i64,
    pub border_x: i64,
    pub border_y: i64,
    pub max_sheet_width: i64,
    pub max_sheet_height: i64,
    pub background: String,
    pub include_guide_sheet: bool,
    pub include_manifest: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GifFrameSheetExportResult {
    pub frame_sheet_paths: Vec<String>,
    pub guide_sheet_paths: Vec<String>,
    pub manifest_path: Option<String>,
    pub frame_count: i64,
    pub page_count: i64,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GifFrameSheetReimportRequest {
    pub manifest_path: String,
    pub edited_frame_sheet_paths: Vec<String>,
    pub target_icon_id: String,
    pub create_variant: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GifFrameSheetReimportResult {
    pub variant_id: Option<String>,
    pub output_path: Option<String>,
    pub frame_count: i64,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct GifFrameTiming {
    pub frame_index: i64,
    pub duration_ms: i64,
    pub disposal_method: Option<String>,
    pub source_frame_hash: Option<String>,
}

pub fn gif_frame_sheet_not_implemented<T>() -> AppResult<T> {
    Err(AppError::new(
        "future_feature",
        "GIF 프레임 시트 내보내기/다시 가져오기는 설계와 매니페스트만 준비되어 있으며 다음 단계에서 구현됩니다.",
    ))
}

pub fn build_gif_frame_manifest_plan(
    icon_id: &str,
    source_hash: Option<String>,
    loop_mode: &str,
    frame_cell_width: i64,
    frame_cell_height: i64,
    columns: i64,
    gap_x: i64,
    gap_y: i64,
    border_x: i64,
    border_y: i64,
    max_sheet_width: i64,
    max_sheet_height: i64,
    frames: &[GifFrameTiming],
) -> AppResult<GifFrameSheetManifest> {
    let split = split_pages(
        frames.len(),
        PageSplitSettings {
            cell_width: frame_cell_width,
            cell_height: frame_cell_height,
            columns,
            gap_x,
            gap_y,
            border_x,
            border_y,
            max_sheet_width,
            max_sheet_height,
        },
    )?;
    let pages = split
        .pages
        .iter()
        .map(|page| GifFrameSheetPage {
            page_index: page.page_index,
            sheet_file: format!("frames_sheet_{:03}.png", page.page_index + 1),
            guide_sheet_file: Some(format!("frames_guide_{:03}.png", page.page_index + 1)),
            width: page.width,
            height: page.height,
        })
        .collect::<Vec<_>>();
    let frame_items = split
        .placements
        .iter()
        .map(|placement| {
            let timing = &frames[placement.item_index];
            GifFrameManifestItem {
                frame_index: timing.frame_index,
                sheet_file: format!("frames_sheet_{:03}.png", placement.page_index + 1),
                page_index: placement.page_index,
                row: placement.row,
                col: placement.col,
                x: placement.x,
                y: placement.y,
                w: placement.w,
                h: placement.h,
                duration_ms: timing.duration_ms,
                disposal_method: timing.disposal_method.clone(),
                source_frame_hash: timing.source_frame_hash.clone(),
            }
        })
        .collect::<Vec<_>>();

    Ok(GifFrameSheetManifest {
        schema: GIF_FRAME_SHEET_SCHEMA.to_string(),
        app: APP_NAME.to_string(),
        created_at: "generated-by-runtime".to_string(),
        icon_id: icon_id.to_string(),
        source_hash,
        loop_mode: loop_mode.to_string(),
        frame_cell_width,
        frame_cell_height,
        columns: split.columns_per_page,
        gap_x,
        gap_y,
        border_x,
        border_y,
        pages,
        frames: frame_items,
    })
}

#[cfg(test)]
mod tests {
    use super::{build_gif_frame_manifest_plan, GifFrameTiming};

    #[test]
    fn gif_frame_manifest_preserves_duration_loop_and_page_mapping() {
        let frames = (0..10)
            .map(|index| GifFrameTiming {
                frame_index: index,
                duration_ms: 80 + index,
                disposal_method: Some("background".to_string()),
                source_frame_hash: Some(format!("hash-{index}")),
            })
            .collect::<Vec<_>>();
        let manifest = build_gif_frame_manifest_plan(
            "icon_1",
            Some("source".to_string()),
            "infinite",
            200,
            200,
            8,
            8,
            8,
            16,
            16,
            2048,
            240,
            &frames,
        )
        .unwrap();

        assert_eq!(manifest.loop_mode, "infinite");
        assert_eq!(manifest.frames[3].duration_ms, 83);
        assert_eq!(manifest.pages.len(), 2);
        assert_eq!(manifest.frames[8].page_index, 1);
    }
}
