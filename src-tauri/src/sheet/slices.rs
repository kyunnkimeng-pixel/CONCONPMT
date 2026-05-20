use std::fmt::Write as FmtWrite;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use image::imageops;
use image::{DynamicImage, ImageFormat, RgbaImage};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::db::repositories::icons as icon_repository;
use crate::error::{AppError, AppResult};
use crate::ids::create_id;
use crate::models::{IconDto, ImportImageFilePayload};
use crate::paths::AppPaths;

use super::importer::{create_icons_from_png_cells, CellImportInput};
use super::{image_format_for_extension, path_string, read_sheet_image_input};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualSlice {
    pub slice_id: String,
    pub name: String,
    pub x: i64,
    pub y: i64,
    pub w: i64,
    pub h: i64,
    pub order_index: i64,
    pub include: bool,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzeManualSlicesRequest {
    pub sheet_path: Option<String>,
    pub sheet_file: Option<ImportImageFilePayload>,
    #[serde(default)]
    pub slices: Vec<ManualSlice>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualSliceAnalysis {
    pub sheet_width: i64,
    pub sheet_height: i64,
    pub slice_count: i64,
    pub included_count: i64,
    pub out_of_bounds_slice_ids: Vec<String>,
    pub slices: Vec<ManualSlicePreview>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualSlicePreview {
    pub slice_id: String,
    pub name: String,
    pub x: i64,
    pub y: i64,
    pub w: i64,
    pub h: i64,
    pub order_index: i64,
    pub include: bool,
    pub out_of_bounds: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportManualSlicesRequest {
    pub sheet_path: Option<String>,
    pub sheet_file: Option<ImportImageFilePayload>,
    pub target_collection_id: String,
    #[serde(default)]
    pub slices: Vec<ManualSlice>,
    pub default_display_name_pattern: Option<String>,
    #[serde(default = "default_preserve_alpha")]
    pub preserve_alpha: bool,
    pub output_cell_width: Option<i64>,
    pub output_cell_height: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportManualSlicesResult {
    pub imported_icons: Vec<IconDto>,
    pub skipped_slices: Vec<SkippedManualSlice>,
    pub warnings: Vec<String>,
    pub preserved_sheet_path: String,
    pub imported_count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedManualSlice {
    pub slice_id: String,
    pub order_index: i64,
    pub reason: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualSliceSaveRequest {
    pub sheet_id: String,
    pub slices: Vec<ManualSlice>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualSliceSaveResult {
    pub saved_count: i64,
    pub metadata_path: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualSliceLoadResult {
    pub sheet_id: String,
    pub slices: Vec<ManualSlice>,
    pub metadata_path: String,
}

pub fn analyze_manual_slices(
    request: AnalyzeManualSlicesRequest,
) -> AppResult<ManualSliceAnalysis> {
    let source = read_sheet_image_input(
        request.sheet_path.as_deref(),
        request.sheet_file.as_ref(),
        false,
    )?;
    let format = image_format_for_extension(&source.extension)?;
    let image = image::load_from_memory_with_format(&source.bytes, format)?;
    let rgba = image.to_rgba8();
    Ok(analyze_manual_slices_for_dimensions(
        i64::from(rgba.width()),
        i64::from(rgba.height()),
        &request.slices,
    ))
}

pub fn import_manual_slices(
    connection: &mut Connection,
    paths: &AppPaths,
    request: ImportManualSlicesRequest,
) -> AppResult<ImportManualSlicesResult> {
    let source = read_sheet_image_input(
        request.sheet_path.as_deref(),
        request.sheet_file.as_ref(),
        false,
    )?;
    let format = image_format_for_extension(&source.extension)?;
    let image = image::load_from_memory_with_format(&source.bytes, format)?;
    let rgba = image.to_rgba8();
    let analysis = analyze_manual_slices_for_dimensions(
        i64::from(rgba.width()),
        i64::from(rgba.height()),
        &request.slices,
    );

    let selected = sorted_included_slices(&request.slices);
    if selected.is_empty() {
        return Err(AppError::new(
            "validation",
            "가져올 직접 Slice가 선택되지 않았습니다.",
        ));
    }

    let preserved_sheet_path = preserve_original_sheet(
        paths,
        &source.original_filename,
        &source.extension,
        &source.bytes,
    )?;
    let mut warnings = analysis.warnings;
    let mut skipped_slices = Vec::new();
    let mut cell_imports = Vec::new();

    if !request.preserve_alpha {
        warnings.push(
            "PMTCONCON Studio는 직접 Slice 결과를 PNG로 저장해 투명도를 보존합니다.".to_string(),
        );
    }

    for slice in selected {
        if slice_out_of_bounds(slice, i64::from(rgba.width()), i64::from(rgba.height())) {
            skipped_slices.push(SkippedManualSlice {
                slice_id: slice.slice_id.clone(),
                order_index: slice.order_index,
                reason: "Slice 영역이 시트 이미지 범위를 벗어났습니다.".to_string(),
            });
            continue;
        }

        let slice_image = crop_manual_slice(&rgba, slice);
        let bytes = encode_png(&slice_image)?;
        let display_name = display_name_for_slice(
            request.default_display_name_pattern.as_deref(),
            slice,
            cell_imports.len(),
        );
        let original_filename = format!("{display_name}.png");
        let extracted_path = paths
            .sheet_import_cells_dir
            .join(format!("{}-{display_name}.png", create_id("slice")));
        if let Some(parent) = extracted_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&extracted_path, &bytes)?;

        cell_imports.push(CellImportInput {
            original_filename,
            bytes,
            display_name,
            alt_text: String::new(),
            cell_width: request.output_cell_width.or(Some(slice.w.max(1))),
            cell_height: request.output_cell_height.or(Some(slice.h.max(1))),
        });
    }

    if cell_imports.is_empty() {
        return Err(AppError::new(
            "validation",
            "선택한 Slice 중 가져올 수 있는 Slice가 없습니다.",
        ));
    }

    let created_icons = create_icons_from_png_cells(
        connection,
        paths,
        &request.target_collection_id,
        cell_imports,
    )?;
    let created_ids = created_icons
        .iter()
        .map(|icon| icon.id.clone())
        .collect::<Vec<_>>();
    let imported_icons = icon_repository::list_icons(connection, &request.target_collection_id)?
        .into_iter()
        .filter(|icon| created_ids.contains(&icon.id))
        .collect::<Vec<_>>();

    Ok(ImportManualSlicesResult {
        imported_count: imported_icons.len() as i64,
        imported_icons,
        skipped_slices,
        warnings,
        preserved_sheet_path: path_string(&preserved_sheet_path),
    })
}

pub fn save_manual_slices(
    paths: &AppPaths,
    request: ManualSliceSaveRequest,
) -> AppResult<ManualSliceSaveResult> {
    validate_manual_slices(&request.slices)?;
    let metadata_path = manual_slice_metadata_path(paths, &request.sheet_id);
    if let Some(parent) = metadata_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_vec_pretty(&request)
        .map_err(|error| AppError::new("json", error.to_string()))?;
    fs::write(&metadata_path, json)?;

    Ok(ManualSliceSaveResult {
        saved_count: request.slices.len() as i64,
        metadata_path: path_string(&metadata_path),
        warnings: manual_slice_warnings(&request.slices),
    })
}

pub fn load_manual_slices(paths: &AppPaths, sheet_id: String) -> AppResult<ManualSliceLoadResult> {
    let metadata_path = manual_slice_metadata_path(paths, &sheet_id);
    let bytes = fs::read(&metadata_path).map_err(|error| {
        AppError::new(
            "not_found",
            format!("직접 Slice metadata를 찾을 수 없습니다. {}", error),
        )
    })?;
    let request: ManualSliceSaveRequest =
        serde_json::from_slice(&bytes).map_err(|error| AppError::new("json", error.to_string()))?;
    Ok(ManualSliceLoadResult {
        sheet_id: request.sheet_id,
        slices: request.slices,
        metadata_path: path_string(&metadata_path),
    })
}

pub fn validate_manual_slices(slices: &[ManualSlice]) -> AppResult<()> {
    for slice in slices {
        if slice.slice_id.trim().is_empty() {
            return Err(AppError::new(
                "validation",
                "직접 Slice ID가 비어 있습니다.",
            ));
        }
        if slice.w <= 0 || slice.h <= 0 {
            return Err(AppError::new(
                "validation",
                "직접 Slice 영역의 너비와 높이는 1px 이상이어야 합니다.",
            ));
        }
    }
    Ok(())
}

fn analyze_manual_slices_for_dimensions(
    sheet_width: i64,
    sheet_height: i64,
    slices: &[ManualSlice],
) -> ManualSliceAnalysis {
    let mut previews = Vec::with_capacity(slices.len());
    let mut out_of_bounds_slice_ids = Vec::new();

    for slice in sorted_slices(slices) {
        let mut warnings = Vec::new();
        if slice.w <= 0 || slice.h <= 0 {
            warnings.push("Slice 너비와 높이는 1px 이상이어야 합니다.".to_string());
        }
        let out_of_bounds = slice_out_of_bounds(slice, sheet_width, sheet_height);
        if out_of_bounds {
            warnings.push("Slice 영역이 시트 범위를 벗어났습니다.".to_string());
            out_of_bounds_slice_ids.push(slice.slice_id.clone());
        }
        if slice.name.trim().is_empty() {
            warnings.push("이름이 비어 있어 가져오기 때 기본 이름 패턴을 사용합니다.".to_string());
        }
        previews.push(ManualSlicePreview {
            slice_id: slice.slice_id.clone(),
            name: slice.name.clone(),
            x: slice.x,
            y: slice.y,
            w: slice.w,
            h: slice.h,
            order_index: slice.order_index,
            include: slice.include,
            out_of_bounds,
            warnings,
        });
    }

    ManualSliceAnalysis {
        sheet_width,
        sheet_height,
        slice_count: slices.len() as i64,
        included_count: slices.iter().filter(|slice| slice.include).count() as i64,
        out_of_bounds_slice_ids,
        slices: previews,
        warnings: manual_slice_warnings(slices),
    }
}

fn manual_slice_warnings(slices: &[ManualSlice]) -> Vec<String> {
    let mut warnings = Vec::new();
    if slices.is_empty() {
        warnings.push("아직 직접 Slice가 없습니다. + Slice로 영역을 추가하세요.".to_string());
    }
    if slices.iter().all(|slice| !slice.include) {
        warnings.push("포함된 직접 Slice가 없습니다.".to_string());
    }
    warnings
}

fn sorted_slices(slices: &[ManualSlice]) -> Vec<&ManualSlice> {
    let mut sorted = slices.iter().collect::<Vec<_>>();
    sorted.sort_by_key(|slice| (slice.order_index, slice.y, slice.x, slice.slice_id.clone()));
    sorted
}

fn sorted_included_slices(slices: &[ManualSlice]) -> Vec<&ManualSlice> {
    sorted_slices(slices)
        .into_iter()
        .filter(|slice| slice.include)
        .collect()
}

fn slice_out_of_bounds(slice: &ManualSlice, sheet_width: i64, sheet_height: i64) -> bool {
    slice.x < 0
        || slice.y < 0
        || slice.w <= 0
        || slice.h <= 0
        || slice.x + slice.w > sheet_width
        || slice.y + slice.h > sheet_height
}

fn crop_manual_slice(image: &RgbaImage, slice: &ManualSlice) -> RgbaImage {
    imageops::crop_imm(
        image,
        slice.x.max(0) as u32,
        slice.y.max(0) as u32,
        slice.w.max(1) as u32,
        slice.h.max(1) as u32,
    )
    .to_image()
}

fn preserve_original_sheet(
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
        .unwrap_or_else(|| "manual_slice_sheet".to_string());
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

fn display_name_for_slice(
    pattern: Option<&str>,
    slice: &ManualSlice,
    imported_index: usize,
) -> String {
    let slice_name = sanitize_name(slice.name.trim());
    if !slice_name.is_empty() {
        return slice_name;
    }
    let pattern = pattern
        .map(str::trim)
        .filter(|pattern| !pattern.is_empty())
        .unwrap_or("slice_{number}");
    let number = imported_index + 1;
    pattern
        .replace("{index}", &slice.order_index.to_string())
        .replace("{number}", &format!("{number:03}"))
        .replace("{slice_id}", &sanitize_name(&slice.slice_id))
}

fn manual_slice_metadata_path(paths: &AppPaths, sheet_id: &str) -> PathBuf {
    paths
        .sheet_import_manifests_dir
        .join("manual_slices")
        .join(format!(
            "{}.json",
            sanitize_name(sheet_id).chars().take(96).collect::<String>()
        ))
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
        .collect::<String>()
        .trim_matches('_')
        .to_string()
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
    use rusqlite::Connection;

    use crate::db::migrations;
    use crate::db::repositories::collections::create_collection;
    use crate::models::ImportImageFilePayload;
    use crate::paths::AppPaths;

    use super::{
        analyze_manual_slices, import_manual_slices, load_manual_slices, save_manual_slices,
        AnalyzeManualSlicesRequest, ImportManualSlicesRequest, ManualSlice, ManualSliceSaveRequest,
    };

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
        AppPaths::prepare(std::env::temp_dir().join(format!("pmtconcon-manual-slice-{suffix}")))
            .unwrap()
    }

    fn png_payload() -> ImportImageFilePayload {
        let mut image = ImageBuffer::from_pixel(32, 16, Rgba([0, 0, 0, 0]));
        image.put_pixel(5, 5, Rgba([255, 0, 0, 72]));
        for y in 0..8 {
            for x in 16..24 {
                image.put_pixel(x, y, Rgba([0, 255, 0, 255]));
            }
        }
        let mut cursor = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image)
            .write_to(&mut cursor, ImageFormat::Png)
            .unwrap();
        ImportImageFilePayload {
            original_filename: "manual_sheet.png".to_string(),
            bytes: cursor.into_inner(),
        }
    }

    fn slice(id: &str, name: &str, x: i64, y: i64, w: i64, h: i64, order: i64) -> ManualSlice {
        ManualSlice {
            slice_id: id.to_string(),
            name: name.to_string(),
            x,
            y,
            w,
            h,
            order_index: order,
            include: true,
            notes: None,
        }
    }

    #[test]
    fn manual_slice_analysis_reports_out_of_bounds() {
        let result = analyze_manual_slices(AnalyzeManualSlicesRequest {
            sheet_path: None,
            sheet_file: Some(png_payload()),
            slices: vec![
                slice("a", "valid", 0, 0, 10, 10, 0),
                slice("b", "bad", 30, 0, 8, 8, 1),
            ],
        })
        .unwrap();

        assert_eq!(result.sheet_width, 32);
        assert_eq!(result.sheet_height, 16);
        assert_eq!(result.out_of_bounds_slice_ids, vec!["b".to_string()]);
        assert_eq!(result.slices.len(), 2);
    }

    #[test]
    fn manual_slice_import_preserves_alpha_original_and_order() {
        let mut connection = connection();
        let paths = temp_paths();
        let collection =
            create_collection(&mut connection, Some("manual slices".to_string())).unwrap();

        let result = import_manual_slices(
            &mut connection,
            &paths,
            ImportManualSlicesRequest {
                sheet_path: None,
                sheet_file: Some(png_payload()),
                target_collection_id: collection.id.clone(),
                slices: vec![
                    slice("second", "slice_two", 16, 0, 8, 8, 1),
                    slice("first", "slice_one", 0, 0, 10, 10, 0),
                    slice("bad", "bad", 31, 0, 8, 8, 2),
                ],
                default_display_name_pattern: Some("slice_{number}".to_string()),
                preserve_alpha: true,
                output_cell_width: None,
                output_cell_height: None,
            },
        )
        .unwrap();

        assert_eq!(result.imported_count, 2);
        assert_eq!(result.skipped_slices.len(), 1);
        assert!(std::path::Path::new(&result.preserved_sheet_path).exists());
        assert_eq!(result.imported_icons[0].display_name, "slice_one");
        assert_eq!(result.imported_icons[1].display_name, "slice_two");

        let first_source = &result.imported_icons[0].thumbnail_url;
        assert!(first_source.is_some());

        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn manual_slice_metadata_roundtrip() {
        let paths = temp_paths();
        let slices = vec![slice("one", "first", 1, 2, 3, 4, 0)];

        let saved = save_manual_slices(
            &paths,
            ManualSliceSaveRequest {
                sheet_id: "sheet-a.png".to_string(),
                slices: slices.clone(),
            },
        )
        .unwrap();
        let loaded = load_manual_slices(&paths, "sheet-a.png".to_string()).unwrap();

        assert_eq!(saved.saved_count, 1);
        assert_eq!(loaded.slices[0].slice_id, slices[0].slice_id);

        std::fs::remove_dir_all(paths.root).unwrap();
    }
}
