use std::collections::HashSet;
use std::fmt::Write as FmtWrite;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use image::imageops;
use image::{DynamicImage, ImageFormat, RgbaImage};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::db::repositories::icons as icon_repository;
use crate::db::repositories::source_files::{
    import_source_file_from_bytes, SourceFileImportOptions, StoredSourceFile,
};
use crate::error::{AppError, AppResult};
use crate::ids::create_id;
use crate::imaging::import_limits::{decode_import_image, validate_import_dimensions};
use crate::models::{IconDto, ImportImageFilePayload};
use crate::paths::AppPaths;

use super::grid::{
    alpha_warning_for_extension, analyze_rgba_grid, SheetCell, SheetGridSettings, MAX_SHEET_CELLS,
};
use super::{image_format_for_extension, path_string, read_sheet_image_input};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSheetCellsRequest {
    pub sheet_path: Option<String>,
    pub sheet_file: Option<ImportImageFilePayload>,
    pub target_collection_id: String,
    pub grid_settings: SheetGridSettings,
    #[serde(default)]
    pub selected_cell_indexes: Vec<i64>,
    pub default_display_name_pattern: Option<String>,
    #[serde(default = "default_preserve_alpha")]
    pub preserve_alpha: bool,
    pub output_cell_width: Option<i64>,
    pub output_cell_height: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSheetCellsResult {
    pub imported_icons: Vec<IconDto>,
    pub skipped_cells: Vec<SkippedSheetCell>,
    pub warnings: Vec<String>,
    pub preserved_sheet_path: String,
    pub imported_count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedSheetCell {
    pub index: i64,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub(crate) struct CellImportInput {
    pub original_filename: String,
    pub bytes: Vec<u8>,
    pub display_name: String,
    pub alt_text: String,
    pub cell_width: Option<i64>,
    pub cell_height: Option<i64>,
}

#[derive(Debug)]
struct CollectionSheetImportRecord {
    id: String,
    cover_icon_id: Option<String>,
    cover_source_file_id: Option<String>,
    default_cell_width: i64,
    default_cell_height: i64,
}

pub fn import_sheet_cells(
    connection: &mut Connection,
    paths: &AppPaths,
    request: ImportSheetCellsRequest,
) -> AppResult<ImportSheetCellsResult> {
    let source = read_sheet_image_input(
        request.sheet_path.as_deref(),
        request.sheet_file.as_ref(),
        false,
    )?;
    let format = image_format_for_extension(&source.extension)?;
    let image = decode_import_image(&source.bytes, format)?;
    let rgba = image.to_rgba8();
    let (sheet_width, sheet_height) = (i64::from(rgba.width()), i64::from(rgba.height()));
    let analysis = analyze_rgba_grid(&rgba, &request.grid_settings, sheet_width, sheet_height)?;
    let selected = request
        .selected_cell_indexes
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    let selected_cells = analysis
        .cells
        .iter()
        .filter(|cell| selected.contains(&cell.index))
        .collect::<Vec<_>>();

    if selected_cells.is_empty() {
        return Err(AppError::new(
            "validation",
            "가져올 시트 셀이 선택되지 않았습니다.",
        ));
    }

    let preserved_sheet_path = preserve_original_sheet(
        paths,
        &source.original_filename,
        &source.extension,
        &source.bytes,
    )?;
    let mut skipped_cells = Vec::new();
    let mut cell_imports = Vec::new();
    let mut warnings = analysis.warnings;
    if let Some(warning) = alpha_warning_for_extension(&source.extension) {
        warnings.push(warning.to_string());
    }

    if !request.preserve_alpha {
        warnings.push(
            "PMTCONCON Studio는 PNG 알파를 손상하지 않기 위해 시트 셀을 PNG로 보존합니다."
                .to_string(),
        );
    }

    for cell in selected_cells {
        if cell.out_of_bounds {
            skipped_cells.push(SkippedSheetCell {
                index: cell.index,
                reason: "셀 영역이 시트 밖으로 나갑니다.".to_string(),
            });
            continue;
        }
        if cell.empty_candidate {
            skipped_cells.push(SkippedSheetCell {
                index: cell.index,
                reason: "투명/빈 셀 후보입니다.".to_string(),
            });
            continue;
        }

        let cell_image = crop_cell(&rgba, cell);
        let bytes = encode_png(&cell_image)?;
        let display_name = display_name_for_cell(
            request.default_display_name_pattern.as_deref(),
            cell.index,
            cell_imports.len(),
        );
        let safe_stem = safe_file_stem(&display_name, "sheet_cell");
        let original_filename = format!("{safe_stem}.png");
        let extracted_path = paths
            .sheet_import_cells_dir
            .join(format!("{}.png", create_id("cell")));
        if let Some(parent) = extracted_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&extracted_path, &bytes)?;

        cell_imports.push(CellImportInput {
            original_filename,
            bytes,
            display_name,
            alt_text: String::new(),
            cell_width: request.output_cell_width,
            cell_height: request.output_cell_height,
        });
    }

    if cell_imports.is_empty() {
        return Err(AppError::new(
            "validation",
            "선택한 셀 중 가져올 수 있는 셀이 없습니다.",
        ));
    }

    let imported_icons = create_icons_from_png_cells(
        connection,
        paths,
        &request.target_collection_id,
        cell_imports,
    )?;

    Ok(ImportSheetCellsResult {
        imported_count: imported_icons.len() as i64,
        imported_icons,
        skipped_cells,
        warnings,
        preserved_sheet_path: path_string(&preserved_sheet_path),
    })
}

pub(crate) fn create_icons_from_png_cells(
    connection: &mut Connection,
    paths: &AppPaths,
    collection_id: &str,
    cells: Vec<CellImportInput>,
) -> AppResult<Vec<IconDto>> {
    if cells.len() > usize::try_from(MAX_SHEET_CELLS).unwrap_or(usize::MAX) {
        return Err(AppError::new(
            "validation",
            format!("한 번에 최대 {MAX_SHEET_CELLS}개 시트 셀까지 가져올 수 있습니다."),
        ));
    }
    let transaction = connection.transaction()?;
    let collection = load_collection_for_sheet_import(&transaction, collection_id)?;
    for cell in &cells {
        let width = u32::try_from(cell.cell_width.unwrap_or(collection.default_cell_width))
            .map_err(|_| AppError::new("validation", "시트 셀 너비가 올바르지 않습니다."))?;
        let height = u32::try_from(cell.cell_height.unwrap_or(collection.default_cell_height))
            .map_err(|_| AppError::new("validation", "시트 셀 높이가 올바르지 않습니다."))?;
        validate_import_dimensions(width, height)?;
    }
    let mut next_order = next_icon_order_index(&transaction, collection_id)?;
    let mut has_cover =
        collection.cover_icon_id.is_some() || collection.cover_source_file_id.is_some();
    let mut created_icon_ids = Vec::with_capacity(cells.len());

    for cell in cells {
        let source_file = import_source_file_from_bytes(
            &transaction,
            paths,
            &ImportImageFilePayload {
                original_filename: cell.original_filename,
                bytes: cell.bytes,
            },
            SourceFileImportOptions {
                allow_gif: false,
                exact_dimensions: None,
            },
        )?;
        let icon_id = insert_sheet_icon(
            &transaction,
            &collection,
            &source_file,
            &cell.display_name,
            next_order,
            &cell.alt_text,
            cell.cell_width,
            cell.cell_height,
        )?;
        next_order = next_order.checked_add(1).ok_or_else(|| {
            AppError::new("validation", "아이콘 정렬 순서가 지원 범위를 벗어났습니다.")
        })?;

        if !has_cover {
            transaction.execute(
                "UPDATE collections
                 SET cover_icon_id = ?1,
                     cover_source_file_id = ?2,
                     updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE id = ?3
                   AND deleted_at IS NULL",
                params![icon_id, source_file.id, collection.id],
            )?;
            has_cover = true;
        }

        created_icon_ids.push(icon_id);
    }

    transaction.commit()?;

    let icons = icon_repository::list_icons(connection, collection_id)?
        .into_iter()
        .filter(|icon| created_icon_ids.contains(&icon.id))
        .collect();
    Ok(icons)
}

pub(crate) fn png_bytes_from_rgba(image: &RgbaImage) -> AppResult<Vec<u8>> {
    encode_png(image)
}

pub(crate) fn crop_cell(image: &RgbaImage, cell: &SheetCell) -> RgbaImage {
    imageops::crop_imm(
        image,
        cell.x.max(0) as u32,
        cell.y.max(0) as u32,
        cell.w.max(1) as u32,
        cell.h.max(1) as u32,
    )
    .to_image()
}

fn load_collection_for_sheet_import(
    transaction: &Transaction<'_>,
    collection_id: &str,
) -> AppResult<CollectionSheetImportRecord> {
    transaction
        .query_row(
            "SELECT
               id,
               cover_icon_id,
               cover_source_file_id,
               default_cell_width,
               default_cell_height
             FROM collections
             WHERE id = ?1
               AND deleted_at IS NULL",
            params![collection_id],
            |row| {
                Ok(CollectionSheetImportRecord {
                    id: row.get("id")?,
                    cover_icon_id: row.get("cover_icon_id")?,
                    cover_source_file_id: row.get("cover_source_file_id")?,
                    default_cell_width: row.get("default_cell_width")?,
                    default_cell_height: row.get("default_cell_height")?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("시트 셀을 가져올 모음을 찾을 수 없습니다."))
}

fn insert_sheet_icon(
    transaction: &Transaction<'_>,
    collection: &CollectionSheetImportRecord,
    source_file: &StoredSourceFile,
    display_name: &str,
    order_index: i64,
    alt_text: &str,
    cell_width_override: Option<i64>,
    cell_height_override: Option<i64>,
) -> AppResult<String> {
    let icon_id = create_id("icon");
    let cell_width = cell_width_override
        .unwrap_or(collection.default_cell_width)
        .max(1);
    let cell_height = cell_height_override
        .unwrap_or(collection.default_cell_height)
        .max(1);

    transaction.execute(
        "INSERT INTO icons (
           id,
           collection_id,
           source_file_id,
           display_name,
           icon_kind,
           readiness,
           shape,
           order_index,
           cell_width_override,
           cell_height_override,
           thumbnail_path,
           current_preview_path,
           created_at,
           updated_at
         )
         VALUES (
           ?1,
           ?2,
           ?3,
           ?4,
           'image',
           'complete',
           'single',
           ?5,
           ?6,
           ?7,
           ?8,
           ?9,
           strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
           strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         )",
        params![
            icon_id,
            collection.id,
            source_file.id,
            display_name,
            order_index,
            cell_width_override,
            cell_height_override,
            source_file.thumbnail_path,
            source_file.thumbnail_path,
        ],
    )?;

    transaction.execute(
        "INSERT INTO crop_settings (
           id,
           icon_id,
           crop_mode,
           crop_x,
           crop_y,
           crop_w,
           crop_h,
           preset_position,
           source_width_at_apply,
           source_height_at_apply,
           viewport_width_at_apply,
           viewport_height_at_apply,
           updated_at
         )
         VALUES (
           ?1,
           ?2,
           'free',
           0,
           0,
           ?3,
           ?4,
           'center',
           ?3,
           ?4,
           ?5,
           ?6,
           strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         )",
        params![
            create_id("crop"),
            icon_id,
            source_file.width,
            source_file.height,
            cell_width,
            cell_height,
        ],
    )?;

    transaction.execute(
        "INSERT INTO icon_pieces (
           id,
           icon_id,
           piece_index,
           piece_role,
           alt_text,
           created_at,
           updated_at
         )
         VALUES (
           ?1,
           ?2,
           0,
           'single',
           ?3,
           strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
           strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         )",
        params![create_id("piece"), icon_id, alt_text],
    )?;

    Ok(icon_id)
}

fn next_icon_order_index(transaction: &Transaction<'_>, collection_id: &str) -> AppResult<i64> {
    Ok(transaction.query_row(
        "SELECT COALESCE(MAX(order_index) + 1, 0)
         FROM icons
         WHERE collection_id = ?1
           AND deleted_at IS NULL",
        params![collection_id],
        |row| row.get(0),
    )?)
}

pub(crate) fn preserve_original_sheet(
    paths: &AppPaths,
    filename: &str,
    extension: &str,
    bytes: &[u8],
) -> AppResult<PathBuf> {
    let hash = sha256_hex(bytes);
    let stem = Path::new(filename)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(sanitize_name)
        .filter(|stem| !stem.is_empty())
        .unwrap_or_else(|| "sheet".to_string());
    let target = paths
        .sheet_import_originals_dir
        .join(format!("{stem}-{hash}.{extension}"));
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    if !target.exists() {
        fs::write(&target, bytes)?;
    }
    Ok(target)
}

fn encode_png(image: &RgbaImage) -> AppResult<Vec<u8>> {
    let mut cursor = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image.clone())
        .write_to(&mut cursor, ImageFormat::Png)
        .map_err(AppError::from)?;
    Ok(cursor.into_inner())
}

fn display_name_for_cell(pattern: Option<&str>, cell_index: i64, imported_index: usize) -> String {
    let pattern = pattern
        .map(str::trim)
        .filter(|pattern| !pattern.is_empty())
        .unwrap_or("sheet_{number}");
    let number = imported_index + 1;
    let rendered = pattern
        .replace("{index}", &cell_index.to_string())
        .replace("{number}", &format!("{number:03}"));
    let display_name = rendered.trim().chars().take(80).collect::<String>();
    if display_name.is_empty() {
        format!("sheet_{number:03}")
    } else {
        display_name
    }
}

fn safe_file_stem(value: &str, fallback: &str) -> String {
    let safe = value
        .chars()
        .map(|character| {
            if character.is_control()
                || matches!(
                    character,
                    '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
                )
            {
                '_'
            } else {
                character
            }
        })
        .collect::<String>()
        .trim_matches(|character: char| character.is_whitespace() || character == '.')
        .chars()
        .take(80)
        .collect::<String>();
    if safe.is_empty() || matches!(safe.as_str(), "." | "..") {
        fallback.to_string()
    } else {
        safe
    }
}

fn sanitize_name(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(64);
    for byte in digest {
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}

fn default_preserve_alpha() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::time::{SystemTime, UNIX_EPOCH};

    use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};
    use rusqlite::{params, Connection};

    use crate::db::migrations;
    use crate::db::repositories::collections::create_collection;
    use crate::models::ImportImageFilePayload;
    use crate::paths::AppPaths;

    use super::{
        create_icons_from_png_cells, display_name_for_cell, import_sheet_cells, safe_file_stem,
        CellImportInput, ImportSheetCellsRequest,
    };
    use crate::sheet::grid::SheetGridSettings;

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
        AppPaths::prepare(std::env::temp_dir().join(format!("pmtconcon-sheet-import-{suffix}")))
            .unwrap()
    }

    #[test]
    fn cell_output_override_rejects_extreme_dimensions_before_files_or_db() {
        let mut connection = connection();
        let paths = temp_paths();
        let collection =
            create_collection(&mut connection, Some("override validation".to_string())).unwrap();
        let before_files = std::fs::read_dir(&paths.originals_dir).unwrap().count();
        let mut cursor = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(ImageBuffer::from_pixel(1, 1, Rgba([255, 0, 0, 255])))
            .write_to(&mut cursor, ImageFormat::Png)
            .unwrap();
        let error = create_icons_from_png_cells(
            &mut connection,
            &paths,
            &collection.id,
            vec![CellImportInput {
                original_filename: "cell.png".to_string(),
                bytes: cursor.into_inner(),
                display_name: "cell".to_string(),
                alt_text: String::new(),
                cell_width: Some(i64::MAX),
                cell_height: Some(1),
            }],
        )
        .unwrap_err();
        assert_eq!(error.code, "validation");
        assert_eq!(
            std::fs::read_dir(&paths.originals_dir).unwrap().count(),
            before_files
        );
        let icon_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM icons WHERE collection_id = ?1",
                [&collection.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(icon_count, 0);
        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn cell_display_name_pattern_cannot_escape_output_directory() {
        let display_name = display_name_for_cell(Some("../../밖/{number}"), 0, 0);
        assert!(display_name.contains('밖'));
        let safe_stem = safe_file_stem(&display_name, "sheet_cell");
        assert!(!safe_stem.contains(['/', '\\']));
        assert!(safe_stem.chars().count() <= 80);
        let root = std::path::Path::new("sheet_cells");
        let candidate = root.join("cell_safe_id.png");
        assert_eq!(candidate.parent(), Some(root));
    }

    #[test]
    fn imported_png_sheet_cells_preserve_alpha_and_original_sheet() {
        let mut image = ImageBuffer::from_pixel(20, 10, Rgba([255, 0, 0, 255]));
        image.put_pixel(2, 2, Rgba([0, 255, 0, 64]));
        for y in 0..10 {
            for x in 10..20 {
                image.put_pixel(x, y, Rgba([0, 0, 0, 0]));
            }
        }
        let mut cursor = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image)
            .write_to(&mut cursor, ImageFormat::Png)
            .unwrap();
        let bytes = cursor.into_inner();
        let mut connection = connection();
        let paths = temp_paths();
        let collection =
            create_collection(&mut connection, Some("sheet import".to_string())).unwrap();

        let result = import_sheet_cells(
            &mut connection,
            &paths,
            ImportSheetCellsRequest {
                sheet_path: None,
                sheet_file: Some(ImportImageFilePayload {
                    original_filename: "sheet.png".to_string(),
                    bytes,
                }),
                target_collection_id: collection.id.clone(),
                grid_settings: SheetGridSettings {
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
                },
                selected_cell_indexes: vec![0, 1],
                default_display_name_pattern: Some("sheet_{number}".to_string()),
                preserve_alpha: true,
                output_cell_width: None,
                output_cell_height: None,
            },
        )
        .unwrap();

        assert_eq!(result.imported_icons.len(), 1);
        assert_eq!(result.skipped_cells.len(), 1);
        assert!(std::path::Path::new(&result.preserved_sheet_path).exists());

        let original_path: String = connection
            .query_row(
                "SELECT s.original_path_in_library
                 FROM source_files s
                 JOIN icons i ON i.source_file_id = s.id
                 WHERE i.id = ?1",
                params![result.imported_icons[0].id],
                |row| row.get(0),
            )
            .unwrap();
        let cell = image::open(original_path).unwrap().to_rgba8();
        assert_eq!(cell.get_pixel(2, 2).0[3], 64);

        std::fs::remove_dir_all(paths.root).unwrap();
    }
}
