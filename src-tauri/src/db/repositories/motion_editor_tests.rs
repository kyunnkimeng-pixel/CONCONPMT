use std::io::Cursor;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use rusqlite::Connection;

use crate::db::migrations;
use crate::db::repositories::collections::create_collection;
use crate::db::repositories::effects::upsert_effect_recipe;
use crate::db::repositories::imports::import_image_files;
use crate::db::repositories::motion_editor::{
    commit_motion_update, prepare_motion_preview, prepare_motion_update, render_motion_preview,
    render_motion_update,
};
use crate::imaging::effects::{EffectRecipe, EffectStep, EFFECT_RECIPE_VERSION};
use crate::imaging::motion::{MotionRecipe, SpatialMotion};
use crate::models::{ImportImageFilePayload, PreviewIconMotionPayload, UpdateIconMotionPayload};
use crate::paths::AppPaths;

#[test]
fn measured_motion_save_rechecks_static_effects_and_preserves_original() {
    let mut connection = Connection::open_in_memory().unwrap();
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .unwrap();
    migrations::run(&mut connection).unwrap();
    let paths = temp_paths();
    let collection =
        create_collection(&mut connection, Some("모션 저장 테스트".to_string())).unwrap();
    let original_bytes = source_png_bytes();
    let imported = import_image_files(
        &mut connection,
        &paths,
        &collection.id,
        vec![ImportImageFilePayload {
            original_filename: "motion-source.png".to_string(),
            bytes: original_bytes.clone(),
        }],
    )
    .unwrap();
    let icon_id = imported.imported_icons[0].id.clone();
    let original_path: String = connection
        .query_row(
            "SELECT s.original_path_in_library
             FROM source_files s
             JOIN icons i ON i.source_file_id = s.id
             WHERE i.id = ?1",
            [&icon_id],
            |row| row.get(0),
        )
        .unwrap();
    let recipe = MotionRecipe {
        duration_ms: 400,
        fps: 10,
        spatial: Some(SpatialMotion::Breathe {
            enabled: true,
            cycles_per_loop: 1,
            scale_percent: 8,
        }),
        ..MotionRecipe::default()
    };

    let first_measurement = render_motion_preview(
        &paths,
        prepare_motion_preview(
            &connection,
            &collection.id,
            PreviewIconMotionPayload {
                icon_id: icon_id.clone(),
                recipe: recipe.clone(),
            },
        )
        .unwrap(),
    )
    .unwrap();
    assert!(Path::new(&first_measurement.preview_path).is_file());
    assert!(Path::new(&first_measurement.poster_path).is_file());
    assert_eq!(first_measurement.frame_count, 4);
    assert_eq!(first_measurement.loop_mode, "infinite");
    assert_eq!(first_measurement.loop_count, None);
    assert_eq!(
        first_measurement.byte_size as u64,
        std::fs::metadata(&first_measurement.preview_path)
            .unwrap()
            .len()
    );

    let stale_render = render_motion_update(
        &paths,
        prepare_motion_update(
            &connection,
            &collection.id,
            UpdateIconMotionPayload {
                icon_id: icon_id.clone(),
                expected_revision: 0,
                expected_render_signature: first_measurement.render_signature,
                recipe: recipe.clone(),
            },
        )
        .unwrap(),
    )
    .unwrap();

    let effect_recipe = EffectRecipe {
        version: EFFECT_RECIPE_VERSION,
        effects: vec![EffectStep::Pixelate {
            id: "pixel-after-measurement".to_string(),
            enabled: true,
            block_size: 2,
        }],
    };
    let transaction = connection.transaction().unwrap();
    upsert_effect_recipe(&transaction, &collection.id, &icon_id, 0, &effect_recipe).unwrap();
    transaction.commit().unwrap();

    let stale_error = commit_motion_update(&mut connection, stale_render).unwrap_err();
    assert_eq!(stale_error.code, "conflict");

    let fresh_measurement = render_motion_preview(
        &paths,
        prepare_motion_preview(
            &connection,
            &collection.id,
            PreviewIconMotionPayload {
                icon_id: icon_id.clone(),
                recipe: recipe.clone(),
            },
        )
        .unwrap(),
    )
    .unwrap();
    let rendered = render_motion_update(
        &paths,
        prepare_motion_update(
            &connection,
            &collection.id,
            UpdateIconMotionPayload {
                icon_id: icon_id.clone(),
                expected_revision: 0,
                expected_render_signature: fresh_measurement.render_signature,
                recipe: recipe.clone(),
            },
        )
        .unwrap(),
    )
    .unwrap();
    let saved = commit_motion_update(&mut connection, rendered).unwrap();

    assert_eq!(saved.motion_revision, 1);
    assert_eq!(saved.motion_recipe, recipe);
    assert_eq!(saved.effect_revision, 1);
    let first_preview_path = saved.icon.current_preview_url.clone().unwrap();
    assert!(first_preview_path.ends_with(".gif"));
    assert!(Path::new(&first_preview_path).is_file());
    let first_revision_dir = Path::new(&first_preview_path)
        .parent()
        .unwrap()
        .to_path_buf();
    assert!(first_revision_dir.is_dir());

    let second_recipe = MotionRecipe {
        spatial: Some(SpatialMotion::Breathe {
            enabled: true,
            cycles_per_loop: 1,
            scale_percent: 12,
        }),
        ..recipe.clone()
    };
    let second_measurement = render_motion_preview(
        &paths,
        prepare_motion_preview(
            &connection,
            &collection.id,
            PreviewIconMotionPayload {
                icon_id: icon_id.clone(),
                recipe: second_recipe.clone(),
            },
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(second_measurement.loop_mode, "infinite");
    assert_eq!(second_measurement.loop_count, None);
    let second_rendered = render_motion_update(
        &paths,
        prepare_motion_update(
            &connection,
            &collection.id,
            UpdateIconMotionPayload {
                icon_id: icon_id.clone(),
                expected_revision: 1,
                expected_render_signature: second_measurement.render_signature,
                recipe: second_recipe.clone(),
            },
        )
        .unwrap(),
    )
    .unwrap();
    let second_saved = commit_motion_update(&mut connection, second_rendered).unwrap();
    let second_preview_path = second_saved.icon.current_preview_url.clone().unwrap();
    let second_revision_dir = Path::new(&second_preview_path)
        .parent()
        .unwrap()
        .to_path_buf();

    assert_eq!(second_saved.motion_revision, 2);
    assert_eq!(second_saved.motion_recipe, second_recipe);
    assert!(Path::new(&second_preview_path).is_file());
    assert!(second_revision_dir.is_dir());
    assert_ne!(first_revision_dir, second_revision_dir);
    assert!(!first_revision_dir.exists());
    assert_eq!(std::fs::read(original_path).unwrap(), original_bytes);

    std::fs::remove_dir_all(paths.root).unwrap();
}

fn source_png_bytes() -> Vec<u8> {
    let mut image = RgbaImage::new(12, 8);
    for (x, y, pixel) in image.enumerate_pixels_mut() {
        *pixel = if x < 6 {
            Rgba([255, y as u8 * 12, 0, 255])
        } else {
            Rgba([0, y as u8 * 12, 255, 255])
        };
    }
    let mut cursor = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image)
        .write_to(&mut cursor, ImageFormat::Png)
        .unwrap();
    cursor.into_inner()
}

fn temp_paths() -> AppPaths {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    AppPaths::prepare(std::env::temp_dir().join(format!("pmtconcon-motion-editor-test-{suffix}")))
        .unwrap()
}
