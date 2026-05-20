use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use image::imageops;
use image::ImageFormat;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::paths::AppPaths;

use super::importer::{create_icons_from_png_cells, png_bytes_from_rgba, CellImportInput};
use super::manifest::{
    read_static_manifest, read_static_manifest_bytes, StaticSheetManifest, StaticSheetManifestItem,
};
use super::path_string;
use crate::models::ImportImageFilePayload;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReimportEditSheetRequest {
    pub manifest_path: String,
    pub manifest_file: Option<ImportImageFilePayload>,
    #[serde(default)]
    pub edited_sheet_paths: Vec<String>,
    #[serde(default)]
    pub edited_sheet_files: Vec<ImportImageFilePayload>,
    pub target_collection_id: String,
    #[serde(default = "default_reimport_mode")]
    pub reimport_mode: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReimportEditSheetResult {
    pub updated_items: Vec<ReimportedItem>,
    pub created_variants: Vec<String>,
    pub skipped_items: Vec<SkippedReimportItem>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReimportedItem {
    pub icon_id: String,
    pub piece_id: Option<String>,
    pub new_icon_id: Option<String>,
    pub variant_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedReimportItem {
    pub index: i64,
    pub icon_id: String,
    pub reason: String,
}

pub fn reimport_edit_sheet(
    connection: &mut Connection,
    paths: &AppPaths,
    request: ReimportEditSheetRequest,
) -> AppResult<ReimportEditSheetResult> {
    let manifest_path = PathBuf::from(request.manifest_path.trim());
    let manifest = match request.manifest_file.as_ref() {
        Some(file) => read_static_manifest_bytes(&file.bytes)?,
        None => read_static_manifest(&manifest_path)?,
    };
    let sheet_paths = resolve_sheet_paths(&manifest, &manifest_path, &request.edited_sheet_paths);
    let sheet_files = request
        .edited_sheet_files
        .iter()
        .map(|file| (file.original_filename.clone(), file.bytes.clone()))
        .collect::<HashMap<_, _>>();
    let mut page_images = HashMap::new();
    let mut warnings = Vec::new();
    let mut errors = Vec::new();

    for page in &manifest.pages {
        let image = if let Some(bytes) = sheet_files.get(&page.clean_sheet_file) {
            match image::load_from_memory_with_format(bytes, ImageFormat::Png) {
                Ok(image) => image.to_rgba8(),
                Err(error) => {
                    errors.push(format!("{}: {}", page.clean_sheet_file, error));
                    continue;
                }
            }
        } else {
            let Some(path) = sheet_paths.get(&page.page_index) else {
                errors.push(format!(
                    "{} 페이지의 수정된 시트 파일이 없습니다.",
                    page.page_index + 1
                ));
                continue;
            };
            match image::open(path) {
                Ok(image) => image.to_rgba8(),
                Err(error) => {
                    errors.push(format!("{}: {}", path.display(), error));
                    continue;
                }
            }
        };
        if i64::from(image.width()) != page.width || i64::from(image.height()) != page.height {
            warnings.push(format!(
                "{} 페이지 크기가 매니페스트와 다릅니다. 매니페스트 영역 기준으로 가능한 셀만 가져옵니다.",
                page.page_index + 1
            ));
        }
        page_images.insert(page.page_index, image);
    }

    if page_images.is_empty() {
        return Ok(ReimportEditSheetResult {
            updated_items: Vec::new(),
            created_variants: Vec::new(),
            skipped_items: Vec::new(),
            warnings,
            errors,
        });
    }

    let mut skipped_items = Vec::new();
    let mut updated_items = Vec::new();
    let mut created_variants = Vec::new();
    let mut new_icon_cells = Vec::new();
    let item_indexes = manifest
        .items
        .iter()
        .map(|item| item.index)
        .collect::<HashSet<_>>();

    if item_indexes.len() != manifest.items.len() {
        warnings.push("매니페스트에 중복된 셀 index가 있습니다.".to_string());
    }

    let should_create_new_icons = !matches!(
        request.reimport_mode.as_str(),
        "create_processed_variants" | "replace_processed_output_only"
    );

    for item in &manifest.items {
        let Some(page) = page_images.get(&item.page_index) else {
            skipped_items.push(skip_item(item, "대상 페이지를 읽을 수 없습니다."));
            continue;
        };
        if item.x < 0
            || item.y < 0
            || item.w <= 0
            || item.h <= 0
            || item.x + item.w > i64::from(page.width())
            || item.y + item.h > i64::from(page.height())
        {
            skipped_items.push(skip_item(
                item,
                "셀 영역이 수정된 시트 이미지 밖으로 나갑니다.",
            ));
            continue;
        }

        let cropped = imageops::crop_imm(
            page,
            item.x as u32,
            item.y as u32,
            item.w as u32,
            item.h as u32,
        )
        .to_image();
        let bytes = png_bytes_from_rgba(&cropped)?;

        match request.reimport_mode.as_str() {
            "create_processed_variants" | "replace_processed_output_only" => {
                let variant_path = write_reimport_variant(paths, item, &bytes)?;
                created_variants.push(path_string(&variant_path));
                updated_items.push(ReimportedItem {
                    icon_id: item.icon_id.clone(),
                    piece_id: item.piece_id.clone(),
                    new_icon_id: None,
                    variant_path: Some(path_string(&variant_path)),
                });
            }
            _ => {
                new_icon_cells.push((
                    item.clone(),
                    CellImportInput {
                        original_filename: format!("{}_sheet_reimport.png", item.icon_id),
                        bytes,
                        display_name: format!("{} sheet edit", item.display_name),
                        alt_text: String::new(),
                        cell_width: Some(item.w),
                        cell_height: Some(item.h),
                    },
                ));
            }
        }
    }

    if should_create_new_icons {
        let cells = new_icon_cells
            .iter()
            .map(|(_, cell)| cell.clone())
            .collect::<Vec<_>>();
        if !cells.is_empty() {
            let icons = create_icons_from_png_cells(
                connection,
                paths,
                &request.target_collection_id,
                cells,
            )?;
            for ((item, _), icon) in new_icon_cells.into_iter().zip(icons) {
                updated_items.push(ReimportedItem {
                    icon_id: item.icon_id,
                    piece_id: item.piece_id,
                    new_icon_id: Some(icon.id),
                    variant_path: None,
                });
            }
        }
    }

    Ok(ReimportEditSheetResult {
        updated_items,
        created_variants,
        skipped_items,
        warnings,
        errors,
    })
}

fn resolve_sheet_paths(
    manifest: &StaticSheetManifest,
    manifest_path: &Path,
    explicit_paths: &[String],
) -> HashMap<i64, PathBuf> {
    let explicit_paths = explicit_paths
        .iter()
        .map(|path| PathBuf::from(path.trim()))
        .filter(|path| !path.as_os_str().is_empty())
        .collect::<Vec<_>>();
    if explicit_paths.len() == manifest.pages.len() {
        let mut pages = manifest.pages.iter().collect::<Vec<_>>();
        pages.sort_by_key(|page| page.page_index);
        return pages
            .into_iter()
            .zip(explicit_paths)
            .map(|(page, path)| (page.page_index, path))
            .collect();
    }

    let mut by_file_name = explicit_paths
        .iter()
        .filter_map(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(|name| (name.to_string(), path.clone()))
        })
        .collect::<HashMap<_, _>>();

    let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    let mut output = HashMap::new();
    for page in &manifest.pages {
        if let Some(path) = by_file_name.remove(&page.clean_sheet_file) {
            output.insert(page.page_index, path);
            continue;
        }

        let same_dir = manifest_dir.join(&page.clean_sheet_file);
        if same_dir.exists() {
            output.insert(page.page_index, same_dir);
            continue;
        }

        let clean_dir = manifest_dir.join("clean").join(&page.clean_sheet_file);
        if clean_dir.exists() {
            output.insert(page.page_index, clean_dir);
        }
    }
    output
}

fn write_reimport_variant(
    paths: &AppPaths,
    item: &StaticSheetManifestItem,
    bytes: &[u8],
) -> AppResult<PathBuf> {
    let file_name = format!("{}_{}_sheet_reimport.png", item.icon_id, item.index);
    let path = paths.sheet_reimport_variants_dir.join(file_name);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, bytes)?;

    if image::load_from_memory_with_format(bytes, ImageFormat::Png).is_err() {
        return Err(AppError::new(
            "image",
            "다시 가져온 셀 PNG를 검증할 수 없습니다.",
        ));
    }

    Ok(path)
}

fn skip_item(item: &StaticSheetManifestItem, reason: &str) -> SkippedReimportItem {
    SkippedReimportItem {
        index: item.index,
        icon_id: item.icon_id.clone(),
        reason: reason.to_string(),
    }
}

fn default_reimport_mode() -> String {
    "create_new_icons".to_string()
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::time::{SystemTime, UNIX_EPOCH};

    use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};
    use rusqlite::Connection;

    use crate::db::migrations;
    use crate::db::repositories::collections::create_collection;
    use crate::models::ImportImageFilePayload;
    use crate::paths::AppPaths;
    use crate::sheet::manifest::{
        StaticSheetManifest, StaticSheetManifestItem, StaticSheetPage, StaticSheetProfile,
        APP_NAME, STATIC_SHEET_SCHEMA,
    };

    use super::{reimport_edit_sheet, ReimportEditSheetRequest};

    fn connection() -> Connection {
        let mut connection = Connection::open_in_memory().unwrap();
        migrations::run(&mut connection).unwrap();
        connection
    }

    fn temp_paths() -> AppPaths {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        AppPaths::prepare(std::env::temp_dir().join(format!("pmtconcon-sheet-reimport-{suffix}")))
            .unwrap()
    }

    #[test]
    fn reimport_maps_manifest_cell_to_new_icon_without_overwriting_original() {
        let mut connection = connection();
        let paths = temp_paths();
        let collection =
            create_collection(&mut connection, Some("sheet reimport".to_string())).unwrap();
        let image = ImageBuffer::from_pixel(20, 20, Rgba([0, 128, 255, 77]));
        let mut image_cursor = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image)
            .write_to(&mut image_cursor, ImageFormat::Png)
            .unwrap();
        let sheet_bytes = image_cursor.into_inner();
        let manifest = StaticSheetManifest {
            schema: STATIC_SHEET_SCHEMA.to_string(),
            app: APP_NAME.to_string(),
            created_at: "2026-05-12T00:00:00Z".to_string(),
            collection_id: collection.id.clone(),
            sheet_type: "static_edit_sheet".to_string(),
            profile: StaticSheetProfile {
                cell_width: 20,
                cell_height: 20,
                columns: 1,
                gap_x: 0,
                gap_y: 0,
                border_x: 0,
                border_y: 0,
                background: "transparent".to_string(),
                read_order: "row_major".to_string(),
            },
            pages: vec![StaticSheetPage {
                page_index: 0,
                clean_sheet_file: "sheet_001.png".to_string(),
                guide_sheet_file: None,
                width: 20,
                height: 20,
            }],
            items: vec![StaticSheetManifestItem {
                icon_id: "icon_original".to_string(),
                piece_id: Some("piece_original".to_string()),
                page_index: 0,
                row: 0,
                col: 0,
                index: 0,
                export_number: 1,
                x: 0,
                y: 0,
                w: 20,
                h: 20,
                display_name: "original".to_string(),
                alt: "가".to_string(),
                icon_type: "single".to_string(),
                format: "png".to_string(),
                source_hash: Some("source".to_string()),
                render_hash: Some("render".to_string()),
            }],
        };
        let manifest_bytes = serde_json::to_vec(&manifest).unwrap();

        let result = reimport_edit_sheet(
            &mut connection,
            &paths,
            ReimportEditSheetRequest {
                manifest_path: String::new(),
                manifest_file: Some(ImportImageFilePayload {
                    original_filename: "sheet_manifest.json".to_string(),
                    bytes: manifest_bytes,
                }),
                edited_sheet_paths: Vec::new(),
                edited_sheet_files: vec![ImportImageFilePayload {
                    original_filename: "sheet_001.png".to_string(),
                    bytes: sheet_bytes,
                }],
                target_collection_id: collection.id,
                reimport_mode: "create_new_icons".to_string(),
            },
        )
        .unwrap();

        assert_eq!(result.updated_items.len(), 1);
        assert!(result.updated_items[0].new_icon_id.is_some());
        assert!(result.errors.is_empty());

        let source_path: String = connection
            .query_row(
                "SELECT s.original_path_in_library
                 FROM source_files s
                 JOIN icons i ON i.source_file_id = s.id
                 WHERE i.id = ?1",
                [result.updated_items[0].new_icon_id.as_ref().unwrap()],
                |row| row.get(0),
            )
            .unwrap();
        let imported = image::open(source_path).unwrap().to_rgba8();
        assert_eq!(imported.get_pixel(0, 0).0[3], 77);

        std::fs::remove_dir_all(paths.root).unwrap();
    }
}
