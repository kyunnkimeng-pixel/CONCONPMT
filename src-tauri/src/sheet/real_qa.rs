use std::fs;
use std::io::Cursor;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use image::{DynamicImage, ImageBuffer, ImageFormat, Rgb, RgbImage, Rgba, RgbaImage};
use rusqlite::{params, Connection};

use crate::db::migrations;
use crate::db::repositories::collections::create_collection;
use crate::db::repositories::editor::get_icon_editor_state;
use crate::db::repositories::export_profiles::list_export_profiles;
use crate::db::repositories::icons::{list_icons, update_icon_piece_alt};
use crate::db::repositories::imports::import_image_files;
use crate::export::export_collection;
use crate::models::{ExportRequestPayload, ImportImageFilePayload};
use crate::paths::AppPaths;
use crate::sheet::exporter::{export_edit_sheet, ExportEditSheetRequest, GuideLabelOptions};
use crate::sheet::grid::{analyze_sheet_grid, SheetGridAnalyzeRequest, SheetGridSettings};
use crate::sheet::importer::{import_sheet_cells, ImportSheetCellsRequest};
use crate::sheet::manifest::StaticSheetManifest;
use crate::sheet::reimport::{reimport_edit_sheet, ReimportEditSheetRequest};

fn connection() -> Connection {
    let mut connection = Connection::open_in_memory().unwrap();
    migrations::run(&mut connection).unwrap();
    connection
}

fn temp_paths(prefix: &str) -> AppPaths {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    AppPaths::prepare(std::env::temp_dir().join(format!("{prefix}-{suffix}"))).unwrap()
}

fn png_bytes(image: RgbaImage) -> Vec<u8> {
    let mut cursor = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image)
        .write_to(&mut cursor, ImageFormat::Png)
        .unwrap();
    cursor.into_inner()
}

fn jpg_bytes(image: RgbImage) -> Vec<u8> {
    let mut cursor = Cursor::new(Vec::new());
    DynamicImage::ImageRgb8(image)
        .write_to(&mut cursor, ImageFormat::Jpeg)
        .unwrap();
    cursor.into_inner()
}

fn payload(name: &str, bytes: Vec<u8>) -> ImportImageFilePayload {
    ImportImageFilePayload {
        original_filename: name.to_string(),
        bytes,
    }
}

fn solid_cell_sheet(columns: u32, rows: u32, cell: u32) -> RgbaImage {
    let mut image = RgbaImage::from_pixel(columns * cell, rows * cell, Rgba([0, 0, 0, 0]));
    for row in 0..rows {
        for col in 0..columns {
            let color = Rgba([
                40 + ((col * 45) % 180) as u8,
                50 + ((row * 70) % 160) as u8,
                80 + (((row * columns + col) * 31) % 150) as u8,
                180 + (((row + col) * 9) % 60) as u8,
            ]);
            fill_rect(&mut image, col * cell, row * cell, cell, cell, color);
        }
    }
    image
}

fn sheet_with_gap_border() -> RgbaImage {
    let mut image = RgbaImage::from_pixel(648, 440, Rgba([0, 0, 0, 0]));
    for row in 0..2 {
        for col in 0..3 {
            let color = Rgba([30 + col as u8 * 50, 80 + row as u8 * 60, 180, 210]);
            fill_rect(&mut image, 16 + col * 208, 16 + row * 208, 200, 200, color);
        }
    }
    image
}

fn sheet_with_empty_center_cell() -> RgbaImage {
    let mut image = RgbaImage::from_pixel(600, 200, Rgba([0, 0, 0, 0]));
    fill_rect(&mut image, 0, 0, 200, 200, Rgba([255, 40, 40, 217]));
    fill_rect(&mut image, 400, 0, 200, 200, Rgba([40, 180, 90, 143]));
    image
}

fn jpg_sheet() -> RgbImage {
    let mut image = ImageBuffer::from_pixel(600, 200, Rgb([255, 255, 255]));
    for row in 0..200 {
        for col in 0..600 {
            let channel = if col < 200 {
                Rgb([230, 30, 30])
            } else if col < 400 {
                Rgb([30, 150, 220])
            } else {
                Rgb([40, 180, 80])
            };
            image.put_pixel(col, row, channel);
        }
    }
    image
}

fn fill_rect(image: &mut RgbaImage, x: u32, y: u32, width: u32, height: u32, color: Rgba<u8>) {
    for yy in y..(y + height) {
        for xx in x..(x + width) {
            image.put_pixel(xx, yy, color);
        }
    }
}

fn grid_settings(mode: &str, rows: Option<i64>, columns: Option<i64>) -> SheetGridSettings {
    SheetGridSettings {
        mode: mode.to_string(),
        rows,
        columns,
        cell_width: None,
        cell_height: None,
        border_left: 0,
        border_top: 0,
        border_right: 0,
        border_bottom: 0,
        gap_x: 0,
        gap_y: 0,
        read_order: "row_major".to_string(),
        empty_cell_threshold: Some(0.98),
    }
}

fn source_path_for_icon(connection: &Connection, icon_id: &str) -> String {
    connection
        .query_row(
            "SELECT s.original_path_in_library
             FROM source_files s
             JOIN icons i ON i.source_file_id = s.id
             WHERE i.id = ?1",
            params![icon_id],
            |row| row.get(0),
        )
        .unwrap()
}

fn read_manifest(path: &str) -> StaticSheetManifest {
    let bytes = fs::read(path).unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

fn custom_export_profile_id(connection: &Connection, collection_id: &str) -> String {
    list_export_profiles(connection, collection_id)
        .unwrap()
        .into_iter()
        .find(|profile| profile.profile_type == "custom")
        .unwrap()
        .id
}

#[test]
fn qa_static_sheet_import_grid_cell_size_empty_alpha_and_jpg_warning() {
    let mut connection = connection();
    let paths = temp_paths("pmtconcon-sheet-real-qa-import");
    let collection = create_collection(&mut connection, Some("sheet real qa".to_string())).unwrap();

    let rows_columns = analyze_sheet_grid(SheetGridAnalyzeRequest {
        sheet_path: None,
        sheet_file: Some(payload(
            "transparent_200_grid.png",
            png_bytes(solid_cell_sheet(3, 2, 200)),
        )),
        mode: "rows_columns".to_string(),
        rows: Some(2),
        columns: Some(3),
        cell_width: None,
        cell_height: None,
        border_left: 0,
        border_top: 0,
        border_right: 0,
        border_bottom: 0,
        gap_x: 0,
        gap_y: 0,
        read_order: "row_major".to_string(),
        empty_cell_threshold: Some(0.98),
    })
    .unwrap();
    assert_eq!(rows_columns.computed_rows, 2);
    assert_eq!(rows_columns.computed_columns, 3);
    assert_eq!(
        (rows_columns.cells[4].x, rows_columns.cells[4].y),
        (200, 200)
    );

    let cell_size = analyze_sheet_grid(SheetGridAnalyzeRequest {
        sheet_path: None,
        sheet_file: Some(payload(
            "transparent_128_grid.png",
            png_bytes(solid_cell_sheet(4, 2, 128)),
        )),
        mode: "cell_size".to_string(),
        rows: None,
        columns: None,
        cell_width: Some(128),
        cell_height: Some(128),
        border_left: 0,
        border_top: 0,
        border_right: 0,
        border_bottom: 0,
        gap_x: 0,
        gap_y: 0,
        read_order: "row_major".to_string(),
        empty_cell_threshold: Some(0.98),
    })
    .unwrap();
    assert_eq!(
        (cell_size.computed_columns, cell_size.computed_rows),
        (4, 2)
    );
    assert_eq!((cell_size.cells[7].x, cell_size.cells[7].y), (384, 128));

    let gapped = analyze_sheet_grid(SheetGridAnalyzeRequest {
        sheet_path: None,
        sheet_file: Some(payload(
            "transparent_200_gaps_border.png",
            png_bytes(sheet_with_gap_border()),
        )),
        mode: "rows_columns".to_string(),
        rows: Some(2),
        columns: Some(3),
        cell_width: None,
        cell_height: None,
        border_left: 16,
        border_top: 16,
        border_right: 16,
        border_bottom: 16,
        gap_x: 8,
        gap_y: 8,
        read_order: "row_major".to_string(),
        empty_cell_threshold: Some(0.98),
    })
    .unwrap();
    assert_eq!((gapped.cells[0].x, gapped.cells[0].y), (16, 16));
    assert_eq!((gapped.cells[5].x, gapped.cells[5].y), (432, 224));

    let empty_sheet = png_bytes(sheet_with_empty_center_cell());
    let empty_analysis = analyze_sheet_grid(SheetGridAnalyzeRequest {
        sheet_path: None,
        sheet_file: Some(payload("transparent_empty_cell.png", empty_sheet.clone())),
        mode: "rows_columns".to_string(),
        rows: Some(1),
        columns: Some(3),
        cell_width: None,
        cell_height: None,
        border_left: 0,
        border_top: 0,
        border_right: 0,
        border_bottom: 0,
        gap_x: 0,
        gap_y: 0,
        read_order: "row_major".to_string(),
        empty_cell_threshold: Some(0.98),
    })
    .unwrap();
    assert_eq!(empty_analysis.empty_cell_candidates, vec![1]);

    let jpg_analysis = analyze_sheet_grid(SheetGridAnalyzeRequest {
        sheet_path: None,
        sheet_file: Some(payload("jpg_no_alpha.jpg", jpg_bytes(jpg_sheet()))),
        mode: "rows_columns".to_string(),
        rows: Some(1),
        columns: Some(3),
        cell_width: None,
        cell_height: None,
        border_left: 0,
        border_top: 0,
        border_right: 0,
        border_bottom: 0,
        gap_x: 0,
        gap_y: 0,
        read_order: "row_major".to_string(),
        empty_cell_threshold: Some(0.98),
    })
    .unwrap();
    assert!(
        jpg_analysis
            .warnings
            .iter()
            .any(|warning| warning.contains("JPG") && warning.to_lowercase().contains("alpha")),
        "JPG sheet analysis should warn that alpha transparency is unavailable: {:?}",
        jpg_analysis.warnings
    );

    let result = import_sheet_cells(
        &mut connection,
        &paths,
        ImportSheetCellsRequest {
            sheet_path: None,
            sheet_file: Some(payload("transparent_empty_cell.png", empty_sheet)),
            target_collection_id: collection.id.clone(),
            grid_settings: grid_settings("rows_columns", Some(1), Some(3)),
            selected_cell_indexes: vec![0, 1, 2],
            default_display_name_pattern: Some("qa_sheet_{number}".to_string()),
            preserve_alpha: true,
            output_cell_width: Some(200),
            output_cell_height: Some(200),
        },
    )
    .unwrap();

    assert_eq!(result.imported_count, 2);
    assert_eq!(result.skipped_cells.len(), 1);
    assert_eq!(result.skipped_cells[0].index, 1);
    assert!(Path::new(&result.preserved_sheet_path).exists());
    assert!(Path::new(&result.preserved_sheet_path).starts_with(&paths.sheet_import_originals_dir));

    let icons = list_icons(&connection, &collection.id).unwrap();
    assert_eq!(icons.len(), 2);
    assert_eq!(icons[0].display_name, "qa_sheet_001");
    assert_eq!(icons[1].display_name, "qa_sheet_002");
    assert_eq!((icons[0].order_index, icons[1].order_index), (0, 1));

    let first_path = source_path_for_icon(&connection, &icons[0].id);
    let imported = image::open(first_path).unwrap().to_rgba8();
    assert_eq!(imported.get_pixel(25, 25).0[3], 217);

    fs::remove_dir_all(paths.root).unwrap();
}

#[test]
fn qa_static_edit_sheet_export_clean_guide_manifest_and_page_split() {
    let mut connection = connection();
    let paths = temp_paths("pmtconcon-sheet-real-qa-export");
    let collection =
        create_collection(&mut connection, Some("sheet export qa".to_string())).unwrap();

    let import = import_sheet_cells(
        &mut connection,
        &paths,
        ImportSheetCellsRequest {
            sheet_path: None,
            sheet_file: Some(payload(
                "transparent_200_grid.png",
                png_bytes(solid_cell_sheet(3, 2, 200)),
            )),
            target_collection_id: collection.id.clone(),
            grid_settings: grid_settings("rows_columns", Some(2), Some(3)),
            selected_cell_indexes: vec![0, 1, 2, 3, 4, 5],
            default_display_name_pattern: Some("export_{number}".to_string()),
            preserve_alpha: true,
            output_cell_width: Some(200),
            output_cell_height: Some(200),
        },
    )
    .unwrap();
    assert_eq!(import.imported_count, 6);

    let result = export_edit_sheet(
        &connection,
        &paths,
        ExportEditSheetRequest {
            collection_id: collection.id.clone(),
            selected_icon_ids: Vec::new(),
            source: "current_collection".to_string(),
            cell_width: 200,
            cell_height: 200,
            columns: 2,
            gap_x: 8,
            gap_y: 8,
            border_x: 16,
            border_y: 16,
            background: "transparent".to_string(),
            include_clean_sheet: true,
            include_guide_sheet: true,
            include_manifest: true,
            label_options: Some(GuideLabelOptions {
                cell_number: true,
                icon_name: false,
                alt_value: false,
                export_number: true,
            }),
            max_sheet_width: 2048,
            max_sheet_height: 240,
            output_directory: None,
            open_output_folder: false,
        },
    )
    .unwrap();

    assert_eq!(result.item_count, 6);
    assert_eq!(result.page_count, 3);
    assert_eq!(result.clean_sheet_paths.len(), 3);
    assert_eq!(result.guide_sheet_paths.len(), 3);

    let clean = image::open(&result.clean_sheet_paths[0])
        .unwrap()
        .to_rgba8();
    let guide = image::open(&result.guide_sheet_paths[0])
        .unwrap()
        .to_rgba8();
    assert_eq!((clean.width(), clean.height()), (440, 232));
    assert_eq!(clean.get_pixel(0, 0).0[3], 0);
    assert_eq!(clean.get_pixel(216, 16).0[3], 0);
    assert_eq!(clean.get_pixel(20, 20).0[3], 180);
    assert_ne!(guide.get_pixel(0, 0), clean.get_pixel(0, 0));
    assert_ne!(guide.get_pixel(16, 16), clean.get_pixel(16, 16));

    let manifest = read_manifest(result.manifest_path.as_ref().unwrap());
    assert_eq!(manifest.schema, "pmtcon-sheet-v2");
    assert_eq!(manifest.pages.len(), 3);
    assert_eq!(manifest.items.len(), 6);
    assert_eq!(manifest.items[0].page_index, 0);
    assert_eq!((manifest.items[0].x, manifest.items[0].y), (16, 16));
    assert_eq!((manifest.items[1].x, manifest.items[1].y), (224, 16));
    assert_eq!(manifest.items[2].page_index, 1);
    assert_eq!((manifest.items[2].x, manifest.items[2].y), (16, 16));

    fs::remove_dir_all(paths.root).unwrap();
}

#[test]
fn qa_static_manifest_reimport_maps_cells_preserves_originals_and_reports_bad_inputs() {
    let mut connection = connection();
    let paths = temp_paths("pmtconcon-sheet-real-qa-reimport");
    let collection =
        create_collection(&mut connection, Some("sheet reimport qa".to_string())).unwrap();

    let imported = import_sheet_cells(
        &mut connection,
        &paths,
        ImportSheetCellsRequest {
            sheet_path: None,
            sheet_file: Some(payload(
                "transparent_200_grid.png",
                png_bytes(solid_cell_sheet(2, 1, 200)),
            )),
            target_collection_id: collection.id.clone(),
            grid_settings: grid_settings("rows_columns", Some(1), Some(2)),
            selected_cell_indexes: vec![0, 1],
            default_display_name_pattern: Some("reimport_{number}".to_string()),
            preserve_alpha: true,
            output_cell_width: Some(200),
            output_cell_height: Some(200),
        },
    )
    .unwrap();
    let original_icon_id = imported.imported_icons[0].id.clone();
    let original_path = source_path_for_icon(&connection, &original_icon_id);
    let original_bytes = fs::read(&original_path).unwrap();

    let exported = export_edit_sheet(
        &connection,
        &paths,
        ExportEditSheetRequest {
            collection_id: collection.id.clone(),
            selected_icon_ids: Vec::new(),
            source: "current_collection".to_string(),
            cell_width: 200,
            cell_height: 200,
            columns: 2,
            gap_x: 8,
            gap_y: 8,
            border_x: 16,
            border_y: 16,
            background: "transparent".to_string(),
            include_clean_sheet: true,
            include_guide_sheet: true,
            include_manifest: true,
            label_options: Some(GuideLabelOptions {
                cell_number: true,
                icon_name: false,
                alt_value: false,
                export_number: false,
            }),
            max_sheet_width: 2048,
            max_sheet_height: 2048,
            output_directory: None,
            open_output_folder: false,
        },
    )
    .unwrap();

    let manifest_path = exported.manifest_path.as_ref().unwrap().clone();
    let mut edited = image::open(&exported.clean_sheet_paths[0])
        .unwrap()
        .to_rgba8();
    fill_rect(&mut edited, 76, 76, 80, 80, Rgba([255, 0, 180, 111]));
    let edited_path = paths.root.join("edited_sheet_001.png");
    DynamicImage::ImageRgba8(edited)
        .save_with_format(&edited_path, ImageFormat::Png)
        .unwrap();

    let reimport = reimport_edit_sheet(
        &mut connection,
        &paths,
        ReimportEditSheetRequest {
            manifest_path: manifest_path.clone(),
            manifest_file: None,
            edited_sheet_paths: vec![edited_path.to_string_lossy().to_string()],
            edited_sheet_files: Vec::new(),
            target_collection_id: collection.id.clone(),
            reimport_mode: "create_new_icons".to_string(),
        },
    )
    .unwrap();
    assert_eq!(reimport.updated_items.len(), 2);
    assert!(reimport.errors.is_empty());
    assert_eq!(fs::read(&original_path).unwrap(), original_bytes);

    let new_icon_id = reimport.updated_items[0].new_icon_id.as_ref().unwrap();
    let new_path = source_path_for_icon(&connection, new_icon_id);
    let reimported = image::open(new_path).unwrap().to_rgba8();
    assert_eq!(reimported.get_pixel(100, 100).0, [255, 0, 180, 111]);

    let wrong_size_path = paths.root.join("wrong_size_sheet_001.png");
    DynamicImage::ImageRgba8(RgbaImage::from_pixel(32, 32, Rgba([0, 0, 0, 0])))
        .save_with_format(&wrong_size_path, ImageFormat::Png)
        .unwrap();
    let wrong_size = reimport_edit_sheet(
        &mut connection,
        &paths,
        ReimportEditSheetRequest {
            manifest_path: manifest_path.clone(),
            manifest_file: None,
            edited_sheet_paths: vec![wrong_size_path.to_string_lossy().to_string()],
            edited_sheet_files: Vec::new(),
            target_collection_id: collection.id.clone(),
            reimport_mode: "create_new_icons".to_string(),
        },
    )
    .unwrap();
    assert!(!wrong_size.warnings.is_empty());
    assert_eq!(wrong_size.updated_items.len(), 0);
    assert_eq!(wrong_size.skipped_items.len(), 2);

    let missing_dir = paths.root.join("missing-page-case");
    fs::create_dir_all(&missing_dir).unwrap();
    let missing_manifest_path = missing_dir.join("sheet_manifest.json");
    fs::copy(&manifest_path, &missing_manifest_path).unwrap();
    let missing = reimport_edit_sheet(
        &mut connection,
        &paths,
        ReimportEditSheetRequest {
            manifest_path: missing_manifest_path.to_string_lossy().to_string(),
            manifest_file: None,
            edited_sheet_paths: Vec::new(),
            edited_sheet_files: Vec::new(),
            target_collection_id: collection.id.clone(),
            reimport_mode: "create_new_icons".to_string(),
        },
    )
    .unwrap();
    assert!(missing.updated_items.is_empty());
    assert!(!missing.errors.is_empty());

    fs::remove_dir_all(paths.root).unwrap();
}

#[test]
fn qa_static_sheet_does_not_break_normal_import_alt_edit_editor_or_export() {
    let mut connection = connection();
    let paths = temp_paths("pmtconcon-sheet-real-qa-regression");
    let collection =
        create_collection(&mut connection, Some("normal regression qa".to_string())).unwrap();

    let imported = import_image_files(
        &mut connection,
        &paths,
        &collection.id,
        vec![
            payload("normal_one.png", png_bytes(solid_cell_sheet(1, 1, 64))),
            payload("normal_two.png", png_bytes(solid_cell_sheet(1, 1, 64))),
        ],
    )
    .unwrap();
    assert_eq!(imported.imported_icons.len(), 2);
    assert!(imported.rejected_files.is_empty());

    let first_icon = &imported.imported_icons[0];
    let first_piece = &first_icon.pieces[0];
    let updated = update_icon_piece_alt(
        &connection,
        &collection.id,
        &first_piece.id,
        "qa".to_string(),
    )
    .unwrap();
    assert_eq!(updated.pieces[0].alt_text, "qa");

    let editor = get_icon_editor_state(&connection, &collection.id, &first_icon.id).unwrap();
    assert_eq!(editor.icon.id, first_icon.id);
    assert_eq!(editor.source.width, 64);

    let profile_id = custom_export_profile_id(&connection, &collection.id);
    let export = export_collection(
        &mut connection,
        &paths,
        &collection.id,
        &ExportRequestPayload {
            profile_id,
            target_format: "png".to_string(),
            target_cell_width: 64,
            target_cell_height: 64,
            max_bytes: 10_000_000,
            filename_mode: "sequence".to_string(),
            include_alt_txt: true,
            strict_warnings: false,
            output_directory: Some(
                paths
                    .root
                    .join("normal-export")
                    .to_string_lossy()
                    .to_string(),
            ),
            open_folder_after_export: false,
            open_alt_txt_after_export: false,
            excluded_piece_ids: Vec::new(),
            resize_filter: "lanczos3".to_string(),
        },
    )
    .unwrap();
    assert!(export.validation.can_export);
    let export_dir = Path::new(export.export_directory.as_ref().unwrap());
    assert!(export_dir.join("files").join("001.png").exists());
    assert!(export_dir.join("files").join("002.png").exists());

    fs::remove_dir_all(paths.root).unwrap();
}
