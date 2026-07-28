#![allow(dead_code)]

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::{Component, Path};

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::imaging::import_limits::{
    validate_gif_workload, validate_import_dimensions, MAX_GIF_TOTAL_FRAME_PIXELS,
    MAX_IMPORT_DIMENSION, MAX_IMPORT_FILE_BYTES,
};

pub const STATIC_SHEET_SCHEMA: &str = "pmtcon-sheet-v2";
pub const LEGACY_STATIC_SHEET_SCHEMA: &str = "pmtcon-sheet-v1";
pub const GIF_FRAME_SHEET_SCHEMA: &str = "pmtcon-gif-frame-sheet-v2";
pub const LEGACY_GIF_FRAME_SHEET_SCHEMA: &str = "pmtcon-gif-frame-sheet-v1";
pub const APP_NAME: &str = "PMTCONCON Studio";

const MAX_MANIFEST_ENTRIES: usize = 10_000;
const MAX_MANIFEST_FILE_NAME_BYTES: usize = 255;
const MAX_MANIFEST_ID_BYTES: usize = 128;
const MAX_MANIFEST_FRAME_DURATION_MS: i64 = 60_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaticSheetManifest {
    pub schema: String,
    pub app: String,
    pub created_at: String,
    pub collection_id: String,
    pub sheet_type: String,
    pub profile: StaticSheetProfile,
    pub pages: Vec<StaticSheetPage>,
    pub items: Vec<StaticSheetManifestItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaticSheetProfile {
    pub cell_width: i64,
    pub cell_height: i64,
    pub columns: i64,
    pub gap_x: i64,
    pub gap_y: i64,
    pub border_x: i64,
    pub border_y: i64,
    pub background: String,
    pub read_order: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaticSheetPage {
    pub page_index: i64,
    pub clean_sheet_file: String,
    pub guide_sheet_file: Option<String>,
    pub width: i64,
    pub height: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaticSheetManifestItem {
    pub icon_id: String,
    pub piece_id: Option<String>,
    pub page_index: i64,
    pub row: i64,
    pub col: i64,
    pub index: i64,
    pub export_number: i64,
    pub x: i64,
    pub y: i64,
    pub w: i64,
    pub h: i64,
    pub display_name: String,
    pub alt: String,
    pub icon_type: String,
    pub format: String,
    pub source_hash: Option<String>,
    pub render_hash: Option<String>,
    #[serde(default)]
    pub visual_source: Option<ManifestVisualSource>,
    pub render_recipe_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GifFrameSheetManifest {
    pub schema: String,
    pub app: String,
    pub created_at: String,
    pub icon_id: String,
    pub source_file_id: Option<String>,
    pub source_hash: Option<String>,
    #[serde(default)]
    pub render_recipe_hash: Option<String>,
    pub display_name: String,
    pub loop_mode: String,
    #[serde(default)]
    pub visual_source: Option<ManifestVisualSource>,
    pub loop_count: Option<i64>,
    pub frame_count: i64,
    pub duration_ms: i64,
    pub frame_cell_width: i64,
    pub frame_cell_height: i64,
    pub columns: i64,
    pub gap_x: i64,
    pub gap_y: i64,
    pub border_x: i64,
    pub border_y: i64,
    pub background: String,
    pub pages: Vec<GifFrameSheetPage>,
    pub frames: Vec<GifFrameManifestItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GifFrameSheetPage {
    pub page_index: i64,
    #[serde(alias = "sheet_file")]
    pub clean_sheet_file: String,
    pub guide_sheet_file: Option<String>,
    pub width: i64,
    pub height: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManifestVisualSource {
    pub original_source_file_id: String,
    pub original_source_hash: String,
    pub original_lineage_id: String,
    pub original_lineage_generation: i64,
    pub effective_source_file_id: String,
    pub effective_source_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GifFrameManifestItem {
    pub frame_index: i64,
    pub sheet_file: String,
    pub page_index: i64,
    pub row: i64,
    pub col: i64,
    pub x: i64,
    pub y: i64,
    pub w: i64,
    pub h: i64,
    pub duration_ms: i64,
    pub disposal_method: Option<String>,
    pub source_frame_hash: Option<String>,
}

pub fn write_static_manifest(path: &Path, manifest: &StaticSheetManifest) -> AppResult<()> {
    validate_static_manifest(manifest)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(manifest)
        .map_err(|error| AppError::new("manifest", error.to_string()))?;
    fs::write(path, json)?;
    Ok(())
}

pub fn read_static_manifest(path: &Path) -> AppResult<StaticSheetManifest> {
    let bytes = read_limited_manifest_file(path)?;
    read_static_manifest_bytes(&bytes)
}

pub fn read_static_manifest_bytes(bytes: &[u8]) -> AppResult<StaticSheetManifest> {
    validate_manifest_byte_size(bytes.len())?;
    let manifest: StaticSheetManifest = serde_json::from_slice(bytes)
        .map_err(|error| AppError::new("manifest", error.to_string()))?;
    validate_static_manifest(&manifest)?;
    Ok(manifest)
}

pub fn write_gif_manifest(path: &Path, manifest: &GifFrameSheetManifest) -> AppResult<()> {
    validate_gif_manifest(manifest)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(manifest)
        .map_err(|error| AppError::new("manifest", error.to_string()))?;
    fs::write(path, json)?;
    Ok(())
}

pub fn read_gif_manifest(path: &Path) -> AppResult<GifFrameSheetManifest> {
    let bytes = read_limited_manifest_file(path)?;
    read_gif_manifest_bytes(&bytes)
}

pub fn read_gif_manifest_bytes(bytes: &[u8]) -> AppResult<GifFrameSheetManifest> {
    validate_manifest_byte_size(bytes.len())?;
    let manifest: GifFrameSheetManifest = serde_json::from_slice(bytes)
        .map_err(|error| AppError::new("manifest", error.to_string()))?;
    validate_gif_manifest(&manifest)?;
    Ok(manifest)
}

pub fn validate_static_manifest(manifest: &StaticSheetManifest) -> AppResult<()> {
    let is_v2 = match manifest.schema.as_str() {
        STATIC_SHEET_SCHEMA => true,
        LEGACY_STATIC_SHEET_SCHEMA => false,
        _ => {
            return Err(AppError::new(
                "manifest_schema",
                "지원하는 PMTCONCON Studio 정적 시트 매니페스트가 아닙니다.",
            ))
        }
    };
    if manifest.app != APP_NAME {
        return Err(AppError::new(
            "manifest_app",
            "PMTCONCON Studio 매니페스트가 아닙니다.",
        ));
    }
    validate_stable_id(&manifest.collection_id, "모음 ID")?;
    validate_manifest_dimensions(
        manifest.profile.cell_width,
        manifest.profile.cell_height,
        "정적 시트 셀 크기",
    )?;
    if manifest.profile.columns <= 0
        || manifest.profile.columns > i64::try_from(MAX_MANIFEST_ENTRIES).unwrap_or(i64::MAX)
    {
        return Err(AppError::new(
            "manifest_validation",
            "시트 매니페스트의 열 수가 올바르지 않습니다.",
        ));
    }
    if manifest.pages.is_empty() || manifest.pages.len() > MAX_MANIFEST_ENTRIES {
        return Err(AppError::new(
            "manifest_workload",
            "시트 매니페스트의 페이지 수가 지원 범위를 벗어났습니다.",
        ));
    }
    if manifest.items.len() > MAX_MANIFEST_ENTRIES {
        return Err(AppError::new(
            "manifest_workload",
            format!("시트 매니페스트는 최대 {MAX_MANIFEST_ENTRIES}개 셀까지 처리할 수 있습니다."),
        ));
    }

    let mut pages = HashMap::with_capacity(manifest.pages.len());
    let mut page_file_names = HashSet::with_capacity(manifest.pages.len());
    let mut total_page_pixels = 0_u64;
    for page in &manifest.pages {
        if page.page_index < 0 || pages.contains_key(&page.page_index) {
            return Err(AppError::new(
                "manifest_validation",
                "시트 매니페스트의 페이지 번호가 중복되었거나 올바르지 않습니다.",
            ));
        }
        validate_manifest_dimensions(page.width, page.height, "정적 시트 페이지 크기")?;
        validate_safe_file_name(&page.clean_sheet_file, "정적 시트 파일명")?;
        if !page_file_names.insert(page.clean_sheet_file.as_str()) {
            return Err(AppError::new(
                "manifest_validation",
                "정적 시트 페이지 파일명이 중복되었습니다.",
            ));
        }
        total_page_pixels = total_page_pixels
            .checked_add(validate_crop_bounds(
                0,
                0,
                page.width,
                page.height,
                page.width,
                page.height,
                "정적 시트 페이지",
            )?)
            .ok_or_else(|| {
                AppError::new(
                    "manifest_workload",
                    "정적 시트 페이지의 전체 픽셀 수가 너무 큽니다.",
                )
            })?;
        if total_page_pixels > MAX_GIF_TOTAL_FRAME_PIXELS {
            return Err(AppError::new(
                "manifest_workload",
                "정적 시트 페이지의 전체 픽셀 수가 지원 범위를 벗어났습니다.",
            ));
        }
        if let Some(guide_sheet_file) = page.guide_sheet_file.as_deref() {
            validate_safe_file_name(guide_sheet_file, "정적 가이드 시트 파일명")?;
        }
        pages.insert(page.page_index, (page.width, page.height));
    }

    let mut item_indexes = HashSet::with_capacity(manifest.items.len());
    let mut total_crop_pixels = 0_u64;
    for item in &manifest.items {
        validate_stable_id(&item.icon_id, "아이콘 ID")?;
        if let Some(piece_id) = item.piece_id.as_deref() {
            validate_stable_id(piece_id, "조각 ID")?;
        }
        match item.visual_source.as_ref() {
            Some(visual_source) => {
                validate_visual_source(visual_source)?;
                if is_v2
                    && item.source_hash.as_deref()
                        != Some(visual_source.effective_source_hash.as_str())
                {
                    return Err(AppError::new(
                        "manifest_provenance",
                        "v2 정적 시트의 source_hash와 effective source hash가 다릅니다.",
                    ));
                }
            }
            None if is_v2 => {
                return Err(AppError::new(
                    "manifest_provenance",
                    "v2 정적 시트 항목에 visual_source provenance가 없습니다.",
                ));
            }
            None => {}
        }
        if item.index < 0 || !item_indexes.insert(item.index) {
            return Err(AppError::new(
                "manifest_validation",
                "시트 매니페스트의 셀 index가 중복되었거나 올바르지 않습니다.",
            ));
        }
        let Some(&(page_width, page_height)) = pages.get(&item.page_index) else {
            return Err(AppError::new(
                "manifest_validation",
                "시트 매니페스트의 셀이 존재하지 않는 페이지를 가리킵니다.",
            ));
        };
        let crop_pixels = validate_crop_bounds(
            item.x,
            item.y,
            item.w,
            item.h,
            page_width,
            page_height,
            "정적 시트 셀",
        )?;
        total_crop_pixels = total_crop_pixels.checked_add(crop_pixels).ok_or_else(|| {
            AppError::new("manifest_workload", "시트 셀의 전체 픽셀 수가 너무 큽니다.")
        })?;
        if total_crop_pixels > MAX_GIF_TOTAL_FRAME_PIXELS {
            return Err(AppError::new(
                "manifest_workload",
                "시트 셀의 전체 픽셀 수가 지원 범위를 벗어났습니다.",
            ));
        }
    }
    Ok(())
}

pub fn validate_gif_manifest(manifest: &GifFrameSheetManifest) -> AppResult<()> {
    let is_v2 = match manifest.schema.as_str() {
        GIF_FRAME_SHEET_SCHEMA => true,
        LEGACY_GIF_FRAME_SHEET_SCHEMA => false,
        _ => {
            return Err(AppError::new(
                "manifest_schema",
                "지원하는 PMTCONCON Studio GIF 프레임 시트 매니페스트가 아닙니다.",
            ))
        }
    };
    if manifest.app != APP_NAME {
        return Err(AppError::new(
            "manifest_app",
            "PMTCONCON Studio 매니페스트가 아닙니다.",
        ));
    }
    validate_stable_id(&manifest.icon_id, "아이콘 ID")?;
    if let Some(source_file_id) = manifest.source_file_id.as_deref() {
        validate_stable_id(source_file_id, "원본 파일 ID")?;
    }
    match manifest.visual_source.as_ref() {
        Some(visual_source) => {
            validate_visual_source(visual_source)?;
            if is_v2
                && (manifest.source_file_id.as_deref()
                    != Some(visual_source.effective_source_file_id.as_str())
                    || manifest.source_hash.as_deref()
                        != Some(visual_source.effective_source_hash.as_str()))
            {
                return Err(AppError::new(
                    "manifest_provenance",
                    "v2 GIF 시트의 source ID/hash와 effective source provenance가 다릅니다.",
                ));
            }
        }
        None if is_v2 => {
            return Err(AppError::new(
                "manifest_provenance",
                "v2 GIF 프레임 시트에 visual_source provenance가 없습니다.",
            ));
        }
        None => {}
    }
    validate_manifest_dimensions(
        manifest.frame_cell_width,
        manifest.frame_cell_height,
        "GIF 프레임 시트 셀 크기",
    )?;
    if manifest.columns <= 0
        || manifest.columns > i64::try_from(MAX_MANIFEST_ENTRIES).unwrap_or(i64::MAX)
    {
        return Err(AppError::new(
            "manifest_validation",
            "GIF 프레임 시트 열 수가 올바르지 않습니다.",
        ));
    }
    if manifest.pages.is_empty() || manifest.pages.len() > MAX_MANIFEST_ENTRIES {
        return Err(AppError::new(
            "manifest_workload",
            "GIF 프레임 매니페스트의 페이지 수가 지원 범위를 벗어났습니다.",
        ));
    }
    if manifest.frame_count <= 0
        || manifest.frame_count as usize != manifest.frames.len()
        || manifest.frames.len() > MAX_MANIFEST_ENTRIES
    {
        return Err(AppError::new(
            "manifest_workload",
            "GIF 프레임 매니페스트의 frame_count와 frames 목록이 올바르지 않습니다.",
        ));
    }
    let workload_width = u32::try_from(manifest.frame_cell_width).map_err(|_| {
        AppError::new(
            "manifest_workload",
            "GIF 프레임 시트 셀 너비가 너무 큽니다.",
        )
    })?;
    let workload_height = u32::try_from(manifest.frame_cell_height).map_err(|_| {
        AppError::new(
            "manifest_workload",
            "GIF 프레임 시트 셀 높이가 너무 큽니다.",
        )
    })?;
    validate_gif_workload(workload_width, workload_height, manifest.frame_count)
        .map_err(|message| AppError::new("manifest_workload", message))?;

    let mut pages = HashMap::with_capacity(manifest.pages.len());
    let mut page_file_names = HashSet::with_capacity(manifest.pages.len());
    let mut total_page_pixels = 0_u64;
    for page in &manifest.pages {
        if page.page_index < 0 || pages.contains_key(&page.page_index) {
            return Err(AppError::new(
                "manifest_validation",
                "GIF 프레임 매니페스트의 페이지 번호가 중복되었거나 올바르지 않습니다.",
            ));
        }
        validate_manifest_dimensions(page.width, page.height, "GIF 프레임 시트 페이지 크기")?;
        validate_safe_file_name(&page.clean_sheet_file, "GIF 프레임 시트 파일명")?;
        if !page_file_names.insert(page.clean_sheet_file.as_str()) {
            return Err(AppError::new(
                "manifest_validation",
                "GIF 프레임 시트 페이지 파일명이 중복되었습니다.",
            ));
        }
        total_page_pixels = total_page_pixels
            .checked_add(validate_crop_bounds(
                0,
                0,
                page.width,
                page.height,
                page.width,
                page.height,
                "GIF 프레임 시트 페이지",
            )?)
            .ok_or_else(|| {
                AppError::new(
                    "manifest_workload",
                    "GIF 프레임 시트 페이지의 전체 픽셀 수가 너무 큽니다.",
                )
            })?;
        if total_page_pixels > MAX_GIF_TOTAL_FRAME_PIXELS {
            return Err(AppError::new(
                "manifest_workload",
                "GIF 프레임 시트 페이지의 전체 픽셀 수가 지원 범위를 벗어났습니다.",
            ));
        }
        if let Some(guide_sheet_file) = page.guide_sheet_file.as_deref() {
            validate_safe_file_name(guide_sheet_file, "GIF 가이드 시트 파일명")?;
        }
        pages.insert(
            page.page_index,
            (page.width, page.height, page.clean_sheet_file.as_str()),
        );
    }

    let mut frame_indexes = HashSet::with_capacity(manifest.frames.len());
    let mut total_crop_pixels = 0_u64;
    let mut total_duration_ms = 0_i64;
    for frame in &manifest.frames {
        validate_safe_file_name(&frame.sheet_file, "GIF 프레임 시트 파일명")?;
        if frame.frame_index < 0
            || frame.frame_index >= manifest.frame_count
            || !frame_indexes.insert(frame.frame_index)
        {
            return Err(AppError::new(
                "manifest_validation",
                "GIF 프레임 번호가 중복되었거나 올바르지 않습니다.",
            ));
        }
        if !(1..=MAX_MANIFEST_FRAME_DURATION_MS).contains(&frame.duration_ms) {
            return Err(AppError::new(
                "manifest_validation",
                format!(
                    "GIF 프레임 재생시간은 1ms 이상 {MAX_MANIFEST_FRAME_DURATION_MS}ms 이하여야 합니다."
                ),
            ));
        }
        let Some(&(page_width, page_height, clean_sheet_file)) = pages.get(&frame.page_index)
        else {
            return Err(AppError::new(
                "manifest_validation",
                "GIF 프레임이 존재하지 않는 페이지를 가리킵니다.",
            ));
        };
        if frame.sheet_file != clean_sheet_file {
            return Err(AppError::new(
                "manifest_validation",
                "GIF 프레임의 시트 파일명과 페이지 파일명이 일치하지 않습니다.",
            ));
        }
        let crop_pixels = validate_crop_bounds(
            frame.x,
            frame.y,
            frame.w,
            frame.h,
            page_width,
            page_height,
            "GIF 프레임 셀",
        )?;
        total_crop_pixels = total_crop_pixels.checked_add(crop_pixels).ok_or_else(|| {
            AppError::new(
                "manifest_workload",
                "GIF 프레임 셀의 전체 픽셀 수가 너무 큽니다.",
            )
        })?;
        if total_crop_pixels > MAX_GIF_TOTAL_FRAME_PIXELS {
            return Err(AppError::new(
                "manifest_workload",
                "GIF 프레임 셀의 전체 픽셀 수가 지원 범위를 벗어났습니다.",
            ));
        }
        total_duration_ms = total_duration_ms
            .checked_add(frame.duration_ms)
            .ok_or_else(|| {
                AppError::new(
                    "manifest_workload",
                    "GIF 프레임의 전체 재생시간이 너무 큽니다.",
                )
            })?;
    }
    if total_duration_ms != manifest.duration_ms {
        return Err(AppError::new(
            "manifest_validation",
            "GIF 프레임 재생시간 합계와 매니페스트 duration_ms가 일치하지 않습니다.",
        ));
    }
    Ok(())
}

fn read_limited_manifest_file(path: &Path) -> AppResult<Vec<u8>> {
    let file = fs::File::open(path)?;
    let metadata_size = usize::try_from(file.metadata()?.len()).map_err(|_| {
        AppError::new(
            "manifest_size",
            "매니페스트 파일 크기를 확인할 수 없습니다.",
        )
    })?;
    validate_manifest_byte_size(metadata_size)?;

    let mut bytes = Vec::with_capacity(metadata_size.min(MAX_IMPORT_FILE_BYTES));
    let mut limited = file.take((MAX_IMPORT_FILE_BYTES as u64).saturating_add(1));
    limited.read_to_end(&mut bytes)?;
    validate_manifest_byte_size(bytes.len())?;
    Ok(bytes)
}

fn validate_manifest_byte_size(byte_size: usize) -> AppResult<()> {
    if byte_size > MAX_IMPORT_FILE_BYTES {
        return Err(AppError::new(
            "manifest_size",
            "시트 매니페스트 파일은 최대 64MB까지 읽을 수 있습니다.",
        ));
    }
    Ok(())
}

fn validate_manifest_dimensions(width: i64, height: i64, label: &str) -> AppResult<()> {
    let width = u32::try_from(width).map_err(|_| {
        AppError::new(
            "manifest_validation",
            format!("{label}가 올바르지 않습니다."),
        )
    })?;
    let height = u32::try_from(height).map_err(|_| {
        AppError::new(
            "manifest_validation",
            format!("{label}가 올바르지 않습니다."),
        )
    })?;
    if width > MAX_IMPORT_DIMENSION || height > MAX_IMPORT_DIMENSION {
        return Err(AppError::new(
            "manifest_workload",
            format!("{label}는 한 변 최대 {MAX_IMPORT_DIMENSION}px까지 지원합니다."),
        ));
    }
    validate_import_dimensions(width, height).map_err(|_| {
        AppError::new(
            "manifest_workload",
            format!("{label}의 전체 픽셀 수가 지원 범위를 벗어났습니다."),
        )
    })
}

fn validate_crop_bounds(
    x: i64,
    y: i64,
    width: i64,
    height: i64,
    page_width: i64,
    page_height: i64,
    label: &str,
) -> AppResult<u64> {
    if x < 0 || y < 0 || width <= 0 || height <= 0 {
        return Err(AppError::new(
            "manifest_validation",
            format!("{label} 영역이 올바르지 않습니다."),
        ));
    }
    let right = x.checked_add(width).ok_or_else(|| {
        AppError::new(
            "manifest_validation",
            format!("{label} 좌표가 지원 범위를 벗어났습니다."),
        )
    })?;
    let bottom = y.checked_add(height).ok_or_else(|| {
        AppError::new(
            "manifest_validation",
            format!("{label} 좌표가 지원 범위를 벗어났습니다."),
        )
    })?;
    if right > page_width || bottom > page_height {
        return Err(AppError::new(
            "manifest_validation",
            format!("{label} 영역이 페이지 이미지 밖으로 나갑니다."),
        ));
    }
    let width = u64::try_from(width).map_err(|_| {
        AppError::new(
            "manifest_validation",
            format!("{label} 너비가 올바르지 않습니다."),
        )
    })?;
    let height = u64::try_from(height).map_err(|_| {
        AppError::new(
            "manifest_validation",
            format!("{label} 높이가 올바르지 않습니다."),
        )
    })?;
    width.checked_mul(height).ok_or_else(|| {
        AppError::new(
            "manifest_workload",
            format!("{label}의 픽셀 수가 너무 큽니다."),
        )
    })
}
fn validate_visual_source(visual_source: &ManifestVisualSource) -> AppResult<()> {
    validate_stable_id(
        &visual_source.original_source_file_id,
        "원본 source file ID",
    )?;
    validate_stable_id(&visual_source.original_lineage_id, "원본 lineage ID")?;
    validate_stable_id(
        &visual_source.effective_source_file_id,
        "effective source file ID",
    )?;
    if visual_source.original_lineage_generation < 0 {
        return Err(AppError::new(
            "manifest_provenance",
            "원본 lineage generation은 0 이상이어야 합니다.",
        ));
    }
    for (hash, label) in [
        (&visual_source.original_source_hash, "원본 source hash"),
        (
            &visual_source.effective_source_hash,
            "effective source hash",
        ),
    ] {
        if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(AppError::new(
                "manifest_provenance",
                format!("{label}가 올바른 SHA-256 형식이 아닙니다."),
            ));
        }
    }
    Ok(())
}

fn validate_stable_id(value: &str, label: &str) -> AppResult<()> {
    if value.is_empty()
        || value.len() > MAX_MANIFEST_ID_BYTES
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err(AppError::new(
            "manifest_path",
            format!("{label}가 안전한 형식이 아닙니다."),
        ));
    }
    Ok(())
}

fn validate_safe_file_name(value: &str, label: &str) -> AppResult<()> {
    let mut components = Path::new(value).components();
    let is_single_normal_component =
        matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none();
    let has_safe_characters = value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'));
    let is_png = Path::new(value)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("png"));
    let device_stem = value
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    let is_reserved_device = matches!(
        device_stem.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    );
    if value.is_empty()
        || value.len() > MAX_MANIFEST_FILE_NAME_BYTES
        || !is_single_normal_component
        || !has_safe_characters
        || !is_png
        || is_reserved_device
    {
        return Err(AppError::new(
            "manifest_path",
            format!("{label}은 안전한 단일 PNG 파일명이어야 합니다."),
        ));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::{
        validate_gif_manifest, validate_static_manifest, GifFrameManifestItem,
        GifFrameSheetManifest, GifFrameSheetPage, ManifestVisualSource, StaticSheetManifest,
        StaticSheetManifestItem, StaticSheetPage, StaticSheetProfile, APP_NAME,
        GIF_FRAME_SHEET_SCHEMA, LEGACY_STATIC_SHEET_SCHEMA, STATIC_SHEET_SCHEMA,
    };

    fn valid_visual_source() -> ManifestVisualSource {
        ManifestVisualSource {
            original_source_file_id: "source_original".to_string(),
            original_source_hash: "a".repeat(64),
            original_lineage_id: "lineage_1".to_string(),
            original_lineage_generation: 0,
            effective_source_file_id: "source_1".to_string(),
            effective_source_hash: "b".repeat(64),
        }
    }

    #[test]
    fn static_manifest_schema_accepts_required_shape() {
        let manifest = StaticSheetManifest {
            schema: STATIC_SHEET_SCHEMA.to_string(),
            app: APP_NAME.to_string(),
            created_at: "2026-05-12T00:00:00Z".to_string(),
            collection_id: "collection_1".to_string(),
            sheet_type: "static_edit_sheet".to_string(),
            profile: StaticSheetProfile {
                cell_width: 200,
                cell_height: 200,
                columns: 5,
                gap_x: 8,
                gap_y: 8,
                border_x: 16,
                border_y: 16,
                background: "transparent".to_string(),
                read_order: "row_major".to_string(),
            },
            pages: vec![StaticSheetPage {
                page_index: 0,
                clean_sheet_file: "sheet_001.png".to_string(),
                guide_sheet_file: Some("sheet_guide_001.png".to_string()),
                width: 1048,
                height: 848,
            }],
            items: vec![StaticSheetManifestItem {
                icon_id: "icon_1".to_string(),
                piece_id: None,
                page_index: 0,
                row: 0,
                col: 0,
                index: 0,
                export_number: 1,
                x: 16,
                y: 16,
                w: 200,
                h: 200,
                display_name: "icon".to_string(),
                alt: "가".to_string(),
                icon_type: "single".to_string(),
                format: "png".to_string(),
                source_hash: Some("b".repeat(64)),
                render_hash: Some("def".to_string()),
                render_recipe_hash: Some("recipe".to_string()),
                visual_source: Some(valid_visual_source()),
            }],
        };

        validate_static_manifest(&manifest).unwrap();

        let mut unsafe_id = manifest.clone();
        unsafe_id.items[0].icon_id = "../../outside".to_string();
        assert_eq!(
            validate_static_manifest(&unsafe_id).unwrap_err().code,
            "manifest_path"
        );

        let mut unsafe_file_name = manifest.clone();
        unsafe_file_name.pages[0].clean_sheet_file = "..\\outside.png".to_string();
        assert_eq!(
            validate_static_manifest(&unsafe_file_name)
                .unwrap_err()
                .code,
            "manifest_path"
        );

        let mut reserved_file_name = manifest.clone();
        reserved_file_name.pages[0].clean_sheet_file = "CON.png".to_string();
        assert_eq!(
            validate_static_manifest(&reserved_file_name)
                .unwrap_err()
                .code,
            "manifest_path"
        );

        let mut overflow = manifest.clone();
        overflow.items[0].x = i64::MAX;
        overflow.items[0].w = 1;
        assert_eq!(
            validate_static_manifest(&overflow).unwrap_err().code,
            "manifest_validation"
        );

        let mut excessive = manifest;
        excessive.items = vec![excessive.items[0].clone(); 10_001];
        assert_eq!(
            validate_static_manifest(&excessive).unwrap_err().code,
            "manifest_workload"
        );
    }

    #[test]
    fn static_manifest_legacy_item_without_render_recipe_hash_is_supported() {
        let json = serde_json::json!({
            "schema": LEGACY_STATIC_SHEET_SCHEMA,
            "app": APP_NAME,
            "created_at": "2026-05-12T00:00:00Z",
            "collection_id": "collection_1",
            "sheet_type": "static_edit_sheet",
            "profile": {
                "cell_width": 20,
                "cell_height": 20,
                "columns": 1,
                "gap_x": 0,
                "gap_y": 0,
                "border_x": 0,
                "border_y": 0,
                "background": "transparent",
                "read_order": "row_major"
            },
            "pages": [{
                "page_index": 0,
                "clean_sheet_file": "sheet_001.png",
                "guide_sheet_file": null,
                "width": 20,
                "height": 20
            }],
            "items": [{
                "icon_id": "icon_1",
                "piece_id": "piece_1",
                "page_index": 0,
                "row": 0,
                "col": 0,
                "index": 0,
                "export_number": 1,
                "x": 0,
                "y": 0,
                "w": 20,
                "h": 20,
                "display_name": "icon",
                "alt": "가",
                "icon_type": "single",
                "format": "png",
                "source_hash": "abc",
                "render_hash": "def"
            }]
        });

        let manifest =
            super::read_static_manifest_bytes(&serde_json::to_vec(&json).unwrap()).unwrap();
        assert_eq!(manifest.items[0].render_recipe_hash, None);
    }

    fn valid_gif_manifest() -> GifFrameSheetManifest {
        GifFrameSheetManifest {
            schema: GIF_FRAME_SHEET_SCHEMA.to_string(),
            app: APP_NAME.to_string(),
            created_at: "2026-05-12T00:00:00Z".to_string(),
            icon_id: "icon_1".to_string(),
            source_file_id: Some("source_1".to_string()),
            source_hash: Some("b".repeat(64)),
            render_recipe_hash: None,
            visual_source: Some(valid_visual_source()),
            display_name: "icon".to_string(),
            loop_mode: "infinite".to_string(),
            loop_count: None,
            frame_count: 1,
            duration_ms: 100,
            frame_cell_width: 200,
            frame_cell_height: 200,
            columns: 1,
            gap_x: 0,
            gap_y: 0,
            border_x: 0,
            border_y: 0,
            background: "transparent".to_string(),
            pages: vec![GifFrameSheetPage {
                page_index: 0,
                clean_sheet_file: "frames_sheet_001.png".to_string(),
                guide_sheet_file: Some("frames_guide_001.png".to_string()),
                width: 200,
                height: 200,
            }],
            frames: vec![GifFrameManifestItem {
                frame_index: 0,
                sheet_file: "frames_sheet_001.png".to_string(),
                page_index: 0,
                row: 0,
                col: 0,
                x: 0,
                y: 0,
                w: 200,
                h: 200,
                duration_ms: 100,
                disposal_method: None,
                source_frame_hash: None,
            }],
        }
    }

    #[test]
    fn gif_manifest_rejects_unsafe_ids_and_sheet_paths() {
        let mut unsafe_id = valid_gif_manifest();
        unsafe_id.icon_id = "../outside".to_string();
        assert_eq!(
            validate_gif_manifest(&unsafe_id).unwrap_err().code,
            "manifest_path"
        );

        let mut unsafe_page = valid_gif_manifest();
        unsafe_page.pages[0].clean_sheet_file = "C:\\outside.png".to_string();
        assert_eq!(
            validate_gif_manifest(&unsafe_page).unwrap_err().code,
            "manifest_path"
        );

        let mut unsafe_frame = valid_gif_manifest();
        unsafe_frame.frames[0].sheet_file = "..\\outside.png".to_string();
        assert_eq!(
            validate_gif_manifest(&unsafe_frame).unwrap_err().code,
            "manifest_path"
        );

        let mut reserved_page = valid_gif_manifest();
        reserved_page.pages[0].clean_sheet_file = "NUL.png".to_string();
        reserved_page.frames[0].sheet_file = "NUL.png".to_string();
        assert_eq!(
            validate_gif_manifest(&reserved_page).unwrap_err().code,
            "manifest_path"
        );
    }

    #[test]
    fn gif_manifest_rejects_overflow_duration_mismatch_and_excessive_frames() {
        let mut overflow = valid_gif_manifest();
        overflow.frames[0].x = i64::MAX;
        overflow.frames[0].w = 1;
        assert_eq!(
            validate_gif_manifest(&overflow).unwrap_err().code,
            "manifest_validation"
        );

        let mut duration_mismatch = valid_gif_manifest();
        duration_mismatch.duration_ms = 101;
        assert_eq!(
            validate_gif_manifest(&duration_mismatch).unwrap_err().code,
            "manifest_validation"
        );

        let mut excessive = valid_gif_manifest();
        let template = excessive.frames[0].clone();
        excessive.frames = (0..501)
            .map(|index| {
                let mut frame = template.clone();
                frame.frame_index = index;
                frame
            })
            .collect();
        excessive.frame_count = 501;
        excessive.duration_ms = 50_100;
        assert_eq!(
            validate_gif_manifest(&excessive).unwrap_err().code,
            "manifest_workload"
        );
    }

    #[test]
    fn gif_manifest_rejects_wrong_schema() {
        let manifest = GifFrameSheetManifest {
            schema: STATIC_SHEET_SCHEMA.to_string(),
            app: APP_NAME.to_string(),
            created_at: "2026-05-12T00:00:00Z".to_string(),
            icon_id: "icon_1".to_string(),
            source_file_id: None,
            source_hash: None,
            render_recipe_hash: None,
            visual_source: None,
            display_name: "icon".to_string(),
            loop_mode: "infinite".to_string(),
            loop_count: None,
            frame_count: 0,
            duration_ms: 0,
            frame_cell_width: 200,
            frame_cell_height: 200,
            columns: 8,
            gap_x: 8,
            gap_y: 8,
            border_x: 16,
            border_y: 16,
            background: "transparent".to_string(),
            pages: Vec::new(),
            frames: Vec::new(),
        };

        assert!(validate_gif_manifest(&manifest).is_err());
        assert_eq!(GIF_FRAME_SHEET_SCHEMA, "pmtcon-gif-frame-sheet-v2");
    }
}
