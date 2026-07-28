use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use image::imageops;
use image::ImageFormat;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::imaging::import_limits::{
    decode_import_image, read_import_file_bytes, validate_import_file_size,
    MAX_GIF_TOTAL_FRAME_PIXELS, MAX_IMPORT_FILE_BYTES,
};
use crate::paths::AppPaths;

use super::exporter::current_static_sheet_render_guard;
use super::importer::{create_icons_from_png_cells, png_bytes_from_rgba, CellImportInput};
use super::manifest::{
    read_static_manifest_bytes, ManifestVisualSource, StaticSheetManifest, StaticSheetManifestItem,
    LEGACY_STATIC_SHEET_SCHEMA,
};
use super::path_string;
use crate::models::ImportImageFilePayload;

const MAX_REIMPORT_TOTAL_ENCODED_BYTES: usize = MAX_IMPORT_FILE_BYTES;

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
    let manifest_file_supplied = request.manifest_file.is_some();
    let (manifest, manifest_payload_bytes) = match request.manifest_file.as_ref() {
        Some(file) => (read_static_manifest_bytes(&file.bytes)?, file.bytes.len()),
        None => {
            let bytes = read_import_file_bytes(&manifest_path)?;
            let byte_size = bytes.len();
            (read_static_manifest_bytes(&bytes)?, byte_size)
        }
    };
    let mut total_encoded_bytes = manifest_payload_bytes;
    for file in &request.edited_sheet_files {
        validate_import_file_size(file.bytes.len())?;
        total_encoded_bytes = total_encoded_bytes
            .checked_add(file.bytes.len())
            .ok_or_else(|| {
                AppError::new(
                    "manifest_workload",
                    "선택한 정적 시트 PNG의 전체 파일 크기가 너무 큽니다.",
                )
            })?;
    }
    if total_encoded_bytes > MAX_REIMPORT_TOTAL_ENCODED_BYTES {
        return Err(AppError::new(
            "manifest_workload",
            "선택한 정적 시트 PNG는 합계 64MB까지 처리할 수 있습니다.",
        ));
    }
    let allow_sibling_lookup = !manifest_file_supplied && request.edited_sheet_files.is_empty();
    let sheet_paths = resolve_sheet_paths(
        &manifest,
        &manifest_path,
        &request.edited_sheet_paths,
        allow_sibling_lookup,
    );
    let sheet_files = request
        .edited_sheet_files
        .iter()
        .map(|file| (file.original_filename.as_str(), file.bytes.as_slice()))
        .collect::<HashMap<_, _>>();
    let mut page_images = HashMap::new();
    let mut actual_decoded_page_pixels = 0_u64;
    let mut warnings = Vec::new();
    let mut errors = Vec::new();

    for page in &manifest.pages {
        let image = if let Some(bytes) = sheet_files.get(page.clean_sheet_file.as_str()) {
            match decode_import_image(bytes, ImageFormat::Png) {
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
            let bytes = match read_import_file_bytes(path) {
                Ok(bytes) => bytes,
                Err(error) => {
                    errors.push(format!("{}: {}", path.display(), error));
                    continue;
                }
            };
            let next_total_encoded_bytes = total_encoded_bytes
                .checked_add(bytes.len())
                .ok_or_else(|| {
                    AppError::new(
                        "manifest_workload",
                        "선택한 정적 시트 PNG의 전체 파일 크기가 너무 큽니다.",
                    )
                })?;
            if next_total_encoded_bytes > MAX_REIMPORT_TOTAL_ENCODED_BYTES {
                return Err(AppError::new(
                    "manifest_workload",
                    "선택한 정적 시트 PNG는 합계 64MB까지 처리할 수 있습니다.",
                ));
            }
            total_encoded_bytes = next_total_encoded_bytes;
            match decode_import_image(&bytes, ImageFormat::Png) {
                Ok(image) => image.to_rgba8(),
                Err(error) => {
                    errors.push(format!("{}: {}", path.display(), error));
                    continue;
                }
            }
        };
        let page_pixels = u64::from(image.width()).saturating_mul(u64::from(image.height()));
        let next_total_page_pixels = actual_decoded_page_pixels
            .checked_add(page_pixels)
            .ok_or_else(|| {
                AppError::new(
                    "manifest_workload",
                    "디코드한 정적 시트 페이지의 전체 픽셀 수가 너무 큽니다.",
                )
            })?;
        if next_total_page_pixels > MAX_GIF_TOTAL_FRAME_PIXELS {
            return Err(AppError::new(
                "manifest_workload",
                "디코드한 정적 시트 페이지의 전체 픽셀 수가 지원 범위를 벗어났습니다.",
            ));
        }
        actual_decoded_page_pixels = next_total_page_pixels;
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
    let mut warned_legacy_recipe_hash = false;
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
    let should_guard_existing_output = !should_create_new_icons;

    for item in &manifest.items {
        if should_guard_existing_output {
            match current_static_sheet_render_guard(
                connection,
                &request.target_collection_id,
                &item.icon_id,
                item.piece_id.as_deref(),
                item.w,
                item.h,
            ) {
                Ok((current_source_hash, current_recipe_hash)) => {
                    if let Err(error) = validate_static_item_visual_source(
                        connection,
                        &request.target_collection_id,
                        item,
                        &manifest.schema,
                    ) {
                        skipped_items.push(skip_item(
                            item,
                            &format!(
                                "작업 시트 source provenance가 현재 아이콘과 다릅니다: {}",
                                error.message
                            ),
                        ));
                        continue;
                    }
                    if let Some(expected_source_hash) = item.source_hash.as_deref() {
                        if expected_source_hash != current_source_hash {
                            skipped_items.push(skip_item(
                                item,
                                "작업 시트를 내보낸 뒤 원본 이미지가 변경되어 가공본을 적용하지 않았습니다.",
                            ));
                            continue;
                        }
                    }
                    match item.render_recipe_hash.as_deref() {
                        Some(expected) if expected != current_recipe_hash => {
                            skipped_items.push(skip_item(
                                item,
                                "작업 시트를 내보낸 뒤 자르기·회전·반전·텍스트·정적 효과 또는 모션 recipe가 변경되어 가공본을 적용하지 않았습니다.",
                            ));
                            continue;
                        }
                        None if !warned_legacy_recipe_hash => {
                            warnings.push(
                                "이전 버전 작업 시트 manifest에는 render recipe hash가 없어 원본 해시만 확인했습니다."
                                    .to_string(),
                            );
                            warned_legacy_recipe_hash = true;
                        }
                        _ => {}
                    }
                }
                Err(error) => {
                    skipped_items.push(skip_item(
                        item,
                        &format!(
                            "현재 원본 상태를 확인할 수 없어 가공본을 적용하지 않았습니다: {}",
                            error.message
                        ),
                    ));
                    continue;
                }
            }
        }

        let Some(page) = page_images.get(&item.page_index) else {
            skipped_items.push(skip_item(item, "대상 페이지를 읽을 수 없습니다."));
            continue;
        };
        if !static_item_fits_image(item, page.width(), page.height()) {
            skipped_items.push(skip_item(
                item,
                "셀 영역이 수정된 시트 이미지 밖으로 나갑니다.",
            ));
            continue;
        }

        let x = u32::try_from(item.x)
            .map_err(|_| AppError::new("validation", "셀 x 좌표가 올바르지 않습니다."))?;
        let y = u32::try_from(item.y)
            .map_err(|_| AppError::new("validation", "셀 y 좌표가 올바르지 않습니다."))?;
        let width = u32::try_from(item.w)
            .map_err(|_| AppError::new("validation", "셀 너비가 올바르지 않습니다."))?;
        let height = u32::try_from(item.h)
            .map_err(|_| AppError::new("validation", "셀 높이가 올바르지 않습니다."))?;
        let cropped = imageops::crop_imm(page, x, y, width, height).to_image();
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

fn static_item_fits_image(
    item: &StaticSheetManifestItem,
    image_width: u32,
    image_height: u32,
) -> bool {
    let Ok(x) = u32::try_from(item.x) else {
        return false;
    };
    let Ok(y) = u32::try_from(item.y) else {
        return false;
    };
    let Ok(width) = u32::try_from(item.w) else {
        return false;
    };
    let Ok(height) = u32::try_from(item.h) else {
        return false;
    };
    let Some(right) = x.checked_add(width) else {
        return false;
    };
    let Some(bottom) = y.checked_add(height) else {
        return false;
    };
    width > 0 && height > 0 && right <= image_width && bottom <= image_height
}

fn validate_static_item_visual_source(
    connection: &Connection,
    collection_id: &str,
    item: &StaticSheetManifestItem,
    schema: &str,
) -> AppResult<()> {
    let current = connection.query_row(
        "SELECT original_source_file_id, original_source_sha256,
                original_lineage_id, original_lineage_generation,
                effective_source_file_id, effective_source_sha256, active_version_id
         FROM effective_visual_sources
         WHERE icon_id = ?1 AND collection_id = ?2",
        params![item.icon_id, collection_id],
        |row| {
            Ok((
                ManifestVisualSource {
                    original_source_file_id: row.get("original_source_file_id")?,
                    original_source_hash: row.get("original_source_sha256")?,
                    original_lineage_id: row.get("original_lineage_id")?,
                    original_lineage_generation: row.get("original_lineage_generation")?,
                    effective_source_file_id: row.get("effective_source_file_id")?,
                    effective_source_hash: row.get("effective_source_sha256")?,
                },
                row.get::<_, Option<String>>("active_version_id")?,
            ))
        },
    )?;

    if schema == LEGACY_STATIC_SHEET_SCHEMA {
        if current.1.is_none() && current.0.original_lineage_generation == 0 {
            return Ok(());
        }
        return Err(AppError::new(
            "manifest_stale",
            "AI 버전이 활성화되었거나 원본 계보가 바뀐 아이콘에는 legacy v1 정적 시트를 적용할 수 없습니다.",
        ));
    }

    if item.visual_source.as_ref() != Some(&current.0) {
        return Err(AppError::new(
            "manifest_stale",
            "정적 시트를 내보낸 뒤 원본 계보 또는 AI 렌더 소스가 바뀌었습니다.",
        ));
    }
    Ok(())
}

fn resolve_sheet_paths(
    manifest: &StaticSheetManifest,
    manifest_path: &Path,
    explicit_paths: &[String],
    allow_sibling_lookup: bool,
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

        if allow_sibling_lookup {
            if let Some(same_dir) =
                contained_manifest_path(manifest_dir, Path::new(&page.clean_sheet_file))
            {
                output.insert(page.page_index, same_dir);
                continue;
            }

            let clean_relative = Path::new("clean").join(&page.clean_sheet_file);
            if let Some(clean_dir) = contained_manifest_path(manifest_dir, &clean_relative) {
                output.insert(page.page_index, clean_dir);
            }
        }
    }
    output
}

fn contained_manifest_path(manifest_dir: &Path, relative_path: &Path) -> Option<PathBuf> {
    if relative_path.is_absolute() {
        return None;
    }
    let canonical_root = fs::canonicalize(manifest_dir).ok()?;
    let candidate = fs::canonicalize(manifest_dir.join(relative_path)).ok()?;
    candidate.starts_with(&canonical_root).then_some(candidate)
}

fn write_reimport_variant(
    paths: &AppPaths,
    item: &StaticSheetManifestItem,
    bytes: &[u8],
) -> AppResult<PathBuf> {
    if decode_import_image(bytes, ImageFormat::Png).is_err() {
        return Err(AppError::new(
            "image",
            "다시 가져온 셀 PNG를 검증할 수 없습니다.",
        ));
    }

    let file_name = format!("{}_{}_sheet_reimport.png", item.icon_id, item.index);
    let path = paths.sheet_reimport_variants_dir.join(file_name);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, bytes)?;
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
    use crate::db::repositories::imports::import_image_files;
    use crate::db::repositories::motion::upsert_motion_recipe;
    use crate::imaging::motion::{MotionRecipe, SpatialMotion};
    use crate::models::ImportImageFilePayload;
    use crate::paths::AppPaths;
    use crate::sheet::exporter::{export_edit_sheet, ExportEditSheetRequest};
    use crate::sheet::manifest::{
        read_static_manifest, write_static_manifest, StaticSheetManifest, StaticSheetManifestItem,
        StaticSheetPage, StaticSheetProfile, APP_NAME, LEGACY_STATIC_SHEET_SCHEMA,
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

    fn export_request(collection_id: &str, output_directory: String) -> ExportEditSheetRequest {
        ExportEditSheetRequest {
            collection_id: collection_id.to_string(),
            selected_icon_ids: Vec::new(),
            source: "current_collection".to_string(),
            cell_width: 20,
            cell_height: 20,
            columns: 1,
            gap_x: 0,
            gap_y: 0,
            border_x: 0,
            border_y: 0,
            background: "transparent".to_string(),
            include_clean_sheet: true,
            include_guide_sheet: false,
            include_manifest: true,
            label_options: None,
            max_sheet_width: 2048,
            max_sheet_height: 2048,
            output_directory: Some(output_directory),
            open_output_folder: false,
        }
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
            schema: LEGACY_STATIC_SHEET_SCHEMA.to_string(),
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
                render_recipe_hash: None,
                visual_source: None,
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

    #[test]
    fn processed_reimport_detects_motion_recipe_stale_when_zero_ms_poster_is_unchanged() {
        let mut connection = connection();
        let paths = temp_paths();
        let collection =
            create_collection(&mut connection, Some("motion stale sheet".to_string())).unwrap();
        let image = ImageBuffer::from_fn(20, 20, |x, y| {
            Rgba([(x * 9) as u8, (y * 11) as u8, ((x + y) * 5) as u8, 255])
        });
        let mut image_cursor = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image)
            .write_to(&mut image_cursor, ImageFormat::Png)
            .unwrap();
        let imported = import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![ImportImageFilePayload {
                original_filename: "motion.png".to_string(),
                bytes: image_cursor.into_inner(),
            }],
        )
        .unwrap();
        let icon_id = imported.imported_icons[0].id.clone();
        let first_recipe = MotionRecipe {
            duration_ms: 1_000,
            seed: 17,
            spatial: Some(SpatialMotion::Shake {
                enabled: true,
                cycles_per_loop: 1,
                amplitude_x: 2,
                amplitude_y: 2,
            }),
            ..MotionRecipe::default()
        };
        let transaction = connection.transaction().unwrap();
        upsert_motion_recipe(&transaction, &collection.id, &icon_id, 0, &first_recipe).unwrap();
        transaction.commit().unwrap();

        let first_export = export_edit_sheet(
            &connection,
            &paths,
            export_request(
                &collection.id,
                paths
                    .root
                    .join("motion-sheet-first")
                    .to_string_lossy()
                    .to_string(),
            ),
        )
        .unwrap();
        let first_manifest_path = first_export.manifest_path.clone().unwrap();
        let first_manifest =
            read_static_manifest(std::path::Path::new(&first_manifest_path)).unwrap();
        let first_recipe_hash = first_manifest.items[0]
            .render_recipe_hash
            .clone()
            .expect("new static manifests must include a render recipe hash");
        assert_eq!(first_recipe_hash.len(), 64);

        let mut legacy_manifest = first_manifest.clone();
        for item in &mut legacy_manifest.items {
            item.render_recipe_hash = None;
        }
        let legacy_manifest_path = paths.root.join("legacy_static_manifest.json");
        write_static_manifest(&legacy_manifest_path, &legacy_manifest).unwrap();
        let legacy_result = reimport_edit_sheet(
            &mut connection,
            &paths,
            ReimportEditSheetRequest {
                manifest_path: legacy_manifest_path.to_string_lossy().to_string(),
                manifest_file: None,
                edited_sheet_paths: first_export.clean_sheet_paths.clone(),
                edited_sheet_files: Vec::new(),
                target_collection_id: collection.id.clone(),
                reimport_mode: "create_processed_variants".to_string(),
            },
        )
        .unwrap();
        assert_eq!(legacy_result.updated_items.len(), 1);
        assert_eq!(legacy_result.created_variants.len(), 1);
        assert!(legacy_result.skipped_items.is_empty());
        assert!(legacy_result
            .warnings
            .iter()
            .any(|warning| warning.contains("render recipe hash")));

        let second_recipe = MotionRecipe {
            duration_ms: 2_000,
            ..first_recipe
        };
        let transaction = connection.transaction().unwrap();
        upsert_motion_recipe(&transaction, &collection.id, &icon_id, 1, &second_recipe).unwrap();
        transaction.commit().unwrap();

        let second_export = export_edit_sheet(
            &connection,
            &paths,
            export_request(
                &collection.id,
                paths
                    .root
                    .join("motion-sheet-second")
                    .to_string_lossy()
                    .to_string(),
            ),
        )
        .unwrap();
        let first_poster = image::open(&first_export.clean_sheet_paths[0])
            .unwrap()
            .to_rgba8();
        let second_poster = image::open(&second_export.clean_sheet_paths[0])
            .unwrap()
            .to_rgba8();
        assert_eq!(first_poster.as_raw(), second_poster.as_raw());
        let second_manifest = read_static_manifest(std::path::Path::new(
            second_export.manifest_path.as_ref().unwrap(),
        ))
        .unwrap();
        assert_ne!(
            first_recipe_hash,
            second_manifest.items[0]
                .render_recipe_hash
                .as_deref()
                .unwrap()
        );

        let stale_result = reimport_edit_sheet(
            &mut connection,
            &paths,
            ReimportEditSheetRequest {
                manifest_path: first_manifest_path,
                manifest_file: None,
                edited_sheet_paths: first_export.clean_sheet_paths,
                edited_sheet_files: Vec::new(),
                target_collection_id: collection.id,
                reimport_mode: "replace_processed_output_only".to_string(),
            },
        )
        .unwrap();
        assert!(stale_result.updated_items.is_empty());
        assert!(stale_result.created_variants.is_empty());
        assert_eq!(stale_result.skipped_items.len(), 1);
        assert!(stale_result.skipped_items[0].reason.contains("모션 recipe"));

        std::fs::remove_dir_all(paths.root).unwrap();
    }
}
