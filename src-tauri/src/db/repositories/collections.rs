use std::collections::HashMap;

use rusqlite::{params, Connection, OptionalExtension, Row, Transaction};

use crate::db::repositories::source_files::{
    import_source_file_from_bytes, SourceFileImportOptions,
};
use crate::error::{AppError, AppResult};
use crate::ids::create_id;
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
           COALESCE(
             cover_icon.thumbnail_override_path,
             cover_icon.current_preview_path,
             cover_icon.thumbnail_path,
             cover_source.original_path_in_library,
             cover_icon_source.original_path_in_library
           ) AS cover_image_url
         FROM collections c
         LEFT JOIN icons cover_icon
           ON cover_icon.id = c.cover_icon_id
          AND cover_icon.deleted_at IS NULL
         LEFT JOIN source_files cover_source
           ON cover_source.id = c.cover_source_file_id
         LEFT JOIN source_files cover_icon_source
           ON cover_icon_source.id = cover_icon.source_file_id
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
               COALESCE(
                 cover_icon.thumbnail_override_path,
                 cover_icon.current_preview_path,
                 cover_icon.thumbnail_path,
                 cover_source.original_path_in_library,
                 cover_icon_source.original_path_in_library
               ) AS cover_image_url
             FROM collections c
             LEFT JOIN icons cover_icon
               ON cover_icon.id = c.cover_icon_id
              AND cover_icon.deleted_at IS NULL
             LEFT JOIN source_files cover_source
               ON cover_source.id = c.cover_source_file_id
             LEFT JOIN source_files cover_icon_source
               ON cover_icon_source.id = cover_icon.source_file_id
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
    collection_id: &str,
) -> AppResult<CollectionDto> {
    let transaction = connection.transaction()?;
    let original = get_collection(&transaction, collection_id)?;
    let duplicate_id = create_id("collection");
    let duplicate_name = format!("{} 복사본", original.name);
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
        params![duplicate_id, duplicate_name, order_index, collection_id],
    )?;

    duplicate_export_profiles(&transaction, collection_id, &duplicate_id)?;
    let icon_id_map = duplicate_icons(&transaction, collection_id, &duplicate_id)?;

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
                    duplicate_id
                ],
            )?;
        }
    }

    transaction.commit()?;

    get_collection(connection, &duplicate_id)
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
        return Err(AppError::not_found("설정을 저장할 모음을 찾을 수 없습니다."));
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
    if payload.default_cell_width <= 0
        || payload.default_cell_height <= 0
        || payload.preview_width <= 0
        || payload.preview_height <= 0
    {
        return Err(AppError::new(
            "validation",
            "모음 기준 크기와 표시 크기는 1px 이상이어야 합니다.",
        ));
    }

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
) -> AppResult<()> {
    let profiles = {
        let mut statement = transaction.prepare(
            "SELECT
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

    for profile in profiles {
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
                create_id("profile"),
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
    }

    Ok(())
}

#[derive(Debug)]
struct IconRecord {
    id: String,
    source_file_id: String,
    display_name: String,
    shape: String,
    order_index: i64,
    cell_width_override: Option<i64>,
    cell_height_override: Option<i64>,
    thumbnail_path: Option<String>,
    thumbnail_override_source_file_id: Option<String>,
    thumbnail_override_path: Option<String>,
    current_preview_path: Option<String>,
    gif_loop_mode: String,
    gif_loop_count: Option<i64>,
}

#[derive(Debug)]
struct IconPieceRecord {
    piece_index: i64,
    piece_role: String,
    alt_text: String,
    generated_preview_path: Option<String>,
    last_export_path: Option<String>,
    export_status: String,
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
    source_collection_id: &str,
    target_collection_id: &str,
) -> AppResult<HashMap<String, String>> {
    let icons = {
        let mut statement = transaction.prepare(
            "SELECT
               id,
               source_file_id,
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
               gif_loop_count
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
                    thumbnail_path: row.get("thumbnail_path")?,
                    thumbnail_override_source_file_id: row
                        .get("thumbnail_override_source_file_id")?,
                    thumbnail_override_path: row.get("thumbnail_override_path")?,
                    current_preview_path: row.get("current_preview_path")?,
                    gif_loop_mode: row.get("gif_loop_mode")?,
                    gif_loop_count: row.get("gif_loop_count")?,
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
               shape,
               order_index,
               cell_width_override,
               cell_height_override,
               thumbnail_path,
               thumbnail_override_source_file_id,
               thumbnail_override_path,
               current_preview_path,
               gif_loop_mode,
               gif_loop_count,
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
                duplicate_icon_id,
                target_collection_id,
                icon.source_file_id,
                icon.display_name,
                icon.shape,
                icon.order_index,
                icon.cell_width_override,
                icon.cell_height_override,
                icon.thumbnail_path,
                icon.thumbnail_override_source_file_id,
                icon.thumbnail_override_path,
                icon.current_preview_path,
                icon.gif_loop_mode,
                icon.gif_loop_count,
            ],
        )?;

        duplicate_icon_pieces(transaction, &icon.id, &duplicate_icon_id)?;
        duplicate_crop_settings(transaction, &icon.id, &duplicate_icon_id)?;
        icon_id_map.insert(icon.id, duplicate_icon_id);
    }

    Ok(icon_id_map)
}

fn duplicate_icon_pieces(
    transaction: &Transaction<'_>,
    source_icon_id: &str,
    target_icon_id: &str,
) -> AppResult<()> {
    let pieces = {
        let mut statement = transaction.prepare(
            "SELECT
               piece_index,
               piece_role,
               alt_text,
               generated_preview_path,
               last_export_path,
               export_status
             FROM icon_pieces
             WHERE icon_id = ?1
             ORDER BY piece_index ASC",
        )?;

        let pieces = statement
            .query_map(params![source_icon_id], |row| {
                Ok(IconPieceRecord {
                    piece_index: row.get("piece_index")?,
                    piece_role: row.get("piece_role")?,
                    alt_text: row.get("alt_text")?,
                    generated_preview_path: row.get("generated_preview_path")?,
                    last_export_path: row.get("last_export_path")?,
                    export_status: row.get("export_status")?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        pieces
    };

    for piece in pieces {
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
                create_id("piece"),
                target_icon_id,
                piece.piece_index,
                piece.piece_role,
                piece.alt_text,
                piece.generated_preview_path,
                piece.last_export_path,
                piece.export_status,
            ],
        )?;
    }

    Ok(())
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

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::time::{SystemTime, UNIX_EPOCH};

    use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};
    use rusqlite::Connection;

    use crate::db::migrations;
    use crate::models::{ImportImageFilePayload, UpdateCollectionSettingsPayload};
    use crate::paths::AppPaths;

    use super::{
        create_collection, delete_collection, duplicate_collection, list_collections,
        import_collection_cover_image, rename_collection, update_collection_settings,
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
        let created = create_collection(&mut connection, Some("원본".to_string())).unwrap();

        let duplicated = duplicate_collection(&mut connection, &created.id).unwrap();

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
