use std::collections::{HashMap, HashSet};

use crate::db::repositories::clone_artifacts::{
    cleanup_cloned_collection_previews, clone_current_ai_lineage, clone_effective_active_variants,
    clone_frame_sheet_gif_recipe, materialize_clone_native_preview,
    validate_collection_clone_sources, validate_icon_clone_target,
};
use rusqlite::{params, Connection, OptionalExtension, Row, Transaction};

use crate::db::repositories::source_files::{
    import_source_file_from_bytes, SourceFileImportOptions,
};
use crate::error::{AppError, AppResult};
use crate::ids::create_id;
use crate::imaging::import_limits::validate_import_dimensions;
use crate::models::{CollectionDto, ImportImageFilePayload, UpdateCollectionSettingsPayload};
use crate::paths::AppPaths;

pub fn list_collections(connection: &Connection) -> AppResult<Vec<CollectionDto>> {
    let mut statement = connection.prepare(
        "SELECT
           c.id,
           c.name,
           c.cover_source_file_id,
           c.cover_icon_id,
           c.default_cell_width,
           c.default_cell_height,
           c.preview_width,
           c.preview_height,
           c.export_format,
           c.max_bytes,
           c.created_at,
           c.updated_at,
           (
             SELECT COUNT(*)
             FROM icons i
             WHERE i.collection_id = c.id
               AND i.deleted_at IS NULL
           ) AS icon_count,
           CASE WHEN cover_icon.id IS NOT NULL THEN CASE
             WHEN cover_icon.thumbnail_override_path IS NOT NULL
               THEN cover_icon.thumbnail_override_path
             WHEN cover_icon.current_preview_path IS NOT NULL
               THEN cover_icon.current_preview_path
             WHEN cover_ai_state.active_version_id IS NOT NULL
               THEN NULL
             ELSE COALESCE(
               cover_icon.thumbnail_path,
               cover_icon_source.original_path_in_library
             )
           END
             ELSE cover_source.original_path_in_library
           END AS cover_image_url
         FROM collections c
         LEFT JOIN icons cover_icon
           ON cover_icon.id = c.cover_icon_id
          AND cover_icon.deleted_at IS NULL
         LEFT JOIN source_files cover_source
           ON cover_source.id = c.cover_source_file_id
         LEFT JOIN effective_visual_sources cover_visual
           ON cover_visual.icon_id = cover_icon.id
         LEFT JOIN source_files cover_icon_source
           ON cover_icon_source.id = cover_visual.effective_source_file_id
         LEFT JOIN icon_ai_state cover_ai_state
           ON cover_ai_state.icon_id = cover_icon.id
         WHERE c.deleted_at IS NULL
         ORDER BY c.order_index ASC, c.created_at ASC",
    )?;

    let collections = statement
        .query_map([], collection_from_row)?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(collections)
}

pub fn get_collection(connection: &Connection, collection_id: &str) -> AppResult<CollectionDto> {
    connection
        .query_row(
            "SELECT
               c.id,
               c.name,
               c.cover_source_file_id,
               c.cover_icon_id,
               c.default_cell_width,
               c.default_cell_height,
               c.preview_width,
               c.preview_height,
               c.export_format,
               c.max_bytes,
               c.created_at,
               c.updated_at,
               (
                 SELECT COUNT(*)
                 FROM icons i
                 WHERE i.collection_id = c.id
                   AND i.deleted_at IS NULL
               ) AS icon_count,
               CASE WHEN cover_icon.id IS NOT NULL THEN CASE
                 WHEN cover_icon.thumbnail_override_path IS NOT NULL
                   THEN cover_icon.thumbnail_override_path
                 WHEN cover_icon.current_preview_path IS NOT NULL
                   THEN cover_icon.current_preview_path
                 WHEN cover_ai_state.active_version_id IS NOT NULL
                   THEN NULL
                 ELSE COALESCE(
                   cover_icon.thumbnail_path,
                   cover_icon_source.original_path_in_library
                 )
               END
                 ELSE cover_source.original_path_in_library
               END AS cover_image_url
             FROM collections c
             LEFT JOIN icons cover_icon
               ON cover_icon.id = c.cover_icon_id
              AND cover_icon.deleted_at IS NULL
             LEFT JOIN source_files cover_source
               ON cover_source.id = c.cover_source_file_id
             LEFT JOIN effective_visual_sources cover_visual
               ON cover_visual.icon_id = cover_icon.id
             LEFT JOIN source_files cover_icon_source
               ON cover_icon_source.id = cover_visual.effective_source_file_id
             LEFT JOIN icon_ai_state cover_ai_state
               ON cover_ai_state.icon_id = cover_icon.id
             WHERE c.id = ?1
               AND c.deleted_at IS NULL",
            params![collection_id],
            collection_from_row,
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("모음을 찾을 수 없습니다."))
}

pub fn create_collection(
    connection: &mut Connection,
    name: Option<String>,
) -> AppResult<CollectionDto> {
    let transaction = connection.transaction()?;
    let collection_id = create_id("collection");
    let profile_id = create_id("profile");
    let collection_name =
        normalized_name(name).unwrap_or_else(|| default_collection_name(&transaction));
    let order_index = next_collection_order_index(&transaction)?;

    transaction.execute(
        "INSERT INTO collections (
           id,
           name,
           order_index,
           created_at,
           updated_at
         )
         VALUES (
           ?1,
           ?2,
           ?3,
           strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
           strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         )",
        params![collection_id, collection_name, order_index],
    )?;

    insert_default_profile(&transaction, &profile_id, &collection_id)?;
    transaction.commit()?;

    get_collection(connection, &collection_id)
}

pub fn rename_collection(
    connection: &Connection,
    collection_id: &str,
    name: String,
) -> AppResult<CollectionDto> {
    let collection_name =
        normalized_name(Some(name)).unwrap_or_else(|| "이름 없는 모음".to_string());
    let changed = connection.execute(
        "UPDATE collections
         SET name = ?1,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?2
           AND deleted_at IS NULL",
        params![collection_name, collection_id],
    )?;

    if changed == 0 {
        return Err(AppError::not_found("이름을 바꿀 모음을 찾을 수 없습니다."));
    }

    get_collection(connection, collection_id)
}

pub fn delete_collection(connection: &mut Connection, collection_id: &str) -> AppResult<()> {
    let transaction = connection.transaction()?;
    let changed = transaction.execute(
        "UPDATE collections
         SET deleted_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?1
           AND deleted_at IS NULL",
        params![collection_id],
    )?;

    if changed == 0 {
        return Err(AppError::not_found("삭제할 모음을 찾을 수 없습니다."));
    }

    transaction.execute(
        "UPDATE icons
         SET deleted_at = COALESCE(deleted_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE collection_id = ?1
           AND deleted_at IS NULL",
        params![collection_id],
    )?;
    transaction.commit()?;

    Ok(())
}

pub fn duplicate_collection(
    connection: &mut Connection,
    paths: &AppPaths,
    collection_id: &str,
) -> AppResult<CollectionDto> {
    validate_collection_clone_sources(connection, collection_id)?;
    let duplicate_id = create_id("collection");
    let duplicate_result = (|| -> AppResult<CollectionDto> {
        let transaction = connection.transaction()?;
        let original = get_collection(&transaction, collection_id)?;
        let duplicate_name = next_duplicate_collection_name(&transaction, &original.name)?;
        let order_index = next_collection_order_index(&transaction)?;

        transaction.execute(
            "INSERT INTO collections (
               id,
               name,
               cover_source_file_id,
               default_cell_width,
               default_cell_height,
               preview_width,
               preview_height,
               export_format,
               max_bytes,
               allowed_formats_json,
               order_index,
               created_at,
               updated_at
             )
             SELECT
               ?1,
               ?2,
               cover_source_file_id,
               default_cell_width,
               default_cell_height,
               preview_width,
               preview_height,
               export_format,
               max_bytes,
               allowed_formats_json,
               ?3,
               strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
               strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             FROM collections
             WHERE id = ?4
               AND deleted_at IS NULL",
            params![&duplicate_id, duplicate_name, order_index, collection_id],
        )?;

        let profile_id_map = duplicate_export_profiles(&transaction, collection_id, &duplicate_id)?;
        duplicate_collection_scoped_sheet_grid_presets(&transaction, collection_id, &duplicate_id)?;
        let icon_id_map = duplicate_icons(
            &transaction,
            paths,
            collection_id,
            &duplicate_id,
            &profile_id_map,
        )?;

        if let Some(original_cover_icon_id) = original.cover_icon_id {
            if let Some(duplicate_cover_icon_id) = icon_id_map.get(&original_cover_icon_id) {
                let duplicate_cover_source_file_id: Option<String> = transaction
                    .query_row(
                        "SELECT source_file_id FROM icons WHERE id = ?1",
                        params![duplicate_cover_icon_id],
                        |row| row.get(0),
                    )
                    .optional()?;
                transaction.execute(
                    "UPDATE collections
                     SET cover_icon_id = ?1,
                         cover_source_file_id = ?2
                     WHERE id = ?3",
                    params![
                        duplicate_cover_icon_id,
                        duplicate_cover_source_file_id,
                        &duplicate_id
                    ],
                )?;
            }
        }

        let duplicated = get_collection(&transaction, &duplicate_id)?;
        transaction.commit()?;
        Ok(duplicated)
    })();

    if duplicate_result.is_err() {
        cleanup_cloned_collection_previews(paths, &duplicate_id);
    }
    duplicate_result
}

pub fn set_collection_cover_icon(
    connection: &Connection,
    collection_id: &str,
    icon_id: &str,
) -> AppResult<CollectionDto> {
    let source_file_id: String = connection
        .query_row(
            "SELECT source_file_id
             FROM icons
             WHERE id = ?1
               AND collection_id = ?2
               AND deleted_at IS NULL",
            params![icon_id, collection_id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("대표 이미지로 지정할 아이콘을 찾을 수 없습니다."))?;

    let changed = connection.execute(
        "UPDATE collections
         SET cover_icon_id = ?1,
             cover_source_file_id = ?2,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?3
           AND deleted_at IS NULL",
        params![icon_id, source_file_id, collection_id],
    )?;

    if changed == 0 {
        return Err(AppError::not_found(
            "대표 이미지를 바꿀 모음을 찾을 수 없습니다.",
        ));
    }

    get_collection(connection, collection_id)
}

pub fn update_collection_settings(
    connection: &Connection,
    collection_id: &str,
    payload: UpdateCollectionSettingsPayload,
) -> AppResult<CollectionDto> {
    validate_collection_settings(&payload)?;
    let export_format = normalized_export_format(&payload.export_format);

    let changed = connection.execute(
        "UPDATE collections
         SET default_cell_width = ?1,
             default_cell_height = ?2,
             preview_width = ?3,
             preview_height = ?4,
             export_format = ?5,
             max_bytes = ?6,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?7
           AND deleted_at IS NULL",
        params![
            payload.default_cell_width,
            payload.default_cell_height,
            payload.preview_width,
            payload.preview_height,
            export_format,
            payload.max_bytes,
            collection_id,
        ],
    )?;

    if changed == 0 {
        return Err(AppError::not_found(
            "설정을 저장할 모음을 찾을 수 없습니다.",
        ));
    }

    connection.execute(
        "UPDATE export_profiles
         SET target_format = ?1,
             target_cell_width = ?2,
             target_cell_height = ?3,
             preview_width = ?4,
             preview_height = ?5,
             max_bytes = ?6,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE collection_id = ?7
           AND profile_type = 'custom'",
        params![
            export_format,
            payload.default_cell_width,
            payload.default_cell_height,
            payload.preview_width,
            payload.preview_height,
            payload.max_bytes,
            collection_id,
        ],
    )?;

    get_collection(connection, collection_id)
}

pub fn import_collection_cover_image(
    connection: &mut Connection,
    paths: &AppPaths,
    collection_id: &str,
    file: ImportImageFilePayload,
) -> AppResult<CollectionDto> {
    let transaction = connection.transaction()?;
    ensure_collection_exists(&transaction, collection_id)?;
    let source_file = import_source_file_from_bytes(
        &transaction,
        paths,
        &file,
        SourceFileImportOptions {
            allow_gif: false,
            exact_dimensions: Some((200, 200)),
        },
    )?;

    transaction.execute(
        "UPDATE collections
         SET cover_source_file_id = ?1,
             cover_icon_id = NULL,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?2
           AND deleted_at IS NULL",
        params![source_file.id, collection_id],
    )?;
    transaction.commit()?;

    get_collection(connection, collection_id)
}

pub(crate) fn collection_from_row(row: &Row<'_>) -> rusqlite::Result<CollectionDto> {
    Ok(CollectionDto {
        id: row.get("id")?,
        name: row.get("name")?,
        cover_source_file_id: row.get("cover_source_file_id")?,
        cover_icon_id: row.get("cover_icon_id")?,
        cover_image_url: row.get("cover_image_url")?,
        icon_count: row.get("icon_count")?,
        default_cell_width: row.get("default_cell_width")?,
        default_cell_height: row.get("default_cell_height")?,
        preview_width: row.get("preview_width")?,
        preview_height: row.get("preview_height")?,
        export_format: row.get("export_format")?,
        max_bytes: row.get("max_bytes")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn ensure_collection_exists(connection: &Connection, collection_id: &str) -> AppResult<()> {
    let exists = connection
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

    if exists {
        Ok(())
    } else {
        Err(AppError::not_found("모음을 찾을 수 없습니다."))
    }
}

fn validate_collection_settings(payload: &UpdateCollectionSettingsPayload) -> AppResult<()> {
    let default_width = u32::try_from(payload.default_cell_width)
        .map_err(|_| AppError::new("validation", "모음 기준 너비가 올바르지 않습니다."))?;
    let default_height = u32::try_from(payload.default_cell_height)
        .map_err(|_| AppError::new("validation", "모음 기준 높이가 올바르지 않습니다."))?;
    validate_import_dimensions(default_width, default_height)?;
    let preview_width = u32::try_from(payload.preview_width)
        .map_err(|_| AppError::new("validation", "모음 표시 너비가 올바르지 않습니다."))?;
    let preview_height = u32::try_from(payload.preview_height)
        .map_err(|_| AppError::new("validation", "모음 표시 높이가 올바르지 않습니다."))?;
    validate_import_dimensions(preview_width, preview_height)?;

    if payload.max_bytes <= 0 {
        return Err(AppError::new(
            "validation",
            "파일 용량 제한은 1바이트 이상이어야 합니다.",
        ));
    }

    match payload.export_format.as_str() {
        "jpg" | "jpeg" | "png" | "gif" | "source" => Ok(()),
        _ => Err(AppError::new(
            "validation",
            "지원하지 않는 기본 내보내기 형식입니다.",
        )),
    }
}

fn normalized_export_format(format: &str) -> String {
    match format.trim().to_ascii_lowercase().as_str() {
        "jpeg" | "jpg" => "jpg".to_string(),
        "gif" => "gif".to_string(),
        "source" => "source".to_string(),
        _ => "png".to_string(),
    }
}

fn normalized_name(name: Option<String>) -> Option<String> {
    name.map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn default_collection_name(connection: &Connection) -> String {
    let next_number = connection
        .query_row(
            "SELECT COUNT(*) + 1 FROM collections WHERE deleted_at IS NULL",
            [],
            |row| row.get::<_, i64>(0),
        )
        .unwrap_or(1);

    format!("디시콘 모음 {next_number}")
}

fn next_collection_order_index(connection: &Connection) -> AppResult<i64> {
    Ok(connection.query_row(
        "SELECT COALESCE(MAX(order_index) + 1, 0)
         FROM collections
         WHERE deleted_at IS NULL",
        [],
        |row| row.get(0),
    )?)
}

fn insert_default_profile(
    transaction: &Transaction<'_>,
    profile_id: &str,
    collection_id: &str,
) -> AppResult<()> {
    transaction.execute(
        "INSERT INTO export_profiles (
           id,
           collection_id,
           name,
           profile_type,
           created_at,
           updated_at
         )
         VALUES (
           ?1,
           ?2,
           'DCInside',
           'dcinside',
           strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
           strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         )",
        params![profile_id, collection_id],
    )?;

    Ok(())
}

#[derive(Debug)]
struct ExportProfileRecord {
    id: String,
    name: String,
    profile_type: String,
    target_format: String,
    target_cell_width: i64,
    target_cell_height: i64,
    preview_width: i64,
    preview_height: i64,
    max_bytes: i64,
    allowed_formats_json: String,
    filename_mode: String,
    include_alt_txt: i64,
    strict_warnings: i64,
}

fn duplicate_export_profiles(
    transaction: &Transaction<'_>,
    source_collection_id: &str,
    target_collection_id: &str,
) -> AppResult<HashMap<String, String>> {
    let profiles = {
        let mut statement = transaction.prepare(
            "SELECT
               id,
               name,
               profile_type,
               target_format,
               target_cell_width,
               target_cell_height,
               preview_width,
               preview_height,
               max_bytes,
               allowed_formats_json,
               filename_mode,
               include_alt_txt,
               strict_warnings
             FROM export_profiles
             WHERE collection_id = ?1
             ORDER BY created_at ASC",
        )?;

        let profiles = statement
            .query_map(params![source_collection_id], |row| {
                Ok(ExportProfileRecord {
                    id: row.get("id")?,
                    name: row.get("name")?,
                    profile_type: row.get("profile_type")?,
                    target_format: row.get("target_format")?,
                    target_cell_width: row.get("target_cell_width")?,
                    target_cell_height: row.get("target_cell_height")?,
                    preview_width: row.get("preview_width")?,
                    preview_height: row.get("preview_height")?,
                    max_bytes: row.get("max_bytes")?,
                    allowed_formats_json: row.get("allowed_formats_json")?,
                    filename_mode: row.get("filename_mode")?,
                    include_alt_txt: row.get("include_alt_txt")?,
                    strict_warnings: row.get("strict_warnings")?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        profiles
    };

    let mut profile_id_map = HashMap::new();
    for profile in profiles {
        let target_profile_id = create_id("profile");
        transaction.execute(
            "INSERT INTO export_profiles (
               id,
               collection_id,
               name,
               profile_type,
               target_format,
               target_cell_width,
               target_cell_height,
               preview_width,
               preview_height,
               max_bytes,
               allowed_formats_json,
               filename_mode,
               include_alt_txt,
               strict_warnings,
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
               strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
               strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             )",
            params![
                &target_profile_id,
                target_collection_id,
                profile.name,
                profile.profile_type,
                profile.target_format,
                profile.target_cell_width,
                profile.target_cell_height,
                profile.preview_width,
                profile.preview_height,
                profile.max_bytes,
                profile.allowed_formats_json,
                profile.filename_mode,
                profile.include_alt_txt,
                profile.strict_warnings,
            ],
        )?;
        profile_id_map.insert(profile.id, target_profile_id);
    }

    Ok(profile_id_map)
}

fn duplicate_collection_scoped_sheet_grid_presets(
    transaction: &Transaction<'_>,
    source_collection_id: &str,
    target_collection_id: &str,
) -> AppResult<()> {
    let source_preset_ids = {
        let mut statement = transaction.prepare(
            "SELECT id
             FROM sheet_grid_presets
             WHERE scope = 'collection'
               AND collection_id = ?1
               AND is_builtin = 0
             ORDER BY created_at ASC, id ASC",
        )?;
        let preset_ids = statement
            .query_map(params![source_collection_id], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        preset_ids
    };

    for source_preset_id in source_preset_ids {
        transaction.execute(
            "INSERT INTO sheet_grid_presets (
               id,
               name,
               scope,
               collection_id,
               kind,
               cell_width,
               cell_height,
               rows,
               columns,
               mode,
               gap_x,
               gap_y,
               border_left,
               border_top,
               border_right,
               border_bottom,
               read_order,
               background,
               max_sheet_width,
               max_sheet_height,
               frames_per_page,
               include_clean_sheet,
               include_guide_sheet,
               include_manifest,
               guide_label_options_json,
               is_default_for_import,
               is_default_for_export,
               is_default_for_gif_frame,
               is_builtin,
               created_at,
               updated_at
             )
             SELECT
               ?1,
               name,
               'collection',
               ?2,
               kind,
               cell_width,
               cell_height,
               rows,
               columns,
               mode,
               gap_x,
               gap_y,
               border_left,
               border_top,
               border_right,
               border_bottom,
               read_order,
               background,
               max_sheet_width,
               max_sheet_height,
               frames_per_page,
               include_clean_sheet,
               include_guide_sheet,
               include_manifest,
               guide_label_options_json,
               is_default_for_import,
               is_default_for_export,
               is_default_for_gif_frame,
               0,
               strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
               strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             FROM sheet_grid_presets
             WHERE id = ?3
               AND scope = 'collection'
               AND collection_id = ?4
               AND is_builtin = 0",
            params![
                create_id("sheet-preset"),
                target_collection_id,
                source_preset_id,
                source_collection_id,
            ],
        )?;
    }

    Ok(())
}
#[derive(Debug)]
struct IconRecord {
    id: String,
    source_file_id: String,
    display_name: String,
    icon_kind: String,
    readiness: String,
    placeholder_text: Option<String>,
    shape: String,
    order_index: i64,
    cell_width_override: Option<i64>,
    cell_height_override: Option<i64>,
    thumbnail_path: Option<String>,
    thumbnail_override_source_file_id: Option<String>,
    thumbnail_override_path: Option<String>,
    current_preview_path: Option<String>,
    text_overlay_enabled: i64,
    text_overlay_text: String,
    text_overlay_font_path: Option<String>,
    text_overlay_font_size: f64,
    text_overlay_x: f64,
    text_overlay_y: f64,
    text_overlay_color: String,
    text_overlay_stroke_color: String,
    text_overlay_stroke_width: f64,
    transform_quarter_turns: i64,
    transform_flip_horizontal: i64,
    transform_flip_vertical: i64,
    gif_loop_mode: String,
    gif_loop_count: Option<i64>,
    gif_pingpong: i64,
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

fn duplicate_icons(
    transaction: &Transaction<'_>,
    paths: &AppPaths,
    source_collection_id: &str,
    target_collection_id: &str,
    profile_id_map: &HashMap<String, String>,
) -> AppResult<HashMap<String, String>> {
    let icons = {
        let mut statement = transaction.prepare(
            "SELECT
               id,
               source_file_id,
               icon_kind,
               readiness,
               placeholder_text,
               display_name,
               shape,
               order_index,
               cell_width_override,
               cell_height_override,
               thumbnail_path,
               thumbnail_override_source_file_id,
               thumbnail_override_path,
               current_preview_path,
               gif_loop_mode,
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
               gif_loop_count,
               gif_pingpong
             FROM icons
             WHERE collection_id = ?1
               AND deleted_at IS NULL
             ORDER BY order_index ASC, created_at ASC",
        )?;

        let icons = statement
            .query_map(params![source_collection_id], |row| {
                Ok(IconRecord {
                    id: row.get("id")?,
                    source_file_id: row.get("source_file_id")?,
                    display_name: row.get("display_name")?,
                    shape: row.get("shape")?,
                    order_index: row.get("order_index")?,
                    cell_width_override: row.get("cell_width_override")?,
                    cell_height_override: row.get("cell_height_override")?,
                    icon_kind: row.get("icon_kind")?,
                    readiness: row.get("readiness")?,
                    placeholder_text: row.get("placeholder_text")?,
                    thumbnail_path: row.get("thumbnail_path")?,
                    thumbnail_override_source_file_id: row
                        .get("thumbnail_override_source_file_id")?,
                    thumbnail_override_path: row.get("thumbnail_override_path")?,
                    current_preview_path: row.get("current_preview_path")?,
                    text_overlay_enabled: row.get("text_overlay_enabled")?,
                    text_overlay_text: row.get("text_overlay_text")?,
                    text_overlay_font_path: row.get("text_overlay_font_path")?,
                    text_overlay_font_size: row.get("text_overlay_font_size")?,
                    text_overlay_x: row.get("text_overlay_x")?,
                    text_overlay_y: row.get("text_overlay_y")?,
                    text_overlay_color: row.get("text_overlay_color")?,
                    text_overlay_stroke_color: row.get("text_overlay_stroke_color")?,
                    text_overlay_stroke_width: row.get("text_overlay_stroke_width")?,
                    transform_quarter_turns: row.get("transform_quarter_turns")?,
                    transform_flip_horizontal: row.get("transform_flip_horizontal")?,
                    transform_flip_vertical: row.get("transform_flip_vertical")?,
                    gif_loop_mode: row.get("gif_loop_mode")?,
                    gif_loop_count: row.get("gif_loop_count")?,
                    gif_pingpong: row.get("gif_pingpong")?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        icons
    };

    let mut icon_id_map = HashMap::new();

    for icon in icons {
        let duplicate_icon_id = create_id("icon");
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
                target_collection_id,
                icon.source_file_id,
                icon.display_name,
                icon.icon_kind,
                icon.readiness,
                icon.placeholder_text,
                icon.shape,
                icon.order_index,
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
            transaction,
            paths,
            target_collection_id,
            &icon.id,
            &duplicate_icon_id,
        )?;
        duplicate_crop_settings(transaction, &icon.id, &duplicate_icon_id)?;
        duplicate_icon_note(transaction, &icon.id, &duplicate_icon_id)?;
        duplicate_icon_effect_recipe(transaction, &icon.id, &duplicate_icon_id)?;
        duplicate_icon_motion_recipe(transaction, &icon.id, &duplicate_icon_id)?;
        clone_frame_sheet_gif_recipe(transaction, &icon.id, &duplicate_icon_id)?;
        clone_current_ai_lineage(transaction, &icon.id, &duplicate_icon_id)?;
        validate_icon_clone_target(transaction, target_collection_id, &duplicate_icon_id)?;
        materialize_clone_native_preview(
            transaction,
            paths,
            target_collection_id,
            &duplicate_icon_id,
        )?;
        clone_effective_active_variants(
            transaction,
            paths,
            target_collection_id,
            &icon.id,
            &duplicate_icon_id,
            &piece_id_map,
            Some(profile_id_map),
        )?;
        icon_id_map.insert(icon.id, duplicate_icon_id);
    }

    Ok(icon_id_map)
}

fn next_duplicate_collection_name(
    connection: &Connection,
    original_name: &str,
) -> AppResult<String> {
    let base = format!("{original_name} 복사본");
    let existing_names = active_collection_names(connection)?;
    if !existing_names.contains(&base) {
        return Ok(base);
    }

    for copy_number in 2..10_000 {
        let candidate = format!("{base} {copy_number}");
        if !existing_names.contains(&candidate) {
            return Ok(candidate);
        }
    }

    Err(AppError::new(
        "validation",
        "복제할 모음 이름을 만들 수 없습니다.",
    ))
}

fn active_collection_names(connection: &Connection) -> AppResult<HashSet<String>> {
    let mut statement = connection.prepare(
        "SELECT name
         FROM collections
         WHERE deleted_at IS NULL",
    )?;
    let names = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<HashSet<_>, _>>()?;
    Ok(names)
}

fn duplicate_icon_pieces(
    transaction: &Transaction<'_>,
    _paths: &AppPaths,
    _target_collection_id: &str,
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

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::time::{SystemTime, UNIX_EPOCH};

    use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};
    use rusqlite::{params, Connection};

    use crate::db::migrations;
    use crate::db::repositories::ai_activation;
    use crate::db::repositories::imports::import_image_files;
    use crate::models::{ImportImageFilePayload, UpdateCollectionSettingsPayload};
    use crate::paths::AppPaths;

    use super::{
        create_collection, delete_collection, duplicate_collection, import_collection_cover_image,
        list_collections, rename_collection, update_collection_settings,
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
        AppPaths::prepare(std::env::temp_dir().join(format!("pmtconcon-collections-{suffix}")))
            .unwrap()
    }

    fn png_bytes(width: u32, height: u32) -> Vec<u8> {
        let image = ImageBuffer::from_pixel(width, height, Rgba([255, 0, 0, 255]));
        let mut cursor = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image)
            .write_to(&mut cursor, ImageFormat::Png)
            .unwrap();
        cursor.into_inner()
    }

    #[test]
    fn collection_crud_uses_sqlite() {
        let mut connection = connection();

        let created = create_collection(&mut connection, Some("테스트 모음".to_string())).unwrap();
        assert_eq!(created.name, "테스트 모음");

        let renamed =
            rename_collection(&connection, &created.id, "수정한 모음".to_string()).unwrap();
        assert_eq!(renamed.name, "수정한 모음");

        let listed = list_collections(&connection).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "수정한 모음");

        delete_collection(&mut connection, &created.id).unwrap();
        assert!(list_collections(&connection).unwrap().is_empty());
    }

    #[test]
    fn duplicate_collection_creates_separate_collection_and_profile() {
        let mut connection = connection();
        let paths = temp_paths();
        let created = create_collection(&mut connection, Some("원본".to_string())).unwrap();

        let duplicated = duplicate_collection(&mut connection, &paths, &created.id).unwrap();

        assert_ne!(created.id, duplicated.id);
        assert_eq!(duplicated.name, "원본 복사본");

        let profile_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM export_profiles WHERE collection_id = ?1",
                [&duplicated.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(profile_count, 1);
        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn duplicate_collection_owns_previews_and_preserves_icon_render_state() {
        let mut connection = connection();
        let paths = temp_paths();
        let created =
            create_collection(&mut connection, Some("render collection clone".to_string()))
                .unwrap();
        let imported = import_image_files(
            &mut connection,
            &paths,
            &created.id,
            vec![ImportImageFilePayload {
                original_filename: "render.png".to_string(),
                bytes: png_bytes(32, 32),
            }],
        )
        .unwrap();
        let source_icon = &imported.imported_icons[0];
        let source_icon_id = source_icon.id.clone();
        let source_piece_id = source_icon.pieces[0].id.clone();
        let source_dir = paths
            .collection_previews_dir
            .join(&created.id)
            .join(&source_icon_id)
            .join("effects")
            .join("source-artifact");
        std::fs::create_dir_all(&source_dir).unwrap();
        let source_preview = source_dir.join("preview.png");
        let source_piece = source_dir.join("piece-00.png");
        std::fs::write(&source_preview, png_bytes(32, 32)).unwrap();
        std::fs::write(&source_piece, png_bytes(32, 32)).unwrap();
        let old_export = paths.root.join("old-piece-export.png");

        connection
            .execute(
                "UPDATE icons
                 SET current_preview_path = ?1,
                     icon_kind = 'placeholder',
                     readiness = 'working',
                     placeholder_text = 'finish me',
                     text_overlay_enabled = 1,
                     text_overlay_text = 'collection text',
                     text_overlay_font_path = NULL,
                     text_overlay_font_size = 21.0,
                     text_overlay_x = 0.2,
                     text_overlay_y = 0.8,
                     text_overlay_color = '#102030FF',
                     text_overlay_stroke_color = '#F0E0D0',
                     text_overlay_stroke_width = 1.5,
                     transform_quarter_turns = 1,
                     transform_flip_horizontal = 1,
                     transform_flip_vertical = 0
                 WHERE id = ?2",
                params![source_preview.to_string_lossy(), source_icon_id],
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
                    source_piece_id
                ],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO icon_notes (icon_id, note, updated_at)
                 VALUES (?1, 'collection note', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
                [&source_icon_id],
            )
            .unwrap();
        let effects_json =
            r#"{"version":1,"effects":[{"kind":"blur","id":"blur","enabled":true,"radius":2}]}"#;
        connection
            .execute(
                "INSERT INTO icon_effect_recipes (
                   icon_id, recipe_schema, revision, effects_json, created_at, updated_at
                 )
                 VALUES (
                   ?1, 'pmtcon-effects-v1', 2, ?2,
                   strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                   strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 )",
                params![source_icon_id, effects_json],
            )
            .unwrap();
        let motion_json = r#"{"version":1,"durationMs":800,"fps":15,"seed":999,"interpolation":"nearest","edgeMode":"clamp","spatial":null,"displacement":{"kind":"wave","enabled":true,"cyclesPerLoop":2,"axis":"horizontal","amplitudePx":2,"wavelengthPx":12},"colorOpacity":null,"overlay":null}"#;
        connection
            .execute(
                "INSERT INTO icon_motion_recipes (
                   icon_id, recipe_schema, revision, motion_json, created_at, updated_at
                 )
                 VALUES (
                   ?1, 'pmtcon-motion-v1', 4, ?2,
                   strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                   strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 )",
                params![source_icon_id, motion_json],
            )
            .unwrap();

        let duplicated = duplicate_collection(&mut connection, &paths, &created.id).unwrap();
        let expected_preview_dir = paths
            .ai_activation_staging_dir
            .join("collection-clone-source-expected");
        let expected_preview = ai_activation::render_effective_preview_to_directory(
            &connection,
            &expected_preview_dir,
            &created.id,
            &source_icon_id,
        )
        .unwrap();
        let (
            target_icon_id,
            target_preview,
            icon_kind,
            readiness,
            placeholder_text,
            quarter_turns,
            flip_horizontal,
            flip_vertical,
        ): (
            String,
            String,
            String,
            String,
            Option<String>,
            i64,
            i64,
            i64,
        ) = connection
            .query_row(
                "SELECT id, current_preview_path, icon_kind, readiness, placeholder_text,
                        transform_quarter_turns, transform_flip_horizontal,
                        transform_flip_vertical
                 FROM icons WHERE collection_id = ?1",
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
                    ))
                },
            )
            .unwrap();
        let (target_piece, last_export, export_status): (String, Option<String>, String) =
            connection
                .query_row(
                    "SELECT generated_preview_path, last_export_path, export_status
                     FROM icon_pieces WHERE icon_id = ?1 AND piece_index = 0",
                    [&target_icon_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .unwrap();
        assert_ne!(target_preview, source_preview.to_string_lossy());
        assert_ne!(target_piece, source_piece.to_string_lossy());
        assert_eq!(
            std::fs::read(&target_preview).unwrap(),
            std::fs::read(&expected_preview.current_preview_path).unwrap()
        );
        assert_eq!(
            std::fs::read(&target_piece).unwrap(),
            std::fs::read(&expected_preview.piece_paths[0]).unwrap()
        );
        assert!(std::path::Path::new(&target_preview).starts_with(
            paths
                .ai_activation_previews_dir
                .join(&duplicated.id)
                .join(&target_icon_id)
                .join("native-clone")
        ));
        assert!(std::path::Path::new(&target_piece).is_file());
        assert_eq!(last_export, None);
        assert_eq!(export_status, "ready");
        assert_eq!(icon_kind, "placeholder");
        assert_eq!(readiness, "working");
        assert_eq!(placeholder_text.as_deref(), Some("finish me"));
        assert_eq!((quarter_turns, flip_horizontal, flip_vertical), (1, 1, 0));

        let source_text: (
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
                            text_overlay_stroke_width FROM icons WHERE id = ?1",
                [&source_icon_id],
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
        let target_text = connection
            .query_row(
                "SELECT text_overlay_enabled, text_overlay_text, text_overlay_font_path,
                        text_overlay_font_size, text_overlay_x, text_overlay_y,
                        text_overlay_color, text_overlay_stroke_color,
                        text_overlay_stroke_width FROM icons WHERE id = ?1",
                [&target_icon_id],
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
        assert_eq!(target_text, source_text);
        let target_note: String = connection
            .query_row(
                "SELECT note FROM icon_notes WHERE icon_id = ?1",
                [&target_icon_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(target_note, "collection note");
        let target_crop_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM crop_settings WHERE icon_id = ?1",
                [&target_icon_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(target_crop_count, 1);
        let target_recipe: (i64, String) = connection
            .query_row(
                "SELECT revision, effects_json FROM icon_effect_recipes WHERE icon_id = ?1",
                [&target_icon_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(target_recipe, (2, effects_json.to_string()));
        let target_motion: (String, i64, String) = connection
            .query_row(
                "SELECT recipe_schema, revision, motion_json
                 FROM icon_motion_recipes WHERE icon_id = ?1",
                [&target_icon_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(target_motion.0, "pmtcon-motion-v1");
        assert_eq!(target_motion.1, 4);
        assert_eq!(target_motion.2, motion_json);
        assert!(target_motion.2.contains("\"seed\":999"));

        let changed_motion_json = motion_json.replace("\"seed\":999", "\"seed\":1000");
        connection
            .execute(
                "UPDATE icon_motion_recipes
                 SET revision = 5, motion_json = ?1
                 WHERE icon_id = ?2",
                params![changed_motion_json, target_icon_id],
            )
            .unwrap();
        let source_motion: (i64, String) = connection
            .query_row(
                "SELECT revision, motion_json FROM icon_motion_recipes WHERE icon_id = ?1",
                [&source_icon_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        let changed_target_motion: (i64, String) = connection
            .query_row(
                "SELECT revision, motion_json FROM icon_motion_recipes WHERE icon_id = ?1",
                [&target_icon_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(source_motion, (4, motion_json.to_string()));
        assert_eq!(changed_target_motion.0, 5);
        assert!(changed_target_motion.1.contains("\"seed\":1000"));
        assert_eq!(
            duplicated.cover_icon_id.as_deref(),
            Some(target_icon_id.as_str())
        );
        for table in ["processed_asset_variants", "optimization_jobs"] {
            let count: i64 = connection
                .query_row(
                    &format!("SELECT COUNT(*) FROM {table} WHERE icon_id = ?1"),
                    [&target_icon_id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 0);
        }

        std::fs::remove_dir_all(&source_dir).unwrap();
        assert!(std::path::Path::new(&target_preview).is_file());
        assert!(std::path::Path::new(&target_piece).is_file());
        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn duplicate_collection_uses_numbered_copy_names() {
        let mut connection = connection();
        let paths = temp_paths();
        let created = create_collection(&mut connection, Some("Original".to_string())).unwrap();

        let first = duplicate_collection(&mut connection, &paths, &created.id).unwrap();
        let second = duplicate_collection(&mut connection, &paths, &created.id).unwrap();

        assert_eq!(first.name, "Original 복사본");
        assert_eq!(second.name, "Original 복사본 2");
        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn update_collection_settings_rejects_extreme_dimensions_before_db_mutation() {
        let mut connection = connection();
        let created = create_collection(&mut connection, Some("크기 제한".to_string())).unwrap();
        let before = created.clone();
        let error = update_collection_settings(
            &connection,
            &created.id,
            UpdateCollectionSettingsPayload {
                default_cell_width: i64::MAX,
                default_cell_height: 1,
                preview_width: 100,
                preview_height: 100,
                export_format: "png".to_string(),
                max_bytes: 1_000_000,
            },
        )
        .unwrap_err();
        assert_eq!(error.code, "validation");
        let after = list_collections(&connection)
            .unwrap()
            .into_iter()
            .find(|collection| collection.id == created.id)
            .unwrap();
        assert_eq!(after.default_cell_width, before.default_cell_width);
        assert_eq!(after.default_cell_height, before.default_cell_height);
    }

    #[test]
    fn update_collection_settings_persists_standard_sizes() {
        let mut connection = connection();
        let created = create_collection(&mut connection, Some("크기".to_string())).unwrap();

        let updated = update_collection_settings(
            &connection,
            &created.id,
            UpdateCollectionSettingsPayload {
                default_cell_width: 180,
                default_cell_height: 160,
                preview_width: 90,
                preview_height: 80,
                export_format: "png".to_string(),
                max_bytes: 1_000_000,
            },
        )
        .unwrap();

        assert_eq!(updated.default_cell_width, 180);
        assert_eq!(updated.default_cell_height, 160);
        assert_eq!(updated.preview_width, 90);
        assert_eq!(updated.preview_height, 80);
    }

    #[test]
    fn import_collection_cover_image_accepts_only_exact_200_png_or_jpg() {
        let mut connection = connection();
        let paths = temp_paths();
        let created = create_collection(&mut connection, Some("대표".to_string())).unwrap();

        let wrong_size = import_collection_cover_image(
            &mut connection,
            &paths,
            &created.id,
            ImportImageFilePayload {
                original_filename: "wrong.png".to_string(),
                bytes: png_bytes(100, 100),
            },
        );
        assert!(wrong_size.is_err());

        let updated = import_collection_cover_image(
            &mut connection,
            &paths,
            &created.id,
            ImportImageFilePayload {
                original_filename: "cover.png".to_string(),
                bytes: png_bytes(200, 200),
            },
        )
        .unwrap();

        assert!(updated.cover_source_file_id.is_some());
        assert_eq!(updated.cover_icon_id, None);
        assert!(updated.cover_image_url.is_some());

        std::fs::remove_dir_all(paths.root).unwrap();
    }
}
