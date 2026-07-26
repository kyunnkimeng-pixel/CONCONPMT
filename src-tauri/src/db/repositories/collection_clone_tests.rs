use std::fs;
use std::io::Cursor;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};
use rusqlite::{params, Connection};

use crate::db::migrations;
use crate::db::repositories::collections::{create_collection, duplicate_collection};
use crate::db::repositories::icons::{duplicate_icon, list_icons};
use crate::db::repositories::imports::import_image_files;
use crate::db::repositories::optimization::{
    find_active_variant, insert_variant, set_active_variant, NewProcessedAssetVariant,
};
use crate::export::export_collection;
use crate::ids::create_id;
use crate::models::{ExportRequestPayload, ImportImageFilePayload};
use crate::optimization::analyzer::load_target;
use crate::optimization::cache::hash_text;
use crate::paths::AppPaths;
use crate::sheet::presets::{
    create_sheet_grid_preset, list_sheet_grid_presets, SheetGridPresetInput,
};

fn connection() -> Connection {
    let mut connection = Connection::open_in_memory().unwrap();
    migrations::run(&mut connection).unwrap();
    connection
}

fn temp_paths(label: &str) -> AppPaths {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    AppPaths::prepare(
        std::env::temp_dir().join(format!("pmtconcon-{label}-clone-completeness-{suffix}")),
    )
    .unwrap()
}

fn png_bytes(width: u32, height: u32) -> Vec<u8> {
    let image = ImageBuffer::from_pixel(width, height, Rgba([20, 40, 60, 255]));
    let mut cursor = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image)
        .write_to(&mut cursor, ImageFormat::Png)
        .unwrap();
    cursor.into_inner()
}

fn gif_bytes(width: u16, height: u16, colors: [[u8; 4]; 2]) -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut encoder = gif::Encoder::new(&mut bytes, width, height, &[]).unwrap();
        encoder.set_repeat(gif::Repeat::Finite(3)).unwrap();
        for (index, color) in colors.into_iter().enumerate() {
            let mut pixels = Vec::with_capacity(usize::from(width) * usize::from(height) * 4);
            for _ in 0..(usize::from(width) * usize::from(height)) {
                pixels.extend_from_slice(&color);
            }
            let mut frame = gif::Frame::from_rgba_speed(width, height, &mut pixels, 10);
            frame.delay = if index == 0 { 6 } else { 11 };
            encoder.write_frame(&frame).unwrap();
        }
    }
    bytes
}

fn default_profile_id(connection: &Connection, collection_id: &str) -> String {
    connection
        .query_row(
            "SELECT id
             FROM export_profiles
             WHERE collection_id = ?1
             ORDER BY created_at ASC
             LIMIT 1",
            [collection_id],
            |row| row.get(0),
        )
        .unwrap()
}

fn configure_double_icon(
    connection: &Connection,
    icon_id: &str,
    shape: &str,
    first_role: &str,
    second_role: &str,
    crop_width: i64,
    crop_height: i64,
    pingpong: bool,
) -> Vec<String> {
    connection
        .execute(
            "UPDATE icons
             SET shape = ?1,
                 cell_width_override = 8,
                 cell_height_override = 8,
                 gif_loop_mode = 'count',
                 gif_loop_count = 3,
                 gif_pingpong = ?2
             WHERE id = ?3",
            params![shape, i64::from(pingpong), icon_id],
        )
        .unwrap();
    connection
        .execute(
            "UPDATE crop_settings
             SET crop_x = 0,
                 crop_y = 0,
                 crop_w = ?1,
                 crop_h = ?2,
                 viewport_width_at_apply = ?1,
                 viewport_height_at_apply = ?2
             WHERE icon_id = ?3",
            params![crop_width, crop_height, icon_id],
        )
        .unwrap();
    let first_piece_id: String = connection
        .query_row(
            "SELECT id FROM icon_pieces WHERE icon_id = ?1 AND piece_index = 0",
            [icon_id],
            |row| row.get(0),
        )
        .unwrap();
    connection
        .execute(
            "UPDATE icon_pieces
             SET piece_role = ?1,
                 alt_text = ?2
             WHERE id = ?3",
            params![first_role, format!("{first_role}-alt"), first_piece_id],
        )
        .unwrap();
    let second_piece_id = create_id("piece");
    connection
        .execute(
            "INSERT INTO icon_pieces (
               id, icon_id, piece_index, piece_role, alt_text,
               created_at, updated_at
             )
             VALUES (
               ?1, ?2, 1, ?3, ?4,
               strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
               strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             )",
            params![
                second_piece_id,
                icon_id,
                second_role,
                format!("{second_role}-alt")
            ],
        )
        .unwrap();
    vec![first_piece_id, second_piece_id]
}

fn seed_active_gif_variant(
    connection: &Connection,
    paths: &AppPaths,
    icon_id: &str,
    profile_id: &str,
    piece_id: &str,
    bytes: &[u8],
) -> (String, String) {
    let target = load_target(connection, icon_id, profile_id, Some(piece_id)).unwrap();
    let variant_id = create_id("variant");
    let directory = paths
        .processed_variants_dir
        .join(icon_id)
        .join(profile_id)
        .join(piece_id);
    fs::create_dir_all(&directory).unwrap();
    let path = directory.join(format!("{variant_id}.gif"));
    fs::write(&path, bytes).unwrap();
    let settings_json = r#"{"mode":"clone-acceptance"}"#.to_string();
    insert_variant(
        connection,
        &NewProcessedAssetVariant {
            id: variant_id.clone(),
            icon_id: icon_id.to_string(),
            piece_id: Some(piece_id.to_string()),
            profile_id: Some(profile_id.to_string()),
            source_file_id: Some(target.source_file_id),
            kind: "optimized_gif".to_string(),
            preset: Some("custom".to_string()),
            path: path.to_string_lossy().to_string(),
            format: "gif".to_string(),
            width: 8,
            height: 8,
            byte_size: i64::try_from(bytes.len()).unwrap(),
            frame_count: Some(2),
            duration_ms: Some(170),
            loop_mode: Some("count".to_string()),
            settings_hash: hash_text(&[settings_json.clone()]),
            settings_json,
            source_hash: target.source_hash,
            crop_hash: target.crop_hash,
            profile_hash: target.profile_hash,
        },
    )
    .unwrap();
    set_active_variant(connection, &variant_id).unwrap();
    (variant_id, path.to_string_lossy().to_string())
}

fn insert_frame_sheet_recipe(connection: &Connection, paths: &AppPaths, icon_id: &str) -> String {
    let source_sheet = paths.sheet_import_originals_dir.join("frame-source.png");
    fs::write(&source_sheet, png_bytes(16, 8)).unwrap();
    let recipe_id = create_id("frame-gif-recipe");
    connection
        .execute(
            "INSERT INTO frame_sheet_gif_recipes (
               id, generated_icon_id, original_sheet_filename, original_sheet_path,
               original_sheet_sha256, recipe_schema, grid_settings_json, frames_json,
               direction, loop_mode, loop_count, measured_byte_size, render_hash
             )
             VALUES (
               ?1, ?2, 'frame-source.png', ?3, ?4,
               'pmtcon-frame-sheet-gif-v1', ?5, ?6,
               'pingpong', 'count', 3, 321, ?7
             )",
            params![
                recipe_id,
                icon_id,
                source_sheet.to_string_lossy(),
                "ab".repeat(32),
                r#"{"cellWidth":8,"cellHeight":8,"columns":2}"#,
                r#"[{"sourceCellIndex":0,"durationMs":60},{"sourceCellIndex":1,"durationMs":110}]"#,
                "cd".repeat(32),
            ],
        )
        .unwrap();
    recipe_id
}

fn create_collection_presets(connection: &Connection, collection_id: &str) {
    for (name, kind, frames_per_page) in [
        ("컬렉션 정적 프리셋", "static_import_export", None),
        ("컬렉션 GIF 프리셋", "gif_frame_export", Some(48)),
    ] {
        let preset = create_sheet_grid_preset(
            connection,
            SheetGridPresetInput {
                name: name.to_string(),
                scope: "collection".to_string(),
                collection_id: Some(collection_id.to_string()),
                kind: kind.to_string(),
                cell_width: 8,
                cell_height: 8,
                rows: None,
                columns: Some(4),
                mode: "rows_columns".to_string(),
                gap_x: 2,
                gap_y: 3,
                border_left: 4,
                border_top: 5,
                border_right: 6,
                border_bottom: 7,
                read_order: "column_major".to_string(),
                background: "checker".to_string(),
                max_sheet_width: 512,
                max_sheet_height: 512,
                frames_per_page,
                include_clean_sheet: true,
                include_guide_sheet: false,
                include_manifest: true,
                guide_label_options_json:
                    r#"{"cellNumber":true,"iconName":false,"altValue":true,"exportNumber":false}"#
                        .to_string(),
            },
        )
        .unwrap();
        connection
            .execute(
                "UPDATE sheet_grid_presets
                 SET is_default_for_import = ?1,
                     is_default_for_export = ?2,
                     is_default_for_gif_frame = ?3
                 WHERE id = ?4",
                params![
                    i64::from(kind == "static_import_export"),
                    i64::from(kind == "static_import_export"),
                    i64::from(kind == "gif_frame_export"),
                    preset.id,
                ],
            )
            .unwrap();
    }
}

fn export_payload(profile_id: &str, output_directory: &Path) -> ExportRequestPayload {
    ExportRequestPayload {
        profile_id: profile_id.to_string(),
        target_format: "gif".to_string(),
        target_cell_width: 8,
        target_cell_height: 8,
        max_bytes: 2_097_152,
        filename_mode: "sequence".to_string(),
        include_alt_txt: false,
        strict_warnings: false,
        output_directory: Some(output_directory.to_string_lossy().to_string()),
        open_folder_after_export: false,
        open_alt_txt_after_export: false,
        excluded_piece_ids: Vec::new(),
        resize_filter: "lanczos3".to_string(),
    }
}

#[test]
fn collection_clone_preserves_active_animated_multi_piece_output_presets_and_provenance() {
    let mut connection = connection();
    let paths = temp_paths("collection");
    let source = create_collection(&mut connection, Some("복제 수용 원본".to_string())).unwrap();
    let imported = import_image_files(
        &mut connection,
        &paths,
        &source.id,
        vec![
            ImportImageFilePayload {
                original_filename: "horizontal.gif".to_string(),
                bytes: gif_bytes(16, 8, [[255, 0, 0, 255], [180, 0, 80, 255]]),
            },
            ImportImageFilePayload {
                original_filename: "vertical.gif".to_string(),
                bytes: gif_bytes(8, 16, [[0, 160, 255, 255], [0, 255, 120, 255]]),
            },
        ],
    )
    .unwrap();
    assert!(imported.rejected_files.is_empty());
    let source_icons = imported.imported_icons;
    let horizontal_pieces = configure_double_icon(
        &connection,
        &source_icons[0].id,
        "horizontal_double",
        "left",
        "right",
        16,
        8,
        true,
    );
    let vertical_pieces = configure_double_icon(
        &connection,
        &source_icons[1].id,
        "vertical_double",
        "top",
        "bottom",
        8,
        16,
        false,
    );
    let source_profile_id = default_profile_id(&connection, &source.id);
    let variant_bytes = [
        gif_bytes(8, 8, [[255, 0, 0, 255], [128, 0, 0, 255]]),
        gif_bytes(8, 8, [[255, 180, 0, 255], [128, 80, 0, 255]]),
        gif_bytes(8, 8, [[0, 120, 255, 255], [0, 60, 128, 255]]),
        gif_bytes(8, 8, [[0, 255, 100, 255], [0, 128, 50, 255]]),
    ];
    let source_piece_groups = [horizontal_pieces, vertical_pieces];
    let mut source_variant_paths = Vec::new();
    for (icon_index, pieces) in source_piece_groups.iter().enumerate() {
        for (piece_index, piece_id) in pieces.iter().enumerate() {
            let (_, path) = seed_active_gif_variant(
                &connection,
                &paths,
                &source_icons[icon_index].id,
                &source_profile_id,
                piece_id,
                &variant_bytes[icon_index * 2 + piece_index],
            );
            source_variant_paths.push(path);
        }
    }
    let source_recipe_id = insert_frame_sheet_recipe(&connection, &paths, &source_icons[0].id);
    create_collection_presets(&connection, &source.id);

    let cloned = duplicate_collection(&mut connection, &paths, &source.id).unwrap();
    let cloned_profile_id = default_profile_id(&connection, &cloned.id);
    assert_ne!(cloned_profile_id, source_profile_id);
    let cloned_icons = list_icons(&connection, &cloned.id).unwrap();
    assert_eq!(cloned_icons.len(), 2);
    assert_eq!(cloned_icons[0].shape, "horizontal_double");
    assert_eq!(cloned_icons[0].gif_loop_mode, "pingpong");
    assert_eq!(cloned_icons[1].shape, "vertical_double");
    assert_eq!(cloned_icons[1].gif_loop_mode, "count");
    assert_eq!(
        cloned_icons[0]
            .pieces
            .iter()
            .map(|piece| piece.piece_role.as_str())
            .collect::<Vec<_>>(),
        vec!["left", "right"]
    );
    assert_eq!(
        cloned_icons[1]
            .pieces
            .iter()
            .map(|piece| piece.piece_role.as_str())
            .collect::<Vec<_>>(),
        vec!["top", "bottom"]
    );

    let mut cloned_variant_paths = Vec::new();
    for (icon_index, icon) in cloned_icons.iter().enumerate() {
        for (piece_index, piece) in icon.pieces.iter().enumerate() {
            let source_target = load_target(
                &connection,
                &source_icons[icon_index].id,
                &source_profile_id,
                Some(&source_piece_groups[icon_index][piece_index]),
            )
            .unwrap();
            let target =
                load_target(&connection, &icon.id, &cloned_profile_id, Some(&piece.id)).unwrap();
            let source_variant = find_active_variant(
                &connection,
                &source_target.icon_id,
                &source_profile_id,
                &source_target.piece_id,
                &source_target.source_hash,
                &source_target.crop_hash,
                &source_target.profile_hash,
                &source_target.output_format,
            )
            .unwrap()
            .unwrap();
            let cloned_variant = find_active_variant(
                &connection,
                &target.icon_id,
                &cloned_profile_id,
                &target.piece_id,
                &target.source_hash,
                &target.crop_hash,
                &target.profile_hash,
                &target.output_format,
            )
            .unwrap()
            .unwrap();
            assert_ne!(cloned_variant.id, source_variant.id);
            assert_ne!(cloned_variant.path, source_variant.path);
            assert_ne!(cloned_variant.profile_hash, source_variant.profile_hash);
            assert_eq!(cloned_variant.profile_hash, target.profile_hash);
            assert_eq!(
                fs::read(&cloned_variant.path).unwrap(),
                fs::read(&source_variant.path).unwrap()
            );
            assert!(Path::new(&cloned_variant.path).starts_with(
                paths
                    .processed_variants_dir
                    .join("cloned")
                    .join(&cloned.id)
                    .join(&icon.id)
            ));
            cloned_variant_paths.push(cloned_variant.path);
        }
    }

    let cloned_presets = list_sheet_grid_presets(&connection, Some(cloned.id.clone())).unwrap();
    let cloned_collection_presets = cloned_presets
        .into_iter()
        .filter(|preset| preset.scope == "collection")
        .collect::<Vec<_>>();
    assert_eq!(cloned_collection_presets.len(), 2);
    assert!(cloned_collection_presets.iter().any(|preset| {
        preset.name == "컬렉션 정적 프리셋"
            && preset.collection_id.as_deref() == Some(cloned.id.as_str())
            && preset.is_default_for_import
            && preset.is_default_for_export
            && !preset.is_default_for_gif_frame
    }));
    assert!(cloned_collection_presets.iter().any(|preset| {
        preset.name == "컬렉션 GIF 프리셋"
            && preset.collection_id.as_deref() == Some(cloned.id.as_str())
            && preset.is_default_for_gif_frame
            && preset.frames_per_page == Some(48)
    }));

    let (cloned_recipe_id, cloned_direction, cloned_frames): (String, String, String) = connection
        .query_row(
            "SELECT id, direction, frames_json
             FROM frame_sheet_gif_recipes
             WHERE generated_icon_id = ?1",
            [&cloned_icons[0].id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_ne!(cloned_recipe_id, source_recipe_id);
    assert_eq!(cloned_direction, "pingpong");
    assert!(cloned_frames.contains("durationMs"));
    connection
        .execute(
            "UPDATE frame_sheet_gif_recipes SET direction = 'reverse' WHERE id = ?1",
            [&cloned_recipe_id],
        )
        .unwrap();
    let source_direction: String = connection
        .query_row(
            "SELECT direction FROM frame_sheet_gif_recipes WHERE id = ?1",
            [&source_recipe_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(source_direction, "pingpong");

    let source_export = export_collection(
        &mut connection,
        &paths,
        &source.id,
        &export_payload(&source_profile_id, &paths.exports_dir.join("clone-source")),
    )
    .unwrap();
    let cloned_export = export_collection(
        &mut connection,
        &paths,
        &cloned.id,
        &export_payload(&cloned_profile_id, &paths.exports_dir.join("clone-target")),
    )
    .unwrap();
    assert_eq!(source_export.validation.items.len(), 4);
    assert_eq!(cloned_export.validation.items.len(), 4);
    for (source_item, cloned_item) in source_export
        .validation
        .items
        .iter()
        .zip(&cloned_export.validation.items)
    {
        assert_eq!(source_item.file_name, cloned_item.file_name);
        assert_eq!(source_item.piece_role, cloned_item.piece_role);
        assert_eq!(source_item.alt_text, cloned_item.alt_text);
        assert_eq!(
            fs::read(source_item.export_path.as_ref().unwrap()).unwrap(),
            fs::read(cloned_item.export_path.as_ref().unwrap()).unwrap()
        );
    }
    let source_report = fs::read_to_string(source_export.report_txt_path.unwrap()).unwrap();
    let cloned_report = fs::read_to_string(cloned_export.report_txt_path.unwrap()).unwrap();
    assert!(source_report.contains("optimized_variant"));
    assert!(cloned_report.contains("optimized_variant"));

    let cloned_job_count: i64 = connection
        .query_row(
            "SELECT COUNT(*)
             FROM optimization_jobs j
             JOIN icons i ON i.id = j.icon_id
             WHERE i.collection_id = ?1",
            [&cloned.id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(cloned_job_count, 0);
    for source_path in source_variant_paths {
        fs::remove_file(source_path).unwrap();
    }
    assert!(cloned_variant_paths
        .iter()
        .all(|path| Path::new(path).is_file()));
    fs::remove_dir_all(paths.root).unwrap();
}

#[test]
fn icon_clone_owns_active_variant_and_frame_sheet_recipe_with_shared_profile() {
    let mut connection = connection();
    let paths = temp_paths("icon");
    let collection =
        create_collection(&mut connection, Some("아이콘 복제 원본".to_string())).unwrap();
    let imported = import_image_files(
        &mut connection,
        &paths,
        &collection.id,
        vec![ImportImageFilePayload {
            original_filename: "single.gif".to_string(),
            bytes: gif_bytes(8, 8, [[255, 20, 20, 255], [20, 20, 255, 255]]),
        }],
    )
    .unwrap();
    let source_icon = &imported.imported_icons[0];
    let source_piece = &source_icon.pieces[0];
    let profile_id = default_profile_id(&connection, &collection.id);
    let active_bytes = gif_bytes(8, 8, [[255, 255, 0, 255], [255, 0, 255, 255]]);
    let (_, source_variant_path) = seed_active_gif_variant(
        &connection,
        &paths,
        &source_icon.id,
        &profile_id,
        &source_piece.id,
        &active_bytes,
    );
    let source_recipe_id = insert_frame_sheet_recipe(&connection, &paths, &source_icon.id);

    let cloned = duplicate_icon(&mut connection, &paths, &collection.id, &source_icon.id).unwrap();
    let target = load_target(
        &connection,
        &cloned.id,
        &profile_id,
        Some(&cloned.pieces[0].id),
    )
    .unwrap();
    let target_variant = find_active_variant(
        &connection,
        &target.icon_id,
        &profile_id,
        &target.piece_id,
        &target.source_hash,
        &target.crop_hash,
        &target.profile_hash,
        &target.output_format,
    )
    .unwrap()
    .unwrap();
    assert_ne!(target_variant.path, source_variant_path);
    assert_eq!(fs::read(&target_variant.path).unwrap(), active_bytes);
    let target_recipe_id: String = connection
        .query_row(
            "SELECT id FROM frame_sheet_gif_recipes WHERE generated_icon_id = ?1",
            [&cloned.id],
            |row| row.get(0),
        )
        .unwrap();
    assert_ne!(target_recipe_id, source_recipe_id);
    fs::remove_file(source_variant_path).unwrap();
    assert!(Path::new(&target_variant.path).is_file());
    fs::remove_dir_all(paths.root).unwrap();
}
#[test]
fn collection_clone_skips_missing_and_stale_active_variants() {
    let mut connection = connection();
    let paths = temp_paths("fallback");
    let source =
        create_collection(&mut connection, Some("최적화 fallback 원본".to_string())).unwrap();
    let imported = import_image_files(
        &mut connection,
        &paths,
        &source.id,
        vec![
            ImportImageFilePayload {
                original_filename: "missing.gif".to_string(),
                bytes: gif_bytes(8, 8, [[255, 0, 0, 255], [0, 0, 0, 255]]),
            },
            ImportImageFilePayload {
                original_filename: "stale.gif".to_string(),
                bytes: gif_bytes(8, 8, [[0, 255, 0, 255], [0, 0, 0, 255]]),
            },
        ],
    )
    .unwrap();
    let profile_id = default_profile_id(&connection, &source.id);
    let (_, missing_path) = seed_active_gif_variant(
        &connection,
        &paths,
        &imported.imported_icons[0].id,
        &profile_id,
        &imported.imported_icons[0].pieces[0].id,
        &gif_bytes(8, 8, [[255, 255, 0, 255], [0, 0, 0, 255]]),
    );
    let (stale_variant_id, _) = seed_active_gif_variant(
        &connection,
        &paths,
        &imported.imported_icons[1].id,
        &profile_id,
        &imported.imported_icons[1].pieces[0].id,
        &gif_bytes(8, 8, [[0, 255, 255, 255], [0, 0, 0, 255]]),
    );
    fs::remove_file(missing_path).unwrap();
    connection
        .execute(
            "UPDATE processed_asset_variants
             SET crop_hash = 'stale-crop-hash'
             WHERE id = ?1",
            [&stale_variant_id],
        )
        .unwrap();

    let cloned = duplicate_collection(&mut connection, &paths, &source.id).unwrap();
    let cloned_variant_count: i64 = connection
        .query_row(
            "SELECT COUNT(*)
             FROM processed_asset_variants v
             JOIN icons i ON i.id = v.icon_id
             WHERE i.collection_id = ?1",
            [&cloned.id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(cloned_variant_count, 0);

    fs::remove_dir_all(paths.root).unwrap();
}
