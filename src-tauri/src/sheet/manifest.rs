#![allow(dead_code)]

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

pub const STATIC_SHEET_SCHEMA: &str = "pmtcon-sheet-v1";
pub const GIF_FRAME_SHEET_SCHEMA: &str = "pmtcon-gif-frame-sheet-v1";
pub const APP_NAME: &str = "PMTCONCON Studio";

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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GifFrameSheetManifest {
    pub schema: String,
    pub app: String,
    pub created_at: String,
    pub icon_id: String,
    pub source_hash: Option<String>,
    pub loop_mode: String,
    pub frame_cell_width: i64,
    pub frame_cell_height: i64,
    pub columns: i64,
    pub gap_x: i64,
    pub gap_y: i64,
    pub border_x: i64,
    pub border_y: i64,
    pub pages: Vec<GifFrameSheetPage>,
    pub frames: Vec<GifFrameManifestItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GifFrameSheetPage {
    pub page_index: i64,
    pub sheet_file: String,
    pub guide_sheet_file: Option<String>,
    pub width: i64,
    pub height: i64,
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
    let bytes = fs::read(path)?;
    read_static_manifest_bytes(&bytes)
}

pub fn read_static_manifest_bytes(bytes: &[u8]) -> AppResult<StaticSheetManifest> {
    let manifest: StaticSheetManifest = serde_json::from_slice(&bytes)
        .map_err(|error| AppError::new("manifest", error.to_string()))?;
    validate_static_manifest(&manifest)?;
    Ok(manifest)
}

pub fn validate_static_manifest(manifest: &StaticSheetManifest) -> AppResult<()> {
    if manifest.schema != STATIC_SHEET_SCHEMA {
        return Err(AppError::new(
            "manifest_schema",
            "pmtcon-sheet-v1 매니페스트가 아닙니다.",
        ));
    }
    if manifest.app != APP_NAME {
        return Err(AppError::new(
            "manifest_app",
            "PMTCONCON Studio 매니페스트가 아닙니다.",
        ));
    }
    if manifest.profile.cell_width <= 0
        || manifest.profile.cell_height <= 0
        || manifest.profile.columns <= 0
    {
        return Err(AppError::new(
            "manifest_validation",
            "시트 매니페스트의 셀 크기 또는 열 수가 올바르지 않습니다.",
        ));
    }
    if manifest.pages.is_empty() {
        return Err(AppError::new(
            "manifest_validation",
            "시트 매니페스트에 페이지 정보가 없습니다.",
        ));
    }
    for item in &manifest.items {
        if item.w <= 0 || item.h <= 0 {
            return Err(AppError::new(
                "manifest_validation",
                "시트 매니페스트에 잘못된 셀 영역이 있습니다.",
            ));
        }
    }
    Ok(())
}

pub fn validate_gif_manifest(manifest: &GifFrameSheetManifest) -> AppResult<()> {
    if manifest.schema != GIF_FRAME_SHEET_SCHEMA {
        return Err(AppError::new(
            "manifest_schema",
            "pmtcon-gif-frame-sheet-v1 매니페스트가 아닙니다.",
        ));
    }
    if manifest.frame_cell_width <= 0 || manifest.frame_cell_height <= 0 {
        return Err(AppError::new(
            "manifest_validation",
            "GIF 프레임 시트 셀 크기가 올바르지 않습니다.",
        ));
    }
    if manifest.frames.is_empty() {
        return Err(AppError::new(
            "manifest_validation",
            "GIF 프레임 매니페스트에 프레임 정보가 없습니다.",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        validate_gif_manifest, validate_static_manifest, GifFrameSheetManifest,
        StaticSheetManifest, StaticSheetManifestItem, StaticSheetPage, StaticSheetProfile,
        APP_NAME, GIF_FRAME_SHEET_SCHEMA, STATIC_SHEET_SCHEMA,
    };

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
                source_hash: Some("abc".to_string()),
                render_hash: Some("def".to_string()),
            }],
        };

        validate_static_manifest(&manifest).unwrap();
    }

    #[test]
    fn gif_manifest_rejects_wrong_schema() {
        let manifest = GifFrameSheetManifest {
            schema: STATIC_SHEET_SCHEMA.to_string(),
            app: APP_NAME.to_string(),
            created_at: "2026-05-12T00:00:00Z".to_string(),
            icon_id: "icon_1".to_string(),
            source_hash: None,
            loop_mode: "infinite".to_string(),
            frame_cell_width: 200,
            frame_cell_height: 200,
            columns: 8,
            gap_x: 8,
            gap_y: 8,
            border_x: 16,
            border_y: 16,
            pages: Vec::new(),
            frames: Vec::new(),
        };

        assert!(validate_gif_manifest(&manifest).is_err());
        assert_eq!(GIF_FRAME_SHEET_SCHEMA, "pmtcon-gif-frame-sheet-v1");
    }
}
