use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};
use rusqlite::{params, Connection, OptionalExtension, Row, Transaction};

use crate::db::repositories::clone_artifacts::{
    cleanup_cloned_icon_previews, clone_current_ai_lineage, clone_effective_active_variants,
    clone_frame_sheet_gif_recipe, clone_source_free_root_provenance,
    materialize_clone_native_preview, validate_icon_clone_source, validate_icon_clone_target,
};
use crate::db::repositories::effects as effect_repository;
use crate::db::repositories::source_files::{
    import_source_file_from_bytes, SourceFileImportOptions,
};
use crate::error::{AppError, AppResult};
use crate::ids::create_id;
use crate::imaging::geometry::viewport_size;
use crate::imaging::motion::parse_motion_recipe_json;
use crate::imaging::preview::{
    generate_icon_preview_in_directory, CropRect as PreviewCropRect, GeneratePreviewRequest,
};
use crate::imaging::text_overlay::text_overlay_from_fields;
use crate::imaging::transform::ImageTransform;
use crate::models::{CreatePlaceholderIconPayload, IconDto, IconPieceDto, ImportImageFilePayload};
use crate::paths::AppPaths;

pub fn list_icons(connection: &Connection, collection_id: &str) -> AppResult<Vec<IconDto>> {
    let collection_exists = connection
        .query_row(
            "SELECT id
             FROM collections
             WHERE id = ?1
               AND deleted_at IS NULL",
            params![collection_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .is_some();

    if !collection_exists {
        return Err(AppError::not_found(
            "아이콘을 조회할 모음을 찾을 수 없습니다.",
        ));
    }

    let mut statement = connection.prepare(
        "SELECT
           id,
           collection_id,
           source_file_id,
           display_name,
           (
             SELECT note
             FROM icon_notes
             WHERE icon_id = icons.id
           ) AS note,
           icon_kind,
           readiness,
           placeholder_text,
           shape,
           order_index,
           cell_width_override,
           cell_height_override,
           CASE
             WHEN current_preview_path IS NULL AND EXISTS (
               SELECT 1 FROM icon_ai_state preview_state
               WHERE preview_state.icon_id = icons.id
                 AND preview_state.active_version_id IS NOT NULL
             ) THEN NULL
             ELSE thumbnail_path
           END AS thumbnail_path,
           thumbnail_override_path,
           current_preview_path,
           transform_quarter_turns,
           transform_flip_horizontal,
           transform_flip_vertical,
           CASE WHEN gif_pingpong = 1 THEN 'pingpong' ELSE gif_loop_mode END AS gif_loop_mode,
           gif_loop_count,
           created_at,
           updated_at
         FROM icons
         WHERE collection_id = ?1
           AND deleted_at IS NULL
         ORDER BY order_index ASC, created_at ASC",
    )?;

    let icon_records = statement
        .query_map(params![collection_id], |row| icon_from_row(connection, row))?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(icon_records)
}

pub fn update_icon_piece_alt(
    connection: &Connection,
    collection_id: &str,
    piece_id: &str,
    alt_text: String,
) -> AppResult<IconDto> {
    let normalized_alt = normalized_alt_text(alt_text);

    let icon_id = icon_id_for_piece(connection, collection_id, piece_id)?;

    let changed = connection.execute(
        "UPDATE icon_pieces
         SET alt_text = ?1,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?2
           AND icon_id = ?3",
        params![normalized_alt, piece_id, icon_id],
    )?;

    if changed == 0 {
        return Err(AppError::not_found("수정할 alt 값을 찾을 수 없습니다."));
    }

    connection.execute(
        "UPDATE icons
         SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?1",
        params![icon_id],
    )?;

    get_icon(connection, collection_id, &icon_id)
}

pub fn rename_icon(
    connection: &Connection,
    collection_id: &str,
    icon_id: &str,
    display_name: String,
) -> AppResult<IconDto> {
    ensure_collection_exists(connection, collection_id)?;
    let display_name = normalized_display_name(display_name);

    let changed = connection.execute(
        "UPDATE icons
         SET display_name = ?1,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?2
           AND collection_id = ?3
           AND deleted_at IS NULL",
        params![display_name, icon_id, collection_id],
    )?;

    if changed == 0 {
        return Err(AppError::not_found(
            "이름을 변경할 아이콘을 찾을 수 없습니다.",
        ));
    }

    get_icon(connection, collection_id, icon_id)
}

pub fn get_icon_note(
    connection: &Connection,
    collection_id: &str,
    icon_id: &str,
) -> AppResult<Option<String>> {
    ensure_icon_exists(connection, collection_id, icon_id)?;
    let note = connection
        .query_row(
            "SELECT note
             FROM icon_notes
             WHERE icon_id = ?1",
            params![icon_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    Ok(note)
}

pub fn update_icon_note(
    connection: &Connection,
    collection_id: &str,
    icon_id: &str,
    note: String,
) -> AppResult<IconDto> {
    ensure_icon_exists(connection, collection_id, icon_id)?;
    let normalized = normalized_note(note);
    if normalized.is_empty() {
        return clear_icon_note(connection, collection_id, icon_id);
    }

    connection.execute(
        "INSERT INTO icon_notes (icon_id, note, updated_at)
         VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
         ON CONFLICT(icon_id) DO UPDATE SET
           note = excluded.note,
           updated_at = excluded.updated_at",
        params![icon_id, normalized],
    )?;
    connection.execute(
        "UPDATE icons
         SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?1
           AND collection_id = ?2
           AND deleted_at IS NULL",
        params![icon_id, collection_id],
    )?;

    get_icon(connection, collection_id, icon_id)
}

pub fn clear_icon_note(
    connection: &Connection,
    collection_id: &str,
    icon_id: &str,
) -> AppResult<IconDto> {
    ensure_icon_exists(connection, collection_id, icon_id)?;
    connection.execute(
        "DELETE FROM icon_notes
         WHERE icon_id = ?1",
        params![icon_id],
    )?;
    connection.execute(
        "UPDATE icons
         SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?1
           AND collection_id = ?2
           AND deleted_at IS NULL",
        params![icon_id, collection_id],
    )?;

    get_icon(connection, collection_id, icon_id)
}

pub fn set_icon_thumbnail_override(
    connection: &mut Connection,
    paths: &AppPaths,
    collection_id: &str,
    icon_id: &str,
    file: ImportImageFilePayload,
) -> AppResult<IconDto> {
    let transaction = connection.transaction()?;
    ensure_collection_exists(&transaction, collection_id)?;
    ensure_icon_exists(&transaction, collection_id, icon_id)?;
    let source_file = import_source_file_from_bytes(
        &transaction,
        paths,
        &file,
        SourceFileImportOptions {
            allow_gif: true,
            exact_dimensions: None,
        },
    )?;

    transaction.execute(
        "UPDATE icons
         SET thumbnail_override_source_file_id = ?1,
             thumbnail_override_path = ?2,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?3
           AND collection_id = ?4
           AND deleted_at IS NULL",
        params![
            source_file.id,
            source_file.thumbnail_path,
            icon_id,
            collection_id
        ],
    )?;
    transaction.commit()?;

    get_icon(connection, collection_id, icon_id)
}

pub fn create_placeholder_icon(
    connection: &mut Connection,
    paths: &AppPaths,
    collection_id: &str,
    payload: CreatePlaceholderIconPayload,
) -> AppResult<IconDto> {
    let label = normalized_placeholder_label(payload.label);
    let placeholder_bytes = transparent_png_bytes(200, 200)?;
    let transaction = connection.transaction()?;
    let collection = collection_sizing(&transaction, collection_id)?;
    let source_file = import_source_file_from_bytes(
        &transaction,
        paths,
        &ImportImageFilePayload {
            original_filename: "blank-dccon.png".to_string(),
            bytes: placeholder_bytes,
        },
        SourceFileImportOptions {
            allow_gif: false,
            exact_dimensions: None,
        },
    )?;
    let icon_id = create_id("icon");
    let order_index = next_icon_order_index(&transaction, collection_id)?;
    let crop = centered_crop_rect(
        source_file.width,
        source_file.height,
        collection.default_cell_width,
        collection.default_cell_height,
    );

    transaction.execute(
        "INSERT INTO icons (
           id,
           collection_id,
           source_file_id,
           display_name,
           icon_kind,
           readiness,
           placeholder_text,
           shape,
           order_index,
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
           'placeholder',
           'working',
           ?5,
           'single',
           ?6,
           ?7,
           ?8,
           strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
           strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         )",
        params![
            icon_id,
            collection_id,
            source_file.id,
            label,
            label,
            order_index,
            source_file.thumbnail_path,
            source_file.thumbnail_path,
        ],
    )?;
    insert_crop_settings(
        &transaction,
        &icon_id,
        crop,
        source_file.width,
        source_file.height,
        collection.default_cell_width,
        collection.default_cell_height,
    )?;
    insert_piece(&transaction, &icon_id, 0, "single", "")?;
    transaction.commit()?;

    get_icon(connection, collection_id, &icon_id)
}

pub fn replace_icon_source(
    connection: &mut Connection,
    paths: &AppPaths,
    collection_id: &str,
    icon_id: &str,
    file: ImportImageFilePayload,
) -> AppResult<IconDto> {
    let transaction = connection.transaction()?;
    ensure_collection_exists(&transaction, collection_id)?;
    let icon = icon_record_for_replace(&transaction, collection_id, icon_id)?;
    let source_file = import_source_file_from_bytes(
        &transaction,
        paths,
        &file,
        SourceFileImportOptions {
            allow_gif: true,
            exact_dimensions: None,
        },
    )?;
    let display_name = if icon.icon_kind == "placeholder" {
        display_name_from_filename(&file.original_filename)
    } else {
        icon.display_name
    };
    let viewport = viewport_size(&icon.shape, icon.cell_width, icon.cell_height)?;
    let crop = centered_crop_rect(
        source_file.width,
        source_file.height,
        viewport.width,
        viewport.height,
    );
    let (source_gif_loop_mode, source_gif_loop_count) = transaction.query_row(
        "SELECT COALESCE(original_loop_mode, 'preserve'), original_loop_count
         FROM source_files
         WHERE id = ?1",
        [&source_file.id],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<i64>>(1)?)),
    )?;
    let text_overlay = text_overlay_from_fields(
        icon.text_overlay_enabled,
        Some(icon.text_overlay_text.clone()),
        icon.text_overlay_font_path.clone(),
        Some(icon.text_overlay_font_size),
        Some(icon.text_overlay_x),
        Some(icon.text_overlay_y),
        Some(icon.text_overlay_color.clone()),
        Some(icon.text_overlay_stroke_color.clone()),
        Some(icon.text_overlay_stroke_width),
    )?;
    let effects = effect_repository::effect_recipe_for_icon(&transaction, collection_id, icon_id)?;
    let motion_json = transaction
        .query_row(
            "SELECT motion_json FROM icon_motion_recipes WHERE icon_id = ?1",
            [icon_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    let motion = parse_motion_recipe_json(motion_json.as_deref().unwrap_or_default())?;
    if icon.original_lineage_generation == i64::MAX {
        return Err(AppError::new(
            "ai_lineage_overflow",
            "AI 원본 계보 번호를 증가시킬 수 없습니다.",
        ));
    }
    let operation_id = create_id("source_replace");
    let staging_dir = paths.ai_activation_staging_dir.join(&operation_id);
    let _ = fs::remove_dir_all(&staging_dir);
    let preview = match generate_icon_preview_in_directory(
        &staging_dir,
        GeneratePreviewRequest {
            collection_id,
            icon_id,
            source_path: Path::new(&source_file.original_path_in_library),
            source_extension: &source_file.original_extension,
            shape: &icon.shape,
            crop: PreviewCropRect {
                x: crop.x,
                y: crop.y,
                width: crop.width,
                height: crop.height,
            },
            cell_width: icon.cell_width,
            cell_height: icon.cell_height,
            transform: ImageTransform::new(0, false, false)?,
            gif_loop_mode: &icon.gif_loop_mode,
            gif_loop_count: icon.gif_loop_count,
            source_gif_loop_mode: Some(&source_gif_loop_mode),
            source_gif_loop_count,
            text_overlay,
            effects: effects.recipe,
            motion,
        },
    ) {
        Ok(preview) => preview,
        Err(error) => {
            let _ = fs::remove_dir_all(&staging_dir);
            return Err(error);
        }
    };
    let next_lineage_id = create_id("lineage");
    let next_lineage_generation =
        icon.original_lineage_generation
            .checked_add(1)
            .ok_or_else(|| {
                AppError::new(
                    "ai_lineage_overflow",
                    "AI 원본 계보 번호를 증가시킬 수 없습니다.",
                )
            })?;

    let final_dir = paths
        .ai_activation_previews_dir
        .join(collection_id)
        .join(icon_id)
        .join(&operation_id);
    let promoted = (|| -> AppResult<(String, Vec<PathBuf>)> {
        if let Some(parent) = final_dir.parent() {
            fs::create_dir_all(parent)?;
        }
        if final_dir.exists() {
            return Err(AppError::new(
                "source_replace_preview_path",
                "원본 교체 미리보기 경로가 이미 존재합니다.",
            ));
        }
        fs::rename(&staging_dir, &final_dir)?;
        let current_preview_path =
            rebase_preview_artifact(&preview.current_preview_path, &staging_dir, &final_dir)?
                .to_string_lossy()
                .to_string();
        let piece_paths = preview
            .piece_paths
            .iter()
            .map(|path| rebase_preview_artifact(path, &staging_dir, &final_dir))
            .collect::<AppResult<Vec<_>>>()?;
        Ok((current_preview_path, piece_paths))
    })();
    let (current_preview_path, promoted_piece_paths) = match promoted {
        Ok(paths) => paths,
        Err(error) => {
            let _ = fs::remove_dir_all(&staging_dir);
            let _ = fs::remove_dir_all(&final_dir);
            return Err(error);
        }
    };

    let commit_result = (|| -> AppResult<()> {
        transaction.execute(
            "UPDATE ai_requests
         SET status = 'cancelled',
             superseded_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
             superseded_reason = 'original_source_replaced',
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE (
             origin_icon_id = ?1
             OR EXISTS (
               SELECT 1
               FROM ai_request_items request_item
               WHERE request_item.request_id = ai_requests.id
                 AND request_item.origin_icon_id = ?1
             )
           )
           AND status IN ('draft', 'prepared', 'awaiting_result', 'running', 'layout_review_pending')",
            [icon_id],
        )?;
        let state_rows = transaction.execute(
            "UPDATE icon_ai_state
         SET active_version_id = NULL,
             revision = revision + 1,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE icon_id = ?1",
            [icon_id],
        )?;
        if state_rows != 1 {
            return Err(AppError::new(
                "ai_state_missing",
                "원본 교체 전에 아이콘의 AI 상태를 찾을 수 없습니다.",
            ));
        }
        transaction.execute(
            "UPDATE processed_asset_variants
         SET is_active_for_export = 0
         WHERE icon_id = ?1
           AND is_active_for_export = 1",
            [icon_id],
        )?;
        transaction.execute(
            "UPDATE optimization_jobs
         SET status = 'cancelled',
             message = '원본 이미지 교체로 취소됨',
             finished_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE icon_id = ?1
           AND status IN ('queued', 'running')",
            [icon_id],
        )?;

        transaction.execute(
            "UPDATE icons
         SET source_file_id = ?1,
             original_lineage_id = ?2,
             original_lineage_generation = ?3,
             display_name = ?4,
             icon_kind = 'image',
             placeholder_text = NULL,
             thumbnail_path = ?5,
             thumbnail_override_source_file_id = NULL,
             thumbnail_override_path = NULL,
             current_preview_path = ?6,
             transform_quarter_turns = 0,
             transform_flip_horizontal = 0,
             transform_flip_vertical = 0,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?7
           AND collection_id = ?8
           AND deleted_at IS NULL",
            params![
                source_file.id,
                next_lineage_id,
                next_lineage_generation,
                display_name,
                source_file.thumbnail_path,
                current_preview_path,
                icon_id,
                collection_id,
            ],
        )?;
        transaction.execute(
            "UPDATE collections
         SET cover_source_file_id = ?1,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?2
           AND cover_icon_id = ?3",
            params![source_file.id, collection_id, icon_id],
        )?;
        replace_crop_settings(
            &transaction,
            icon_id,
            crop,
            source_file.width,
            source_file.height,
            viewport.width,
            viewport.height,
        )?;
        let mut updated_piece_previews = 0;
        for (piece_index, piece_path) in promoted_piece_paths.iter().enumerate() {
            updated_piece_previews += transaction.execute(
                "UPDATE icon_pieces
             SET generated_preview_path = ?1,
                 last_export_path = NULL,
                 export_status = 'ready',
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE icon_id = ?2
               AND piece_index = ?3",
                params![
                    piece_path.to_string_lossy().to_string(),
                    icon_id,
                    piece_index as i64,
                ],
            )?;
        }
        if updated_piece_previews != promoted_piece_paths.len() {
            return Err(AppError::new(
                "validation",
                "교체한 이미지의 조각 미리보기를 연결할 수 없습니다.",
            ));
        }
        transaction.commit()?;
        Ok(())
    })();
    if commit_result.is_err() {
        let _ = fs::remove_dir_all(&staging_dir);
        let _ = fs::remove_dir_all(&final_dir);
    }
    commit_result?;

    get_icon(connection, collection_id, icon_id)
}

pub fn set_icons_readiness(
    connection: &Connection,
    collection_id: &str,
    icon_ids: Vec<String>,
    readiness: String,
) -> AppResult<Vec<IconDto>> {
    ensure_collection_exists(connection, collection_id)?;
    let readiness = normalized_readiness(readiness)?;
    if icon_ids.is_empty() {
        return list_icons(connection, collection_id);
    }

    let mut changed = 0;
    for icon_id in icon_ids {
        changed += connection.execute(
            "UPDATE icons
             SET readiness = ?1,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?2
               AND collection_id = ?3
               AND deleted_at IS NULL",
            params![readiness, icon_id, collection_id],
        )?;
    }

    if changed == 0 {
        return Err(AppError::not_found(
            "상태를 변경할 아이콘을 찾을 수 없습니다.",
        ));
    }

    list_icons(connection, collection_id)
}

pub fn duplicate_icon(
    connection: &mut Connection,
    paths: &AppPaths,
    collection_id: &str,
    icon_id: &str,
) -> AppResult<IconDto> {
    validate_icon_clone_source(connection, collection_id, icon_id)?;
    let duplicate_icon_id = create_id("icon");
    let duplicate_result = (|| -> AppResult<IconDto> {
        let transaction = connection.transaction()?;
        ensure_collection_exists(&transaction, collection_id)?;
        let icon = icon_record_for_duplicate(&transaction, collection_id, icon_id)?;
        let order_index = icon.order_index + 1;
        transaction.execute(
            "UPDATE icons
             SET order_index = order_index + 1
             WHERE collection_id = ?1
               AND deleted_at IS NULL
               AND order_index >= ?2",
            params![collection_id, order_index],
        )?;
        let duplicate_name = format!("{} 복사본", icon.display_name);
        transaction.execute(
            "INSERT INTO icons (
               id,
               collection_id,
               source_file_id,
               display_name,
               icon_kind,
               readiness,
               placeholder_text,
               shape,
               order_index,
               cell_width_override,
               cell_height_override,
               thumbnail_path,
               thumbnail_override_source_file_id,
               thumbnail_override_path,
               current_preview_path,
               text_overlay_enabled,
               text_overlay_text,
               text_overlay_font_path,
               text_overlay_font_size,
               text_overlay_x,
               text_overlay_y,
               text_overlay_color,
               text_overlay_stroke_color,
               text_overlay_stroke_width,
               transform_quarter_turns,
               transform_flip_horizontal,
               transform_flip_vertical,
               gif_loop_mode,
               gif_loop_count,
               gif_pingpong,
               created_at,
               updated_at
             )
             VALUES (
               ?1,
               ?2,
               ?3,
               ?4,
               ?5,
               ?6,
               ?7,
               ?8,
               ?9,
               ?10,
               ?11,
               ?12,
               ?13,
               ?14,
               ?15,
               ?16,
               ?17,
               ?18,
               ?19,
               ?20,
               ?21,
               ?22,
               ?23,
               ?24,
               ?25,
               ?26,
               ?27,
               ?28,
               ?29,
               ?30,
               strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
               strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             )",
            params![
                duplicate_icon_id,
                collection_id,
                icon.source_file_id,
                duplicate_name,
                icon.icon_kind,
                icon.readiness,
                icon.placeholder_text,
                icon.shape,
                order_index,
                icon.cell_width_override,
                icon.cell_height_override,
                icon.thumbnail_path,
                icon.thumbnail_override_source_file_id,
                icon.thumbnail_override_path,
                Option::<String>::None,
                icon.text_overlay_enabled,
                icon.text_overlay_text,
                icon.text_overlay_font_path,
                icon.text_overlay_font_size,
                icon.text_overlay_x,
                icon.text_overlay_y,
                icon.text_overlay_color,
                icon.text_overlay_stroke_color,
                icon.text_overlay_stroke_width,
                icon.transform_quarter_turns,
                icon.transform_flip_horizontal,
                icon.transform_flip_vertical,
                icon.gif_loop_mode,
                icon.gif_loop_count,
                icon.gif_pingpong,
            ],
        )?;

        let piece_id_map = duplicate_icon_pieces(
            &transaction,
            paths,
            collection_id,
            icon_id,
            &duplicate_icon_id,
        )?;
        duplicate_crop_settings(&transaction, icon_id, &duplicate_icon_id)?;
        duplicate_icon_note(&transaction, icon_id, &duplicate_icon_id)?;
        duplicate_icon_effect_recipe(&transaction, icon_id, &duplicate_icon_id)?;
        duplicate_icon_motion_recipe(&transaction, icon_id, &duplicate_icon_id)?;
        clone_frame_sheet_gif_recipe(&transaction, icon_id, &duplicate_icon_id)?;
        clone_current_ai_lineage(&transaction, icon_id, &duplicate_icon_id)?;
        clone_source_free_root_provenance(&transaction, icon_id, &duplicate_icon_id)?;
        validate_icon_clone_target(&transaction, collection_id, &duplicate_icon_id)?;
        materialize_clone_native_preview(&transaction, paths, collection_id, &duplicate_icon_id)?;
        clone_effective_active_variants(
            &transaction,
            paths,
            collection_id,
            icon_id,
            &duplicate_icon_id,
            &piece_id_map,
            None,
        )?;
        let duplicated = get_icon(&transaction, collection_id, &duplicate_icon_id)?;
        transaction.commit()?;
        Ok(duplicated)
    })();

    if duplicate_result.is_err() {
        cleanup_cloned_icon_previews(paths, collection_id, &duplicate_icon_id);
    }
    duplicate_result
}

pub fn delete_icons(
    connection: &mut Connection,
    collection_id: &str,
    icon_ids: Vec<String>,
) -> AppResult<()> {
    if icon_ids.is_empty() {
        return Ok(());
    }

    let transaction = connection.transaction()?;
    ensure_collection_exists(&transaction, collection_id)?;

    let requested_ids: HashSet<&str> = icon_ids.iter().map(String::as_str).collect();
    let mut deleted_count = 0;

    for icon_id in &requested_ids {
        transaction.execute(
            "UPDATE ai_requests
             SET status = 'cancelled',
                 superseded_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                 superseded_reason = 'target_icon_deleted',
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE (
                 origin_icon_id = ?1
                 OR EXISTS (
                   SELECT 1
                   FROM ai_request_items request_item
                   WHERE request_item.request_id = ai_requests.id
                     AND request_item.origin_icon_id = ?1
                 )
               )
               AND status IN ('draft', 'prepared', 'awaiting_result', 'running', 'layout_review_pending')",
            [icon_id],
        )?;
        let changed = transaction.execute(
            "UPDATE icons
             SET deleted_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?1
               AND collection_id = ?2
               AND deleted_at IS NULL",
            params![icon_id, collection_id],
        )?;
        deleted_count += changed;
    }

    if deleted_count == 0 {
        return Err(AppError::not_found("삭제할 아이콘을 찾을 수 없습니다."));
    }

    repair_cover_after_delete(&transaction, collection_id, &requested_ids)?;
    compact_icon_order(&transaction, collection_id)?;
    transaction.commit()?;

    Ok(())
}

pub fn reorder_icons(
    connection: &Connection,
    collection_id: &str,
    icon_ids: Vec<String>,
) -> AppResult<Vec<IconDto>> {
    ensure_collection_exists(connection, collection_id)?;

    let current_ids = active_icon_ids(connection, collection_id)?;
    if current_ids.len() != icon_ids.len() {
        return Err(AppError::new(
            "validation",
            "아이콘 순서 저장 목록이 현재 모음과 일치하지 않습니다.",
        ));
    }

    let current_set: HashSet<&str> = current_ids.iter().map(String::as_str).collect();
    let requested_set: HashSet<&str> = icon_ids.iter().map(String::as_str).collect();
    if current_set != requested_set {
        return Err(AppError::new(
            "validation",
            "다른 모음의 아이콘은 이 순서에 포함할 수 없습니다.",
        ));
    }

    for (order_index, icon_id) in icon_ids.iter().enumerate() {
        connection.execute(
            "UPDATE icons
             SET order_index = ?1,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?2
               AND collection_id = ?3
               AND deleted_at IS NULL",
            params![order_index as i64, icon_id, collection_id],
        )?;
    }

    list_icons(connection, collection_id)
}

pub fn original_path_for_icon(
    connection: &Connection,
    collection_id: &str,
    icon_id: &str,
) -> AppResult<String> {
    connection
        .query_row(
            "SELECT s.original_path_in_library
             FROM source_files s
             JOIN icons i ON i.source_file_id = s.id
             WHERE i.id = ?1
               AND i.collection_id = ?2
               AND i.deleted_at IS NULL",
            params![icon_id, collection_id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("원본 파일을 찾을 수 없습니다."))
}

pub fn export_result_path_for_icon(
    connection: &Connection,
    collection_id: &str,
    icon_id: &str,
) -> AppResult<String> {
    connection
        .query_row(
            "SELECT p.last_export_path
             FROM icon_pieces p
             JOIN icons i ON i.id = p.icon_id
             WHERE i.id = ?1
               AND i.collection_id = ?2
               AND i.deleted_at IS NULL
               AND p.last_export_path IS NOT NULL
             ORDER BY p.piece_index ASC
             LIMIT 1",
            params![icon_id, collection_id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("이 아이콘의 내보내기 결과가 아직 없습니다."))
}

fn icon_from_row(connection: &Connection, row: &Row<'_>) -> rusqlite::Result<IconDto> {
    let icon_id: String = row.get("id")?;

    Ok(IconDto {
        pieces: list_pieces(connection, &icon_id)?,
        id: icon_id,
        collection_id: row.get("collection_id")?,
        source_file_id: row.get("source_file_id")?,
        display_name: row.get("display_name")?,
        note: row.get("note")?,
        icon_kind: row.get("icon_kind")?,
        readiness: row.get("readiness")?,
        placeholder_text: row.get("placeholder_text")?,
        shape: row.get("shape")?,
        order_index: row.get("order_index")?,
        cell_width_override: row.get("cell_width_override")?,
        cell_height_override: row.get("cell_height_override")?,
        thumbnail_url: row.get("thumbnail_path")?,
        thumbnail_override_url: row.get("thumbnail_override_path")?,
        current_preview_url: row.get("current_preview_path")?,
        transform_quarter_turns: row.get("transform_quarter_turns")?,
        transform_flip_horizontal: row.get::<_, i64>("transform_flip_horizontal")? != 0,
        transform_flip_vertical: row.get::<_, i64>("transform_flip_vertical")? != 0,
        gif_loop_mode: row.get("gif_loop_mode")?,
        gif_loop_count: row.get("gif_loop_count")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn list_pieces(connection: &Connection, icon_id: &str) -> rusqlite::Result<Vec<IconPieceDto>> {
    let mut statement = connection.prepare(
        "SELECT
           id,
           icon_id,
           piece_index,
           piece_role,
           alt_text,
           generated_preview_path,
           last_export_path,
           export_status,
           created_at,
           updated_at
         FROM icon_pieces
         WHERE icon_id = ?1
         ORDER BY piece_index ASC",
    )?;

    let pieces = statement
        .query_map(params![icon_id], |row| {
            Ok(IconPieceDto {
                id: row.get("id")?,
                icon_id: row.get("icon_id")?,
                piece_index: row.get("piece_index")?,
                piece_role: row.get("piece_role")?,
                alt_text: row.get("alt_text")?,
                generated_preview_url: row.get("generated_preview_path")?,
                last_export_url: row.get("last_export_path")?,
                export_status: row.get("export_status")?,
                created_at: row.get("created_at")?,
                updated_at: row.get("updated_at")?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(pieces)
}

pub(crate) fn get_icon(
    connection: &Connection,
    collection_id: &str,
    icon_id: &str,
) -> AppResult<IconDto> {
    connection
        .query_row(
            "SELECT
               id,
               collection_id,
               source_file_id,
               display_name,
               (
                 SELECT note
                 FROM icon_notes
                 WHERE icon_id = icons.id
               ) AS note,
               order_index,
               icon_kind,
               readiness,
               placeholder_text,
               shape,
               order_index,
               cell_width_override,
               cell_height_override,
               CASE
                 WHEN current_preview_path IS NULL AND EXISTS (
                   SELECT 1 FROM icon_ai_state preview_state
                   WHERE preview_state.icon_id = icons.id
                     AND preview_state.active_version_id IS NOT NULL
                 ) THEN NULL
                 ELSE thumbnail_path
               END AS thumbnail_path,
               thumbnail_override_path,
               current_preview_path,
               transform_quarter_turns,
               transform_flip_horizontal,
               transform_flip_vertical,
               CASE WHEN gif_pingpong = 1 THEN 'pingpong' ELSE gif_loop_mode END AS gif_loop_mode,
               gif_loop_count,
               created_at,
               updated_at
             FROM icons
             WHERE id = ?1
               AND collection_id = ?2
               AND deleted_at IS NULL",
            params![icon_id, collection_id],
            |row| icon_from_row(connection, row),
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("아이콘을 찾을 수 없습니다."))
}

fn ensure_collection_exists(connection: &Connection, collection_id: &str) -> AppResult<()> {
    let collection_exists = connection
        .query_row(
            "SELECT id
             FROM collections
             WHERE id = ?1
               AND deleted_at IS NULL",
            params![collection_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .is_some();

    if collection_exists {
        Ok(())
    } else {
        Err(AppError::not_found(
            "아이콘을 관리할 모음을 찾을 수 없습니다.",
        ))
    }
}

fn ensure_icon_exists(
    connection: &Connection,
    collection_id: &str,
    icon_id: &str,
) -> AppResult<()> {
    let exists = connection
        .query_row(
            "SELECT id
             FROM icons
             WHERE id = ?1
               AND collection_id = ?2
               AND deleted_at IS NULL",
            params![icon_id, collection_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .is_some();

    if exists {
        Ok(())
    } else {
        Err(AppError::not_found("아이콘을 찾을 수 없습니다."))
    }
}

fn icon_id_for_piece(
    connection: &Connection,
    collection_id: &str,
    piece_id: &str,
) -> AppResult<String> {
    connection
        .query_row(
            "SELECT p.icon_id
             FROM icon_pieces p
             JOIN icons i ON i.id = p.icon_id
             WHERE p.id = ?1
               AND i.collection_id = ?2
               AND i.deleted_at IS NULL",
            params![piece_id, collection_id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("수정할 alt 값을 찾을 수 없습니다."))
}

#[derive(Debug)]
struct IconDuplicateRecord {
    source_file_id: String,
    display_name: String,
    order_index: i64,
    icon_kind: String,
    readiness: String,
    placeholder_text: Option<String>,
    shape: String,
    cell_width_override: Option<i64>,
    cell_height_override: Option<i64>,
    thumbnail_path: Option<String>,
    thumbnail_override_source_file_id: Option<String>,
    thumbnail_override_path: Option<String>,
    text_overlay_enabled: i64,
    text_overlay_text: String,
    text_overlay_font_path: Option<String>,
    text_overlay_font_size: f64,
    text_overlay_x: f64,
    text_overlay_y: f64,
    text_overlay_color: String,
    text_overlay_stroke_color: String,
    text_overlay_stroke_width: f64,
    current_preview_path: Option<String>,
    transform_quarter_turns: i64,
    transform_flip_horizontal: i64,
    transform_flip_vertical: i64,
    gif_loop_mode: String,
    gif_loop_count: Option<i64>,
    gif_pingpong: i64,
}

#[derive(Debug)]
struct CollectionSizingRecord {
    default_cell_width: i64,
    default_cell_height: i64,
}

#[derive(Debug)]
struct IconReplaceRecord {
    display_name: String,
    icon_kind: String,
    original_lineage_generation: i64,
    shape: String,
    cell_width: i64,
    cell_height: i64,
    gif_loop_mode: String,
    gif_loop_count: Option<i64>,
    text_overlay_enabled: bool,
    text_overlay_text: String,
    text_overlay_font_path: Option<String>,
    text_overlay_font_size: f64,
    text_overlay_x: f64,
    text_overlay_y: f64,
    text_overlay_color: String,
    text_overlay_stroke_color: String,
    text_overlay_stroke_width: f64,
}

#[derive(Debug)]
struct IconPieceRecord {
    id: String,
    piece_index: i64,
    piece_role: String,
    alt_text: String,
    generated_preview_path: Option<String>,
}

#[derive(Debug)]
struct CropSettingsRecord {
    crop_mode: String,
    crop_x: f64,
    crop_y: f64,
    crop_w: f64,
    crop_h: f64,
    preset_position: String,
    source_width_at_apply: Option<i64>,
    source_height_at_apply: Option<i64>,
    viewport_width_at_apply: i64,
    viewport_height_at_apply: i64,
}

fn icon_record_for_duplicate(
    connection: &Connection,
    collection_id: &str,
    icon_id: &str,
) -> AppResult<IconDuplicateRecord> {
    connection
        .query_row(
            "SELECT
               source_file_id,
               display_name,
               order_index,
               icon_kind,
               readiness,
               placeholder_text,
               shape,
               cell_width_override,
               cell_height_override,
               text_overlay_enabled,
               text_overlay_text,
               text_overlay_font_path,
               text_overlay_font_size,
               text_overlay_x,
               text_overlay_y,
               text_overlay_color,
               text_overlay_stroke_color,
               text_overlay_stroke_width,
               thumbnail_path,
               thumbnail_override_source_file_id,
               thumbnail_override_path,
               current_preview_path,
               transform_quarter_turns,
               transform_flip_horizontal,
               transform_flip_vertical,
               gif_loop_mode,
               gif_loop_count,
               gif_pingpong
             FROM icons
             WHERE id = ?1
               AND collection_id = ?2
               AND deleted_at IS NULL",
            params![icon_id, collection_id],
            |row| {
                Ok(IconDuplicateRecord {
                    source_file_id: row.get("source_file_id")?,
                    display_name: row.get("display_name")?,
                    order_index: row.get("order_index")?,
                    icon_kind: row.get("icon_kind")?,
                    readiness: row.get("readiness")?,
                    placeholder_text: row.get("placeholder_text")?,
                    shape: row.get("shape")?,
                    cell_width_override: row.get("cell_width_override")?,
                    cell_height_override: row.get("cell_height_override")?,
                    thumbnail_path: row.get("thumbnail_path")?,
                    text_overlay_enabled: row.get("text_overlay_enabled")?,
                    text_overlay_text: row.get("text_overlay_text")?,
                    text_overlay_font_path: row.get("text_overlay_font_path")?,
                    text_overlay_font_size: row.get("text_overlay_font_size")?,
                    text_overlay_x: row.get("text_overlay_x")?,
                    text_overlay_y: row.get("text_overlay_y")?,
                    text_overlay_color: row.get("text_overlay_color")?,
                    text_overlay_stroke_color: row.get("text_overlay_stroke_color")?,
                    text_overlay_stroke_width: row.get("text_overlay_stroke_width")?,
                    thumbnail_override_source_file_id: row
                        .get("thumbnail_override_source_file_id")?,
                    thumbnail_override_path: row.get("thumbnail_override_path")?,
                    current_preview_path: row.get("current_preview_path")?,
                    transform_quarter_turns: row.get("transform_quarter_turns")?,
                    transform_flip_horizontal: row.get("transform_flip_horizontal")?,
                    transform_flip_vertical: row.get("transform_flip_vertical")?,
                    gif_loop_mode: row.get("gif_loop_mode")?,
                    gif_loop_count: row.get("gif_loop_count")?,
                    gif_pingpong: row.get("gif_pingpong")?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("복제할 아이콘을 찾을 수 없습니다."))
}

fn collection_sizing(
    connection: &Connection,
    collection_id: &str,
) -> AppResult<CollectionSizingRecord> {
    connection
        .query_row(
            "SELECT default_cell_width, default_cell_height
             FROM collections
             WHERE id = ?1
               AND deleted_at IS NULL",
            params![collection_id],
            |row| {
                Ok(CollectionSizingRecord {
                    default_cell_width: row.get("default_cell_width")?,
                    default_cell_height: row.get("default_cell_height")?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("아이콘을 추가할 모음을 찾을 수 없습니다."))
}

fn icon_record_for_replace(
    connection: &Connection,
    collection_id: &str,
    icon_id: &str,
) -> AppResult<IconReplaceRecord> {
    connection
        .query_row(
            "SELECT
               i.display_name,
               i.icon_kind,
               i.original_lineage_generation,
               i.shape,
               COALESCE(i.cell_width_override, c.default_cell_width) AS cell_width,
               COALESCE(i.cell_height_override, c.default_cell_height) AS cell_height,
               CASE WHEN i.gif_pingpong = 1 THEN 'pingpong' ELSE i.gif_loop_mode END AS gif_loop_mode,
               i.gif_loop_count,
               i.text_overlay_enabled,
               i.text_overlay_text,
               i.text_overlay_font_path,
               i.text_overlay_font_size,
               i.text_overlay_x,
               i.text_overlay_y,
               i.text_overlay_color,
               i.text_overlay_stroke_color,
               i.text_overlay_stroke_width
             FROM icons i
             JOIN collections c ON c.id = i.collection_id
             WHERE i.id = ?1
               AND i.collection_id = ?2
               AND i.deleted_at IS NULL
               AND c.deleted_at IS NULL",
            params![icon_id, collection_id],
            |row| {
                Ok(IconReplaceRecord {
                    display_name: row.get("display_name")?,
                    icon_kind: row.get("icon_kind")?,
                    original_lineage_generation: row.get("original_lineage_generation")?,
                    shape: row.get("shape")?,
                    cell_width: row.get("cell_width")?,
                    cell_height: row.get("cell_height")?,
                    gif_loop_mode: row.get("gif_loop_mode")?,
                    gif_loop_count: row.get("gif_loop_count")?,
                    text_overlay_enabled: row.get::<_, i64>("text_overlay_enabled")? != 0,
                    text_overlay_text: row.get("text_overlay_text")?,
                    text_overlay_font_path: row.get("text_overlay_font_path")?,
                    text_overlay_font_size: row.get("text_overlay_font_size")?,
                    text_overlay_x: row.get("text_overlay_x")?,
                    text_overlay_y: row.get("text_overlay_y")?,
                    text_overlay_color: row.get("text_overlay_color")?,
                    text_overlay_stroke_color: row.get("text_overlay_stroke_color")?,
                    text_overlay_stroke_width: row.get("text_overlay_stroke_width")?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("이미지를 대체할 아이콘을 찾을 수 없습니다."))
}

fn duplicate_icon_pieces(
    transaction: &Transaction<'_>,
    _paths: &AppPaths,
    _collection_id: &str,
    source_icon_id: &str,
    target_icon_id: &str,
) -> AppResult<HashMap<String, String>> {
    let pieces = {
        let mut statement = transaction.prepare(
            "SELECT
               id,
               piece_index,
               piece_role,
               alt_text,
               generated_preview_path
             FROM icon_pieces
             WHERE icon_id = ?1
             ORDER BY piece_index ASC",
        )?;

        let pieces = statement
            .query_map(params![source_icon_id], |row| {
                Ok(IconPieceRecord {
                    id: row.get("id")?,
                    piece_index: row.get("piece_index")?,
                    piece_role: row.get("piece_role")?,
                    alt_text: row.get("alt_text")?,
                    generated_preview_path: row.get("generated_preview_path")?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        pieces
    };
    let mut piece_id_map = HashMap::new();
    for piece in pieces {
        let target_piece_id = create_id("piece");
        let cloned_preview_path = Option::<String>::None;
        transaction.execute(
            "INSERT INTO icon_pieces (
               id,
               icon_id,
               piece_index,
               piece_role,
               alt_text,
               generated_preview_path,
               last_export_path,
               export_status,
               created_at,
               updated_at
             )
             VALUES (
               ?1,
               ?2,
               ?3,
               ?4,
               ?5,
               ?6,
               ?7,
               ?8,
               strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
               strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             )",
            params![
                &target_piece_id,
                target_icon_id,
                piece.piece_index,
                piece.piece_role,
                piece.alt_text,
                cloned_preview_path,
                Option::<String>::None,
                "not_exported",
            ],
        )?;
        piece_id_map.insert(piece.id, target_piece_id);
    }

    Ok(piece_id_map)
}

fn duplicate_crop_settings(
    transaction: &Transaction<'_>,
    source_icon_id: &str,
    target_icon_id: &str,
) -> AppResult<()> {
    let crop_settings = transaction
        .query_row(
            "SELECT
               crop_mode,
               crop_x,
               crop_y,
               crop_w,
               crop_h,
               preset_position,
               source_width_at_apply,
               source_height_at_apply,
               viewport_width_at_apply,
               viewport_height_at_apply
             FROM crop_settings
             WHERE icon_id = ?1",
            params![source_icon_id],
            |row| {
                Ok(CropSettingsRecord {
                    crop_mode: row.get("crop_mode")?,
                    crop_x: row.get("crop_x")?,
                    crop_y: row.get("crop_y")?,
                    crop_w: row.get("crop_w")?,
                    crop_h: row.get("crop_h")?,
                    preset_position: row.get("preset_position")?,
                    source_width_at_apply: row.get("source_width_at_apply")?,
                    source_height_at_apply: row.get("source_height_at_apply")?,
                    viewport_width_at_apply: row.get("viewport_width_at_apply")?,
                    viewport_height_at_apply: row.get("viewport_height_at_apply")?,
                })
            },
        )
        .optional()?;

    if let Some(crop) = crop_settings {
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
               ?3,
               ?4,
               ?5,
               ?6,
               ?7,
               ?8,
               ?9,
               ?10,
               ?11,
               ?12,
               strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             )",
            params![
                create_id("crop"),
                target_icon_id,
                crop.crop_mode,
                crop.crop_x,
                crop.crop_y,
                crop.crop_w,
                crop.crop_h,
                crop.preset_position,
                crop.source_width_at_apply,
                crop.source_height_at_apply,
                crop.viewport_width_at_apply,
                crop.viewport_height_at_apply,
            ],
        )?;
    }

    Ok(())
}

fn duplicate_icon_note(
    transaction: &Transaction<'_>,
    source_icon_id: &str,
    target_icon_id: &str,
) -> AppResult<()> {
    let note = transaction
        .query_row(
            "SELECT note
             FROM icon_notes
             WHERE icon_id = ?1",
            params![source_icon_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?;

    if let Some(note) = note {
        transaction.execute(
            "INSERT INTO icon_notes (icon_id, note, updated_at)
             VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
            params![target_icon_id, note],
        )?;
    }

    Ok(())
}

fn duplicate_icon_effect_recipe(
    transaction: &Transaction<'_>,
    source_icon_id: &str,
    target_icon_id: &str,
) -> AppResult<()> {
    transaction.execute(
        "INSERT INTO icon_effect_recipes (
           icon_id,
           recipe_schema,
           revision,
           effects_json,
           created_at,
           updated_at
         )
         SELECT
           ?2,
           recipe_schema,
           revision,
           effects_json,
           strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
           strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         FROM icon_effect_recipes
         WHERE icon_id = ?1",
        params![source_icon_id, target_icon_id],
    )?;

    Ok(())
}

fn duplicate_icon_motion_recipe(
    transaction: &Transaction<'_>,
    source_icon_id: &str,
    target_icon_id: &str,
) -> AppResult<()> {
    transaction.execute(
        "INSERT INTO icon_motion_recipes (
           icon_id,
           recipe_schema,
           revision,
           motion_json,
           created_at,
           updated_at
         )
         SELECT
           ?2,
           recipe_schema,
           revision,
           motion_json,
           strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
           strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         FROM icon_motion_recipes
         WHERE icon_id = ?1",
        params![source_icon_id, target_icon_id],
    )?;

    Ok(())
}

fn repair_cover_after_delete(
    transaction: &Transaction<'_>,
    collection_id: &str,
    deleted_ids: &HashSet<&str>,
) -> AppResult<()> {
    let cover_icon_id: Option<String> = transaction.query_row(
        "SELECT cover_icon_id
         FROM collections
         WHERE id = ?1
           AND deleted_at IS NULL",
        params![collection_id],
        |row| row.get(0),
    )?;

    if !cover_icon_id
        .as_deref()
        .is_some_and(|icon_id| deleted_ids.contains(icon_id))
    {
        return Ok(());
    }

    let replacement = transaction
        .query_row(
            "SELECT id, source_file_id
             FROM icons
             WHERE collection_id = ?1
               AND deleted_at IS NULL
             ORDER BY order_index ASC, created_at ASC
             LIMIT 1",
            params![collection_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?;

    if let Some((icon_id, source_file_id)) = replacement {
        transaction.execute(
            "UPDATE collections
             SET cover_icon_id = ?1,
                 cover_source_file_id = ?2,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?3",
            params![icon_id, source_file_id, collection_id],
        )?;
    } else {
        transaction.execute(
            "UPDATE collections
             SET cover_icon_id = NULL,
                 cover_source_file_id = NULL,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?1",
            params![collection_id],
        )?;
    }

    Ok(())
}

fn compact_icon_order(transaction: &Transaction<'_>, collection_id: &str) -> AppResult<()> {
    let icon_ids = active_icon_ids(transaction, collection_id)?;
    for (order_index, icon_id) in icon_ids.iter().enumerate() {
        transaction.execute(
            "UPDATE icons
             SET order_index = ?1
             WHERE id = ?2",
            params![order_index as i64, icon_id],
        )?;
    }

    Ok(())
}

fn active_icon_ids(connection: &Connection, collection_id: &str) -> AppResult<Vec<String>> {
    let mut statement = connection.prepare(
        "SELECT id
         FROM icons
         WHERE collection_id = ?1
           AND deleted_at IS NULL
         ORDER BY order_index ASC, created_at ASC",
    )?;

    let ids = statement
        .query_map(params![collection_id], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(ids)
}

fn next_icon_order_index(connection: &Connection, collection_id: &str) -> AppResult<i64> {
    Ok(connection.query_row(
        "SELECT COALESCE(MAX(order_index) + 1, 0)
         FROM icons
         WHERE collection_id = ?1
           AND deleted_at IS NULL",
        params![collection_id],
        |row| row.get(0),
    )?)
}

#[derive(Debug, Clone, Copy)]
struct CropRect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

fn insert_crop_settings(
    transaction: &Transaction<'_>,
    icon_id: &str,
    crop: CropRect,
    source_width: i64,
    source_height: i64,
    viewport_width: i64,
    viewport_height: i64,
) -> AppResult<()> {
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
           ?3,
           ?4,
           ?5,
           ?6,
           'center',
           ?7,
           ?8,
           ?9,
           ?10,
           strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         )",
        params![
            create_id("crop"),
            icon_id,
            crop.x,
            crop.y,
            crop.width,
            crop.height,
            source_width,
            source_height,
            viewport_width,
            viewport_height,
        ],
    )?;

    Ok(())
}

fn replace_crop_settings(
    transaction: &Transaction<'_>,
    icon_id: &str,
    crop: CropRect,
    source_width: i64,
    source_height: i64,
    viewport_width: i64,
    viewport_height: i64,
) -> AppResult<()> {
    let changed = transaction.execute(
        "UPDATE crop_settings
         SET crop_mode = 'free',
             crop_x = ?1,
             crop_y = ?2,
             crop_w = ?3,
             crop_h = ?4,
             preset_position = 'center',
             source_width_at_apply = ?5,
             source_height_at_apply = ?6,
             viewport_width_at_apply = ?7,
             viewport_height_at_apply = ?8,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE icon_id = ?9",
        params![
            crop.x,
            crop.y,
            crop.width,
            crop.height,
            source_width,
            source_height,
            viewport_width,
            viewport_height,
            icon_id,
        ],
    )?;

    if changed == 0 {
        insert_crop_settings(
            transaction,
            icon_id,
            crop,
            source_width,
            source_height,
            viewport_width,
            viewport_height,
        )?;
    }

    Ok(())
}

fn insert_piece(
    transaction: &Transaction<'_>,
    icon_id: &str,
    piece_index: i64,
    piece_role: &str,
    alt_text: &str,
) -> AppResult<()> {
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
           ?3,
           ?4,
           ?5,
           strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
           strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         )",
        params![
            create_id("piece"),
            icon_id,
            piece_index,
            piece_role,
            alt_text
        ],
    )?;

    Ok(())
}

fn centered_crop_rect(
    source_width: i64,
    source_height: i64,
    cell_width: i64,
    cell_height: i64,
) -> CropRect {
    let source_width = source_width.max(1) as f64;
    let source_height = source_height.max(1) as f64;
    let target_aspect = cell_width.max(1) as f64 / cell_height.max(1) as f64;
    let source_aspect = source_width / source_height;

    let (crop_width, crop_height) = if source_aspect > target_aspect {
        (source_height * target_aspect, source_height)
    } else {
        (source_width, source_width / target_aspect)
    };

    CropRect {
        x: ((source_width - crop_width) / 2.0).max(0.0),
        y: ((source_height - crop_height) / 2.0).max(0.0),
        width: crop_width.min(source_width),
        height: crop_height.min(source_height),
    }
}

fn transparent_png_bytes(width: u32, height: u32) -> AppResult<Vec<u8>> {
    let image = ImageBuffer::from_pixel(width.max(1), height.max(1), Rgba([0, 0, 0, 0]));
    let mut cursor = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image)
        .write_to(&mut cursor, ImageFormat::Png)
        .map_err(AppError::from)?;
    Ok(cursor.into_inner())
}

fn normalized_alt_text(alt_text: String) -> String {
    alt_text.trim().to_string()
}

fn normalized_note(note: String) -> String {
    note.trim().to_string()
}

fn normalized_display_name(display_name: String) -> String {
    let trimmed = display_name.trim();
    if trimmed.is_empty() {
        "이름 없는 아이콘".to_string()
    } else {
        trimmed.to_string()
    }
}

fn normalized_placeholder_label(label: String) -> String {
    let trimmed = label.trim();
    if trimmed.is_empty() {
        "빈 디시콘".to_string()
    } else {
        trimmed.chars().take(40).collect()
    }
}

fn normalized_readiness(readiness: String) -> AppResult<String> {
    match readiness.as_str() {
        "complete" | "working" => Ok(readiness),
        _ => Err(AppError::new(
            "validation",
            "지원하지 않는 아이콘 상태입니다.",
        )),
    }
}

fn display_name_from_filename(filename: &str) -> String {
    std::path::Path::new(filename)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(str::trim)
        .filter(|stem| !stem.is_empty())
        .unwrap_or("새 아이콘")
        .to_string()
}

fn rebase_preview_artifact(path: &Path, from: &Path, to: &Path) -> AppResult<PathBuf> {
    let relative = path.strip_prefix(from).map_err(|_| {
        AppError::new(
            "source_replace_preview_path",
            "원본 교체 미리보기 경로가 staging 디렉터리를 벗어났습니다.",
        )
    })?;
    Ok(to.join(relative))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::fs;
    use std::io::Cursor;
    use std::time::{SystemTime, UNIX_EPOCH};

    use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};
    use rusqlite::{params, Connection};
    use sha2::{Digest, Sha256};

    use crate::db::migrations;
    use crate::db::repositories::ai::{activate_ai_candidate, import_local_ai_candidate};
    use crate::db::repositories::collections::create_collection;
    use crate::db::repositories::imports::import_image_files;
    use crate::ids::create_id;
    use crate::models::{
        ActivateAiCandidatePayload, CreatePlaceholderIconPayload, IconDto,
        ImportAiCandidatePayload, ImportImageFilePayload,
    };
    use crate::paths::AppPaths;

    use super::{
        create_placeholder_icon, delete_icons, duplicate_icon, list_icons, rename_icon,
        reorder_icons, replace_icon_source, set_icon_thumbnail_override, set_icons_readiness,
        update_icon_note, update_icon_piece_alt,
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
        AppPaths::prepare(std::env::temp_dir().join(format!("pmtconcon-icons-{suffix}"))).unwrap()
    }

    fn png_bytes() -> Vec<u8> {
        png_bytes_with_color([0, 0, 255, 255])
    }

    fn png_bytes_with_color(color: [u8; 4]) -> Vec<u8> {
        let image = ImageBuffer::from_pixel(20, 20, Rgba(color));
        let mut cursor = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image)
            .write_to(&mut cursor, ImageFormat::Png)
            .unwrap();
        cursor.into_inner()
    }

    fn import_test_icon(
        connection: &mut Connection,
        paths: &AppPaths,
        collection_id: &str,
    ) -> IconDto {
        import_image_files(
            connection,
            paths,
            collection_id,
            vec![ImportImageFilePayload {
                original_filename: "original.png".to_string(),
                bytes: png_bytes_with_color([0, 40, 255, 255]),
            }],
        )
        .unwrap()
        .imported_icons
        .into_iter()
        .next()
        .unwrap()
    }

    fn import_and_activate_ai_candidate(
        connection: &mut Connection,
        paths: &AppPaths,
        collection_id: &str,
        icon_id: &str,
        color: [u8; 4],
    ) -> String {
        let review = import_local_ai_candidate(
            connection,
            paths,
            collection_id,
            ImportAiCandidatePayload {
                icon_id: icon_id.to_string(),
                service_surface: "gemini_web".to_string(),
                file: ImportImageFilePayload {
                    original_filename: format!("candidate-{}.png", color[0]),
                    bytes: png_bytes_with_color(color),
                },
            },
        )
        .unwrap();
        let candidate_id = review
            .candidates
            .iter()
            .find(|candidate| !candidate.is_materialized)
            .unwrap()
            .id
            .clone();
        activate_ai_candidate(
            connection,
            paths,
            collection_id,
            ActivateAiCandidatePayload {
                icon_id: icon_id.to_string(),
                candidate_id: candidate_id.clone(),
                expected_revision: review.visual_source.activation_revision,
                normalization: Default::default(),
                expected_preview_signature: None,
            },
        )
        .unwrap();
        candidate_id
    }

    fn seed_icon(
        connection: &Connection,
        collection_id: &str,
        order_index: i64,
        alt_text: &str,
    ) -> (String, String) {
        let source_file_id = create_id("source");
        let icon_id = create_id("icon");
        let piece_id = create_id("piece");
        let source_color = u8::try_from(order_index.rem_euclid(256)).unwrap();
        let source_bytes = png_bytes_with_color([source_color, 40, 60, 255]);
        let source_path = std::env::temp_dir().join(format!("pmtconcon-seed-{icon_id}.png"));
        fs::write(&source_path, &source_bytes).unwrap();
        let source_sha256 = format!("{:x}", Sha256::digest(&source_bytes));
        let source_byte_size = i64::try_from(source_bytes.len()).unwrap();

        connection
            .execute(
                "INSERT INTO source_files (
                   id,
                   original_filename,
                   original_path_in_library,
                   original_extension,
                   mime_type,
                   width,
                   height,
                   byte_size,
                   sha256,
                   created_at
                 )
                 VALUES (
                   ?1,
                   ?2,
                   ?3,
                   'png',
                   'image/png',
                   20,
                   20,
                   ?4,
                   ?5,
                   strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 )",
                params![
                    source_file_id,
                    format!("{icon_id}.png"),
                    source_path.to_string_lossy(),
                    source_byte_size,
                    source_sha256,
                ],
            )
            .unwrap();

        connection
            .execute(
                "INSERT INTO icons (
                   id,
                   collection_id,
                   source_file_id,
                   display_name,
                   icon_kind,
                   readiness,
                   placeholder_text,
                   shape,
                   order_index,
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
                   NULL,
                   'single',
                   ?5,
                   strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                   strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 )",
                params![
                    icon_id,
                    collection_id,
                    source_file_id,
                    format!("아이콘 {order_index}"),
                    order_index,
                ],
            )
            .unwrap();

        connection
            .execute(
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
                params![piece_id, icon_id, alt_text],
            )
            .unwrap();

        connection
            .execute(
                "INSERT INTO crop_settings (
                   id, icon_id, crop_mode, crop_x, crop_y, crop_w, crop_h,
                   preset_position, source_width_at_apply, source_height_at_apply,
                   viewport_width_at_apply, viewport_height_at_apply, updated_at
                 ) VALUES (
                   ?1, ?2, 'free', 0, 0, 20, 20, 'center', 20, 20, 200, 200,
                   strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 )",
                params![create_id("crop"), icon_id],
            )
            .unwrap();

        (icon_id, piece_id)
    }

    #[test]
    fn update_icon_piece_alt_accepts_warning_values_and_duplicates() {
        let mut connection = connection();
        let collection =
            create_collection(&mut connection, Some("alt 테스트".to_string())).unwrap();
        let (_first_icon_id, _first_piece_id) = seed_icon(&connection, &collection.id, 0, "가");
        let (_second_icon_id, second_piece_id) = seed_icon(&connection, &collection.id, 1, "나");

        let long_alt = update_icon_piece_alt(
            &connection,
            &collection.id,
            &second_piece_id,
            "가나다라".to_string(),
        )
        .unwrap();
        assert_eq!(long_alt.pieces[0].alt_text, "가나다라");

        let updated = update_icon_piece_alt(
            &connection,
            &collection.id,
            &second_piece_id,
            "가".to_string(),
        )
        .unwrap();
        assert_eq!(updated.pieces[0].alt_text, "가");
    }

    #[test]
    fn rename_icon_updates_display_name_without_changing_alt_text() {
        let mut connection = connection();
        let collection =
            create_collection(&mut connection, Some("아이콘명 테스트".to_string())).unwrap();
        let (icon_id, _) = seed_icon(&connection, &collection.id, 0, "가");

        let updated = rename_icon(
            &connection,
            &collection.id,
            &icon_id,
            "새 아이콘".to_string(),
        )
        .unwrap();

        assert_eq!(updated.display_name, "새 아이콘");
        assert_eq!(updated.pieces[0].alt_text, "가");
    }

    #[test]
    fn set_icon_thumbnail_override_persists_without_replacing_export_source() {
        let mut connection = connection();
        let paths = temp_paths();
        let collection =
            create_collection(&mut connection, Some("썸네일 테스트".to_string())).unwrap();
        let (icon_id, _) = seed_icon(&connection, &collection.id, 0, "가");
        let original_source_file_id: String = connection
            .query_row(
                "SELECT source_file_id FROM icons WHERE id = ?1",
                [&icon_id],
                |row| row.get(0),
            )
            .unwrap();

        let updated = set_icon_thumbnail_override(
            &mut connection,
            &paths,
            &collection.id,
            &icon_id,
            ImportImageFilePayload {
                original_filename: "thumb.png".to_string(),
                bytes: png_bytes(),
            },
        )
        .unwrap();

        assert!(updated.thumbnail_override_url.is_some());
        assert!(std::path::Path::new(updated.thumbnail_override_url.as_ref().unwrap()).exists());
        let source_file_id_after: String = connection
            .query_row(
                "SELECT source_file_id FROM icons WHERE id = ?1",
                [&icon_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(source_file_id_after, original_source_file_id);

        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn placeholder_icons_are_working_until_replaced_and_marked_complete() {
        let mut connection = connection();
        let paths = temp_paths();
        let collection =
            create_collection(&mut connection, Some("빈 디시콘 테스트".to_string())).unwrap();

        let placeholder = create_placeholder_icon(
            &mut connection,
            &paths,
            &collection.id,
            CreatePlaceholderIconPayload {
                label: "울음".to_string(),
            },
        )
        .unwrap();

        assert_eq!(placeholder.icon_kind, "placeholder");
        assert_eq!(placeholder.readiness, "working");
        assert_eq!(placeholder.placeholder_text.as_deref(), Some("울음"));
        connection
            .execute(
                "UPDATE icons
                 SET transform_quarter_turns = 1,
                     transform_flip_horizontal = 1
                 WHERE id = ?1",
                [&placeholder.id],
            )
            .unwrap();
        connection
            .execute(
                "UPDATE icon_pieces
                 SET generated_preview_path = 'C:/stale/old-piece.png',
                     last_export_path = 'C:/stale/old-export.png',
                     export_status = 'warning'
                 WHERE icon_id = ?1",
                [&placeholder.id],
            )
            .unwrap();

        let replaced = replace_icon_source(
            &mut connection,
            &paths,
            &collection.id,
            &placeholder.id,
            ImportImageFilePayload {
                original_filename: "cry.png".to_string(),
                bytes: png_bytes(),
            },
        )
        .unwrap();

        assert_eq!(replaced.icon_kind, "image");
        assert_eq!(replaced.readiness, "working");
        assert_eq!(replaced.placeholder_text, None);
        assert_eq!(replaced.display_name, "cry");
        assert_eq!(replaced.transform_quarter_turns, 0);
        assert!(!replaced.transform_flip_horizontal);
        assert!(!replaced.transform_flip_vertical);
        assert!(std::path::Path::new(replaced.current_preview_url.as_ref().unwrap()).is_file());
        assert_ne!(
            replaced.pieces[0].generated_preview_url.as_deref(),
            Some("C:/stale/old-piece.png")
        );
        assert!(
            std::path::Path::new(replaced.pieces[0].generated_preview_url.as_ref().unwrap())
                .is_file()
        );
        assert!(replaced.pieces[0].last_export_url.is_none());

        let icons = set_icons_readiness(
            &connection,
            &collection.id,
            vec![replaced.id],
            "complete".to_string(),
        )
        .unwrap();
        assert_eq!(icons[0].readiness, "complete");

        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn replacing_double_icon_source_regenerates_piece_previews_and_composite_crop_metadata() {
        let mut connection = connection();
        let paths = temp_paths();
        let collection =
            create_collection(&mut connection, Some("2칸 교체 테스트".to_string())).unwrap();
        let (icon_id, first_piece_id) = seed_icon(&connection, &collection.id, 0, "왼");
        connection
            .execute(
                "UPDATE icons
                 SET shape = 'horizontal_double',
                     cell_width_override = 12,
                     cell_height_override = 8,
                     transform_quarter_turns = 1
                 WHERE id = ?1",
                [&icon_id],
            )
            .unwrap();
        connection
            .execute(
                "UPDATE icon_pieces
                 SET piece_role = 'left'
                 WHERE id = ?1",
                [&first_piece_id],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO icon_pieces (
                   id, icon_id, piece_index, piece_role, alt_text, created_at, updated_at
                 )
                 VALUES (
                   ?1, ?2, 1, 'right', '오른',
                   strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                   strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 )",
                params![create_id("piece"), icon_id],
            )
            .unwrap();

        let replaced = replace_icon_source(
            &mut connection,
            &paths,
            &collection.id,
            &icon_id,
            ImportImageFilePayload {
                original_filename: "double.png".to_string(),
                bytes: png_bytes(),
            },
        )
        .unwrap();

        assert_eq!(replaced.shape, "horizontal_double");
        assert_eq!(replaced.transform_quarter_turns, 0);
        assert_eq!(replaced.pieces.len(), 2);
        assert!(replaced.pieces.iter().all(|piece| {
            piece
                .generated_preview_url
                .as_deref()
                .is_some_and(|path| std::path::Path::new(path).is_file())
        }));
        let viewport_at_apply: (i64, i64) = connection
            .query_row(
                "SELECT viewport_width_at_apply, viewport_height_at_apply
                 FROM crop_settings
                 WHERE icon_id = ?1",
                [&icon_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(viewport_at_apply, (24, 8));

        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn reorder_icons_persists_order_indexes() {
        let mut connection = connection();
        let collection =
            create_collection(&mut connection, Some("순서 테스트".to_string())).unwrap();
        let (first_icon_id, _) = seed_icon(&connection, &collection.id, 0, "가");
        let (second_icon_id, _) = seed_icon(&connection, &collection.id, 1, "나");
        let (third_icon_id, _) = seed_icon(&connection, &collection.id, 2, "다");

        let icons = reorder_icons(
            &connection,
            &collection.id,
            vec![
                third_icon_id.clone(),
                first_icon_id.clone(),
                second_icon_id.clone(),
            ],
        )
        .unwrap();

        let ordered_ids: Vec<String> = icons.into_iter().map(|icon| icon.id).collect();
        assert_eq!(
            ordered_ids,
            vec![third_icon_id, first_icon_id, second_icon_id]
        );
    }

    #[test]
    fn duplicate_icon_creates_new_order_and_preserves_alt_warning_state() {
        let mut connection = connection();
        let paths = temp_paths();
        let collection =
            create_collection(&mut connection, Some("복제 테스트".to_string())).unwrap();
        let (icon_id, _) = seed_icon(&connection, &collection.id, 0, "가");

        connection
            .execute(
                "UPDATE icons
                 SET transform_quarter_turns = 3,
                     transform_flip_horizontal = 1
                 WHERE id = ?1",
                [&icon_id],
            )
            .unwrap();

        let duplicated = duplicate_icon(&mut connection, &paths, &collection.id, &icon_id).unwrap();

        assert_ne!(duplicated.id, icon_id);
        assert_eq!(duplicated.order_index, 1);
        assert_eq!(duplicated.transform_quarter_turns, 3);
        assert!(duplicated.transform_flip_horizontal);
        assert!(!duplicated.transform_flip_vertical);
        assert_eq!(duplicated.pieces[0].alt_text, "가");
        assert_eq!(list_icons(&connection, &collection.id).unwrap().len(), 2);
        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn icon_note_persists_and_whitespace_clears() {
        let mut connection = connection();
        let collection = create_collection(&mut connection, Some("memo test".to_string())).unwrap();
        let (icon_id, _) = seed_icon(&connection, &collection.id, 0, "ga");

        let updated = update_icon_note(
            &connection,
            &collection.id,
            &icon_id,
            "  작업 메모\n두 번째 줄  ".to_string(),
        )
        .unwrap();
        assert_eq!(updated.note.as_deref(), Some("작업 메모\n두 번째 줄"));
        assert_eq!(
            list_icons(&connection, &collection.id).unwrap()[0]
                .note
                .as_deref(),
            Some("작업 메모\n두 번째 줄")
        );

        let cleared =
            update_icon_note(&connection, &collection.id, &icon_id, "   ".to_string()).unwrap();
        assert_eq!(cleared.note, None);
    }

    #[test]
    fn duplicate_icon_copies_note_without_mutating_source() {
        let mut connection = connection();
        let paths = temp_paths();
        let collection =
            create_collection(&mut connection, Some("memo duplicate test".to_string())).unwrap();
        let (icon_id, _) = seed_icon(&connection, &collection.id, 0, "ga");
        update_icon_note(
            &connection,
            &collection.id,
            &icon_id,
            "원본 메모".to_string(),
        )
        .unwrap();

        let duplicated = duplicate_icon(&mut connection, &paths, &collection.id, &icon_id).unwrap();

        assert_eq!(duplicated.note.as_deref(), Some("원본 메모"));
        let original = list_icons(&connection, &collection.id)
            .unwrap()
            .into_iter()
            .find(|icon| icon.id == icon_id)
            .unwrap();
        assert_eq!(original.note.as_deref(), Some("원본 메모"));
        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn duplicate_icon_copies_effect_and_motion_recipes_independently() {
        let mut connection = connection();
        let paths = temp_paths();
        let collection =
            create_collection(&mut connection, Some("effect duplicate test".to_string())).unwrap();
        let (icon_id, _) = seed_icon(&connection, &collection.id, 0, "ga");
        let effects_json = r#"{"version":1,"effects":[{"kind":"pixelate","id":"pixel","enabled":true,"blockSize":6}]}"#;
        connection
            .execute(
                "INSERT INTO icon_effect_recipes (
                   icon_id, recipe_schema, revision, effects_json, created_at, updated_at
                 )
                 VALUES (
                   ?1, 'pmtcon-effects-v1', 3, ?2,
                   strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                   strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 )",
                params![icon_id, effects_json],
            )
            .unwrap();
        let motion_json = r#"{"version":1,"durationMs":1000,"fps":20,"seed":4242,"interpolation":"bilinear","edgeMode":"transparent","spatial":{"kind":"shake","enabled":true,"cyclesPerLoop":1,"amplitudeX":2,"amplitudeY":1},"displacement":null,"colorOpacity":null,"overlay":null}"#;
        connection
            .execute(
                "INSERT INTO icon_motion_recipes (
                   icon_id, recipe_schema, revision, motion_json, created_at, updated_at
                 )
                 VALUES (
                   ?1, 'pmtcon-motion-v1', 5, ?2,
                   strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                   strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 )",
                params![icon_id, motion_json],
            )
            .unwrap();

        let duplicated = duplicate_icon(&mut connection, &paths, &collection.id, &icon_id).unwrap();
        let original_recipe: (String, i64, String) = connection
            .query_row(
                "SELECT recipe_schema, revision, effects_json
                 FROM icon_effect_recipes
                 WHERE icon_id = ?1",
                [&icon_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        let duplicated_recipe: (String, i64, String) = connection
            .query_row(
                "SELECT recipe_schema, revision, effects_json
                 FROM icon_effect_recipes
                 WHERE icon_id = ?1",
                [&duplicated.id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();

        assert_eq!(duplicated_recipe, original_recipe);
        let original_motion: (String, i64, String) = connection
            .query_row(
                "SELECT recipe_schema, revision, motion_json
                 FROM icon_motion_recipes WHERE icon_id = ?1",
                [&icon_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        let duplicated_motion: (String, i64, String) = connection
            .query_row(
                "SELECT recipe_schema, revision, motion_json
                 FROM icon_motion_recipes WHERE icon_id = ?1",
                [&duplicated.id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(duplicated_motion, original_motion);
        assert!(original_motion.2.contains("\"seed\":4242"));

        let changed_motion_json = motion_json.replace("\"seed\":4242", "\"seed\":7");
        connection
            .execute(
                "UPDATE icon_motion_recipes
                 SET revision = 6, motion_json = ?1
                 WHERE icon_id = ?2",
                params![changed_motion_json, duplicated.id],
            )
            .unwrap();
        let source_after: (i64, String) = connection
            .query_row(
                "SELECT revision, motion_json FROM icon_motion_recipes WHERE icon_id = ?1",
                [&icon_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        let duplicate_after: (i64, String) = connection
            .query_row(
                "SELECT revision, motion_json FROM icon_motion_recipes WHERE icon_id = ?1",
                [&duplicated.id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(source_after, (5, motion_json.to_string()));
        assert_eq!(duplicate_after.0, 6);
        assert!(duplicate_after.1.contains("\"seed\":7"));
        for table in ["processed_asset_variants", "optimization_jobs"] {
            let count: i64 = connection
                .query_row(
                    &format!("SELECT COUNT(*) FROM {table} WHERE icon_id = ?1"),
                    [&duplicated.id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 0);
        }
        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn duplicate_icon_owns_preview_files_and_preserves_render_state() {
        let mut connection = connection();
        let paths = temp_paths();
        let collection =
            create_collection(&mut connection, Some("render clone test".to_string())).unwrap();
        let (icon_id, piece_id) = seed_icon(&connection, &collection.id, 0, "ga");
        let source_dir = paths
            .collection_previews_dir
            .join(&collection.id)
            .join(&icon_id)
            .join("effects")
            .join("source-artifact");
        fs::create_dir_all(&source_dir).unwrap();
        let source_preview = source_dir.join("preview.png");
        let source_piece = source_dir.join("piece-00.png");
        fs::write(&source_preview, png_bytes()).unwrap();
        fs::write(&source_piece, png_bytes()).unwrap();
        let old_export = paths.root.join("old-export.png");

        connection
            .execute(
                "UPDATE icons
                 SET current_preview_path = ?1,
                     text_overlay_enabled = 1,
                     text_overlay_text = 'copy text',
                     text_overlay_font_path = NULL,
                     text_overlay_font_size = 24.0,
                     text_overlay_x = 0.25,
                     text_overlay_y = 0.75,
                     text_overlay_color = '#12345678',
                     text_overlay_stroke_color = '#ABCDEF',
                     text_overlay_stroke_width = 2.5,
                     transform_quarter_turns = 3,
                     transform_flip_horizontal = 1,
                     transform_flip_vertical = 1
                 WHERE id = ?2",
                params![source_preview.to_string_lossy(), icon_id],
            )
            .unwrap();
        connection
            .execute(
                "UPDATE icon_pieces
                 SET generated_preview_path = ?1,
                     last_export_path = ?2,
                     export_status = 'ready'
                 WHERE id = ?3",
                params![
                    source_piece.to_string_lossy(),
                    old_export.to_string_lossy(),
                    piece_id
                ],
            )
            .unwrap();
        let effects_json = r#"{"version":1,"effects":[{"kind":"pixelate","id":"pixel","enabled":true,"blockSize":6}]}"#;
        connection
            .execute(
                "INSERT INTO icon_effect_recipes (
                   icon_id, recipe_schema, revision, effects_json, created_at, updated_at
                 )
                 VALUES (
                   ?1, 'pmtcon-effects-v1', 4, ?2,
                   strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                   strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 )",
                params![icon_id, effects_json],
            )
            .unwrap();

        let duplicated = duplicate_icon(&mut connection, &paths, &collection.id, &icon_id).unwrap();
        let duplicate_preview = duplicated.current_preview_url.clone().unwrap();
        let (duplicate_piece, last_export, export_status): (String, Option<String>, String) =
            connection
                .query_row(
                    "SELECT generated_preview_path, last_export_path, export_status
                     FROM icon_pieces
                     WHERE icon_id = ?1 AND piece_index = 0",
                    [&duplicated.id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .unwrap();
        assert_ne!(duplicate_preview, source_preview.to_string_lossy());
        assert_ne!(duplicate_piece, source_piece.to_string_lossy());
        let native_root = paths
            .ai_activation_previews_dir
            .join(&collection.id)
            .join(&duplicated.id)
            .join("native-clone");
        assert!(std::path::Path::new(&duplicate_preview).starts_with(&native_root));
        assert!(std::path::Path::new(&duplicate_piece).starts_with(&native_root));
        assert_eq!(
            image::image_dimensions(&duplicate_preview).unwrap(),
            (200, 200)
        );
        assert_eq!(
            image::image_dimensions(&duplicate_piece).unwrap(),
            (200, 200)
        );
        assert_eq!(last_export, None);
        assert_eq!(export_status, "ready");

        let source_text_state: (
            i64,
            String,
            Option<String>,
            f64,
            f64,
            f64,
            String,
            String,
            f64,
        ) = connection
            .query_row(
                "SELECT text_overlay_enabled, text_overlay_text, text_overlay_font_path,
                            text_overlay_font_size, text_overlay_x, text_overlay_y,
                            text_overlay_color, text_overlay_stroke_color,
                            text_overlay_stroke_width
                     FROM icons WHERE id = ?1",
                [&icon_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                        row.get(8)?,
                    ))
                },
            )
            .unwrap();
        let duplicate_text_state = connection
            .query_row(
                "SELECT text_overlay_enabled, text_overlay_text, text_overlay_font_path,
                        text_overlay_font_size, text_overlay_x, text_overlay_y,
                        text_overlay_color, text_overlay_stroke_color,
                        text_overlay_stroke_width
                 FROM icons WHERE id = ?1",
                [&duplicated.id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                        row.get(8)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(duplicate_text_state, source_text_state);
        assert_eq!(duplicated.transform_quarter_turns, 3);
        assert!(duplicated.transform_flip_horizontal);
        assert!(duplicated.transform_flip_vertical);
        let duplicated_recipe: (String, i64, String) = connection
            .query_row(
                "SELECT recipe_schema, revision, effects_json
                 FROM icon_effect_recipes WHERE icon_id = ?1",
                [&duplicated.id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(duplicated_recipe.1, 4);
        assert_eq!(duplicated_recipe.2, effects_json);
        for table in ["processed_asset_variants", "optimization_jobs"] {
            let count: i64 = connection
                .query_row(
                    &format!("SELECT COUNT(*) FROM {table} WHERE icon_id = ?1"),
                    [&duplicated.id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 0);
        }

        fs::remove_dir_all(&source_dir).unwrap();
        assert!(std::path::Path::new(&duplicate_preview).is_file());
        assert!(std::path::Path::new(&duplicate_piece).is_file());
        fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn duplicate_icon_is_inserted_next_to_source_icon() {
        let mut connection = connection();
        let paths = temp_paths();
        let collection =
            create_collection(&mut connection, Some("duplicate order test".to_string())).unwrap();
        let (first_icon_id, _) = seed_icon(&connection, &collection.id, 0, "ga");
        let (second_icon_id, _) = seed_icon(&connection, &collection.id, 1, "na");
        let (third_icon_id, _) = seed_icon(&connection, &collection.id, 2, "da");

        let duplicated =
            duplicate_icon(&mut connection, &paths, &collection.id, &second_icon_id).unwrap();
        let icons = list_icons(&connection, &collection.id).unwrap();
        let ordered_ids = icons.iter().map(|icon| icon.id.clone()).collect::<Vec<_>>();
        let ordered_indexes = icons
            .iter()
            .map(|icon| icon.order_index)
            .collect::<Vec<_>>();

        assert_eq!(
            ordered_ids,
            vec![first_icon_id, second_icon_id, duplicated.id, third_icon_id]
        );
        assert_eq!(ordered_indexes, vec![0, 1, 2, 3]);
        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn delete_icons_soft_deletes_and_repairs_cover() {
        let mut connection = connection();
        let collection =
            create_collection(&mut connection, Some("삭제 테스트".to_string())).unwrap();
        let (first_icon_id, _) = seed_icon(&connection, &collection.id, 0, "가");
        let (second_icon_id, _) = seed_icon(&connection, &collection.id, 1, "나");

        connection
            .execute(
                "UPDATE collections
                 SET cover_icon_id = ?1,
                     cover_source_file_id = (
                       SELECT source_file_id FROM icons WHERE id = ?1
                     )
                 WHERE id = ?2",
                params![first_icon_id, collection.id],
            )
            .unwrap();

        delete_icons(&mut connection, &collection.id, vec![first_icon_id]).unwrap();

        let icons = list_icons(&connection, &collection.id).unwrap();
        assert_eq!(icons.len(), 1);
        assert_eq!(icons[0].id, second_icon_id);

        let cover_icon_id: Option<String> = connection
            .query_row(
                "SELECT cover_icon_id FROM collections WHERE id = ?1",
                [&collection.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(cover_icon_id, Some(second_icon_id));
    }

    #[test]
    fn replacing_source_advances_ai_lineage_and_supersedes_pending_work() {
        let mut connection = connection();
        let paths = temp_paths();
        let collection =
            create_collection(&mut connection, Some("AI 원본 교체 테스트".to_string())).unwrap();
        let icon = import_test_icon(&mut connection, &paths, &collection.id);
        let candidate_id = import_and_activate_ai_candidate(
            &mut connection,
            &paths,
            &collection.id,
            &icon.id,
            [30, 220, 90, 255],
        );
        let request_id: String = connection
            .query_row(
                "SELECT request_id FROM ai_candidates WHERE id = ?1",
                [&candidate_id],
                |row| row.get(0),
            )
            .unwrap();
        connection
            .execute(
                "UPDATE ai_requests
                 SET status = 'awaiting_result', completed_at = NULL
                 WHERE id = ?1",
                [&request_id],
            )
            .unwrap();
        let before: (String, String, i64, Option<String>, i64) = connection
            .query_row(
                "SELECT i.source_file_id, i.original_lineage_id,
                        i.original_lineage_generation, st.active_version_id, st.revision
                 FROM icons i JOIN icon_ai_state st ON st.icon_id = i.id
                 WHERE i.id = ?1",
                [&icon.id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .unwrap();
        assert!(before.3.is_some());

        replace_icon_source(
            &mut connection,
            &paths,
            &collection.id,
            &icon.id,
            ImportImageFilePayload {
                original_filename: "replacement.png".to_string(),
                bytes: png_bytes_with_color([240, 50, 30, 255]),
            },
        )
        .unwrap();

        let after: (String, String, i64, Option<String>, i64) = connection
            .query_row(
                "SELECT i.source_file_id, i.original_lineage_id,
                        i.original_lineage_generation, st.active_version_id, st.revision
                 FROM icons i JOIN icon_ai_state st ON st.icon_id = i.id
                 WHERE i.id = ?1",
                [&icon.id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .unwrap();
        assert_ne!(after.0, before.0);
        assert_ne!(after.1, before.1);
        assert_eq!(after.2, before.2 + 1);
        assert_eq!(after.3, None);
        assert_eq!(after.4, before.4 + 1);
        let request_state: (String, Option<String>, Option<String>) = connection
            .query_row(
                "SELECT status, superseded_at, superseded_reason
                 FROM ai_requests WHERE id = ?1",
                [&request_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(request_state.0, "cancelled");
        assert!(request_state.1.is_some());
        assert_eq!(request_state.2.as_deref(), Some("original_source_replaced"));
        let cover_source: Option<String> = connection
            .query_row(
                "SELECT cover_source_file_id FROM collections WHERE id = ?1",
                [&collection.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(cover_source.as_deref(), Some(after.0.as_str()));
        fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn duplicate_icon_remaps_ai_version_dag_without_copying_requests() {
        let mut connection = connection();
        let paths = temp_paths();
        let collection =
            create_collection(&mut connection, Some("AI 계보 복제 테스트".to_string())).unwrap();
        let icon = import_test_icon(&mut connection, &paths, &collection.id);
        import_and_activate_ai_candidate(
            &mut connection,
            &paths,
            &collection.id,
            &icon.id,
            [20, 200, 80, 255],
        );
        import_and_activate_ai_candidate(
            &mut connection,
            &paths,
            &collection.id,
            &icon.id,
            [230, 120, 20, 255],
        );
        let request_count_before: i64 = connection
            .query_row("SELECT COUNT(*) FROM ai_requests", [], |row| row.get(0))
            .unwrap();
        let candidate_count_before: i64 = connection
            .query_row("SELECT COUNT(*) FROM ai_candidates", [], |row| row.get(0))
            .unwrap();

        let duplicate = duplicate_icon(&mut connection, &paths, &collection.id, &icon.id).unwrap();

        let request_count_after: i64 = connection
            .query_row("SELECT COUNT(*) FROM ai_requests", [], |row| row.get(0))
            .unwrap();
        let candidate_count_after: i64 = connection
            .query_row("SELECT COUNT(*) FROM ai_candidates", [], |row| row.get(0))
            .unwrap();
        assert_eq!(request_count_after, request_count_before);
        assert_eq!(candidate_count_after, candidate_count_before);

        let load_versions = |target_icon_id: &str| {
            let mut statement = connection
                .prepare(
                    "SELECT id, candidate_id, parent_version_id,
                            base_original_lineage_id, effective_source_file_id
                     FROM icon_ai_versions
                     WHERE icon_id = ?1",
                )
                .unwrap();
            let rows = statement
                .query_map([target_icon_id], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                })
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            rows
        };
        let source_versions = load_versions(&icon.id);
        let target_versions = load_versions(&duplicate.id);
        assert_eq!(source_versions.len(), 2);
        assert_eq!(target_versions.len(), source_versions.len());
        let source_by_candidate = source_versions
            .iter()
            .map(|version| (version.1.clone(), version))
            .collect::<HashMap<_, _>>();
        let target_by_candidate = target_versions
            .iter()
            .map(|version| (version.1.clone(), version))
            .collect::<HashMap<_, _>>();
        for (candidate_id, source_version) in &source_by_candidate {
            let target_version = target_by_candidate.get(candidate_id).unwrap();
            assert_ne!(target_version.0, source_version.0);
            assert_eq!(target_version.4, source_version.4);
            let source_parent_candidate = source_version.2.as_ref().map(|parent_id| {
                source_versions
                    .iter()
                    .find(|version| &version.0 == parent_id)
                    .unwrap()
                    .1
                    .clone()
            });
            let target_parent_candidate = target_version.2.as_ref().map(|parent_id| {
                target_versions
                    .iter()
                    .find(|version| &version.0 == parent_id)
                    .unwrap()
                    .1
                    .clone()
            });
            assert_eq!(target_parent_candidate, source_parent_candidate);
        }
        let source_state: (String, String, i64) = connection
            .query_row(
                "SELECT v.candidate_id, v.effective_source_file_id, st.revision
                 FROM icon_ai_state st
                 JOIN icon_ai_versions v ON v.icon_id = st.icon_id AND v.id = st.active_version_id
                 WHERE st.icon_id = ?1",
                [&icon.id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        let target_state: (String, String, i64) = connection
            .query_row(
                "SELECT v.candidate_id, v.effective_source_file_id, st.revision
                 FROM icon_ai_state st
                 JOIN icon_ai_versions v ON v.icon_id = st.icon_id AND v.id = st.active_version_id
                 WHERE st.icon_id = ?1",
                [&duplicate.id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(target_state, source_state);
        let lineages: (String, String) = connection
            .query_row(
                "SELECT source.original_lineage_id, target.original_lineage_id
                 FROM icons source JOIN icons target ON target.id = ?2
                 WHERE source.id = ?1",
                params![icon.id, duplicate.id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_ne!(lineages.0, lineages.1);
        assert!(target_versions
            .iter()
            .all(|version| version.3 == lineages.1));
        fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn duplicate_icon_maps_every_ai_lineage_without_merging_history() {
        let mut connection = connection();
        let paths = temp_paths();
        let collection =
            create_collection(&mut connection, Some("AI multi-lineage clone".to_string())).unwrap();
        let icon = import_test_icon(&mut connection, &paths, &collection.id);
        let historical_candidate_id = import_and_activate_ai_candidate(
            &mut connection,
            &paths,
            &collection.id,
            &icon.id,
            [30, 180, 90, 255],
        );

        replace_icon_source(
            &mut connection,
            &paths,
            &collection.id,
            &icon.id,
            ImportImageFilePayload {
                original_filename: "replacement-for-clone.png".to_string(),
                bytes: png_bytes_with_color([240, 40, 60, 255]),
            },
        )
        .unwrap();
        replace_icon_source(
            &mut connection,
            &paths,
            &collection.id,
            &icon.id,
            ImportImageFilePayload {
                original_filename: "current-source-for-clone.png".to_string(),
                bytes: png_bytes_with_color([210, 150, 35, 255]),
            },
        )
        .unwrap();

        let current_candidate_id = import_and_activate_ai_candidate(
            &mut connection,
            &paths,
            &collection.id,
            &icon.id,
            [80, 90, 230, 255],
        );

        let duplicate = duplicate_icon(&mut connection, &paths, &collection.id, &icon.id).unwrap();
        let load_version_lineage = |target_icon_id: &str, candidate_id: &str| {
            connection
                .query_row(
                    "SELECT base_original_source_file_id,
                            base_original_lineage_id,
                            base_original_lineage_generation
                     FROM icon_ai_versions
                     WHERE icon_id = ?1 AND candidate_id = ?2",
                    params![target_icon_id, candidate_id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, i64>(2)?,
                        ))
                    },
                )
                .unwrap()
        };

        let source_historical = load_version_lineage(&icon.id, &historical_candidate_id);
        let source_current = load_version_lineage(&icon.id, &current_candidate_id);
        let target_historical = load_version_lineage(&duplicate.id, &historical_candidate_id);
        let target_current = load_version_lineage(&duplicate.id, &current_candidate_id);

        assert_ne!(source_historical.1, source_current.1);
        assert_ne!(target_historical.1, target_current.1);
        assert_eq!(target_historical.0, source_historical.0);
        assert_eq!(target_historical.2, source_historical.2);
        assert_eq!(target_current.0, source_current.0);
        assert_eq!(target_current.2, source_current.2);
        assert_ne!(target_historical.1, source_historical.1);
        assert_ne!(target_current.1, source_current.1);

        let target_icon_lineage: (String, String, i64) = connection
            .query_row(
                "SELECT source_file_id, original_lineage_id, original_lineage_generation
                 FROM icons WHERE id = ?1",
                [&duplicate.id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(
            target_icon_lineage,
            (
                target_current.0.clone(),
                target_current.1.clone(),
                target_current.2,
            )
        );
        assert_ne!(target_historical.1, target_icon_lineage.1);

        let target_version_lineage_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM (
                   SELECT DISTINCT base_original_source_file_id,
                                   base_original_lineage_id,
                                   base_original_lineage_generation
                   FROM icon_ai_versions
                   WHERE icon_id = ?1
                 )",
                [&duplicate.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(target_version_lineage_count, 2);
        let source_registry_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM icon_ai_lineages WHERE icon_id = ?1",
                [&icon.id],
                |row| row.get(0),
            )
            .unwrap();
        let target_registry_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM icon_ai_lineages WHERE icon_id = ?1",
                [&duplicate.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(source_registry_count, 3);
        assert_eq!(target_registry_count, source_registry_count);

        let matched_registry_tuples: i64 = connection
            .query_row(
                "SELECT COUNT(*)
                 FROM icon_ai_lineages source
                 JOIN icon_ai_lineages target
                   ON target.original_source_file_id = source.original_source_file_id
                  AND target.lineage_generation = source.lineage_generation
                 WHERE source.icon_id = ?1
                   AND target.icon_id = ?2",
                params![icon.id, duplicate.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(matched_registry_tuples, source_registry_count);
        let reused_lineage_ids: i64 = connection
            .query_row(
                "SELECT COUNT(*)
                 FROM icon_ai_lineages source
                 JOIN icon_ai_lineages target
                   ON target.original_source_file_id = source.original_source_file_id
                  AND target.lineage_generation = source.lineage_generation
                  AND target.lineage_id = source.lineage_id
                 WHERE source.icon_id = ?1
                   AND target.icon_id = ?2",
                params![icon.id, duplicate.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(reused_lineage_ids, 0);

        let active_candidate_id: String = connection
            .query_row(
                "SELECT v.candidate_id
                 FROM icon_ai_state st
                 JOIN icon_ai_versions v ON v.id = st.active_version_id
                 WHERE st.icon_id = ?1 AND v.icon_id = st.icon_id",
                [&duplicate.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(active_candidate_id, current_candidate_id);

        let violations = connection
            .prepare("PRAGMA foreign_key_check")
            .unwrap()
            .query_map([], |_| Ok(()))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert!(violations.is_empty());
        fs::remove_dir_all(paths.root).unwrap();
    }
    fn active_effective_source_path(connection: &Connection, icon_id: &str) -> String {
        connection
            .query_row(
                "SELECT sf.original_path_in_library
                 FROM icon_ai_state st
                 JOIN icon_ai_versions version
                   ON version.id = st.active_version_id
                  AND version.icon_id = st.icon_id
                 JOIN source_files sf ON sf.id = version.effective_source_file_id
                 WHERE st.icon_id = ?1",
                [icon_id],
                |row| row.get(0),
            )
            .unwrap()
    }

    #[test]
    fn duplicate_icon_rejects_missing_active_effective_source_even_with_preview_remaining() {
        let mut connection = connection();
        let paths = temp_paths();
        let collection = create_collection(
            &mut connection,
            Some("missing active AI source clone".to_string()),
        )
        .unwrap();
        let icon = import_test_icon(&mut connection, &paths, &collection.id);
        import_and_activate_ai_candidate(
            &mut connection,
            &paths,
            &collection.id,
            &icon.id,
            [20, 220, 80, 255],
        );
        let effective_source_path = active_effective_source_path(&connection, &icon.id);
        let preview_path: String = connection
            .query_row(
                "SELECT current_preview_path FROM icons WHERE id = ?1",
                [&icon.id],
                |row| row.get(0),
            )
            .unwrap();
        assert!(std::path::Path::new(&preview_path).is_file());
        let count_before: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM icons WHERE collection_id = ?1 AND deleted_at IS NULL",
                [&collection.id],
                |row| row.get(0),
            )
            .unwrap();

        fs::remove_file(&effective_source_path).unwrap();
        let result = duplicate_icon(&mut connection, &paths, &collection.id, &icon.id);

        assert!(result.is_err());
        let count_after: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM icons WHERE collection_id = ?1 AND deleted_at IS NULL",
                [&collection.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count_after, count_before);
        assert!(std::path::Path::new(&preview_path).is_file());
        fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn duplicate_icon_rejects_tampered_active_effective_source_even_with_preview_remaining() {
        let mut connection = connection();
        let paths = temp_paths();
        let collection = create_collection(
            &mut connection,
            Some("tampered active AI source clone".to_string()),
        )
        .unwrap();
        let icon = import_test_icon(&mut connection, &paths, &collection.id);
        import_and_activate_ai_candidate(
            &mut connection,
            &paths,
            &collection.id,
            &icon.id,
            [230, 60, 100, 255],
        );
        let effective_source_path = active_effective_source_path(&connection, &icon.id);
        let preview_path: String = connection
            .query_row(
                "SELECT current_preview_path FROM icons WHERE id = ?1",
                [&icon.id],
                |row| row.get(0),
            )
            .unwrap();
        assert!(std::path::Path::new(&preview_path).is_file());
        let count_before: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM icons WHERE collection_id = ?1 AND deleted_at IS NULL",
                [&collection.id],
                |row| row.get(0),
            )
            .unwrap();
        let mut tampered = fs::read(&effective_source_path).unwrap();
        let tamper_index = tampered.len() / 2;
        tampered[tamper_index] ^= 0x01;
        fs::write(&effective_source_path, &tampered).unwrap();

        let result = duplicate_icon(&mut connection, &paths, &collection.id, &icon.id);

        assert!(result.is_err());
        let count_after: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM icons WHERE collection_id = ?1 AND deleted_at IS NULL",
                [&collection.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count_after, count_before);
        assert!(std::path::Path::new(&preview_path).is_file());
        fs::remove_dir_all(paths.root).unwrap();
    }
}
