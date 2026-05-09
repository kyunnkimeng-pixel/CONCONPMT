use std::collections::HashSet;

use rusqlite::{params, Connection, OptionalExtension, Row, Transaction};

use crate::error::{AppError, AppResult};
use crate::ids::create_id;
use crate::models::{IconDto, IconPieceDto};

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
           shape,
           order_index,
           cell_width_override,
           cell_height_override,
           thumbnail_path,
           current_preview_path,
           gif_loop_mode,
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
    validate_alt_text(&normalized_alt)?;

    let icon_id = icon_id_for_piece(connection, collection_id, piece_id)?;
    ensure_unique_alt(connection, collection_id, piece_id, &normalized_alt)?;

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

pub fn duplicate_icon(
    connection: &mut Connection,
    collection_id: &str,
    icon_id: &str,
) -> AppResult<IconDto> {
    let transaction = connection.transaction()?;
    ensure_collection_exists(&transaction, collection_id)?;
    let icon = icon_record_for_duplicate(&transaction, collection_id, icon_id)?;
    let duplicate_icon_id = create_id("icon");
    let order_index = next_icon_order_index(&transaction, collection_id)?;
    let duplicate_name = format!("{} 복사본", icon.display_name);

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
           strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
           strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         )",
        params![
            duplicate_icon_id,
            collection_id,
            icon.source_file_id,
            duplicate_name,
            icon.shape,
            order_index,
            icon.cell_width_override,
            icon.cell_height_override,
            icon.thumbnail_path,
            icon.current_preview_path,
            icon.gif_loop_mode,
            icon.gif_loop_count,
        ],
    )?;

    duplicate_icon_pieces(&transaction, collection_id, icon_id, &duplicate_icon_id)?;
    duplicate_crop_settings(&transaction, icon_id, &duplicate_icon_id)?;
    transaction.commit()?;

    get_icon(connection, collection_id, &duplicate_icon_id)
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

fn icon_from_row(connection: &Connection, row: &Row<'_>) -> rusqlite::Result<IconDto> {
    let icon_id: String = row.get("id")?;

    Ok(IconDto {
        pieces: list_pieces(connection, &icon_id)?,
        id: icon_id,
        collection_id: row.get("collection_id")?,
        source_file_id: row.get("source_file_id")?,
        display_name: row.get("display_name")?,
        shape: row.get("shape")?,
        order_index: row.get("order_index")?,
        cell_width_override: row.get("cell_width_override")?,
        cell_height_override: row.get("cell_height_override")?,
        thumbnail_url: row.get("thumbnail_path")?,
        current_preview_url: row.get("current_preview_path")?,
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
               shape,
               order_index,
               cell_width_override,
               cell_height_override,
               thumbnail_path,
               current_preview_path,
               gif_loop_mode,
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

fn ensure_unique_alt(
    connection: &Connection,
    collection_id: &str,
    piece_id: &str,
    alt_text: &str,
) -> AppResult<()> {
    let duplicate_exists = connection
        .query_row(
            "SELECT p.id
             FROM icon_pieces p
             JOIN icons i ON i.id = p.icon_id
             WHERE i.collection_id = ?1
               AND i.deleted_at IS NULL
               AND p.id <> ?2
               AND p.alt_text = ?3
             LIMIT 1",
            params![collection_id, piece_id, alt_text],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .is_some();

    if duplicate_exists {
        Err(AppError::new(
            "validation",
            "같은 모음 안에서 alt 값은 중복될 수 없습니다.",
        ))
    } else {
        Ok(())
    }
}

#[derive(Debug)]
struct IconDuplicateRecord {
    source_file_id: String,
    display_name: String,
    shape: String,
    cell_width_override: Option<i64>,
    cell_height_override: Option<i64>,
    thumbnail_path: Option<String>,
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
               shape,
               cell_width_override,
               cell_height_override,
               thumbnail_path,
               current_preview_path,
               gif_loop_mode,
               gif_loop_count
             FROM icons
             WHERE id = ?1
               AND collection_id = ?2
               AND deleted_at IS NULL",
            params![icon_id, collection_id],
            |row| {
                Ok(IconDuplicateRecord {
                    source_file_id: row.get("source_file_id")?,
                    display_name: row.get("display_name")?,
                    shape: row.get("shape")?,
                    cell_width_override: row.get("cell_width_override")?,
                    cell_height_override: row.get("cell_height_override")?,
                    thumbnail_path: row.get("thumbnail_path")?,
                    current_preview_path: row.get("current_preview_path")?,
                    gif_loop_mode: row.get("gif_loop_mode")?,
                    gif_loop_count: row.get("gif_loop_count")?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("복제할 아이콘을 찾을 수 없습니다."))
}

fn duplicate_icon_pieces(
    transaction: &Transaction<'_>,
    collection_id: &str,
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
    let mut used_alt_texts = collection_alt_texts(transaction, collection_id)?;

    for piece in pieces {
        let duplicate_alt_text = unique_duplicate_alt_text(&mut used_alt_texts, &piece.alt_text);
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
                duplicate_alt_text,
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

fn collection_alt_texts(
    connection: &Connection,
    collection_id: &str,
) -> AppResult<HashSet<String>> {
    let mut statement = connection.prepare(
        "SELECT p.alt_text
         FROM icon_pieces p
         JOIN icons i ON i.id = p.icon_id
         WHERE i.collection_id = ?1
           AND i.deleted_at IS NULL",
    )?;

    let values = statement
        .query_map(params![collection_id], |row| row.get::<_, String>(0))?
        .collect::<Result<HashSet<_>, _>>()?;

    Ok(values)
}

fn unique_duplicate_alt_text(used_alt_texts: &mut HashSet<String>, original: &str) -> String {
    let prefix = original
        .chars()
        .find(|character| is_allowed_alt_character(*character))
        .unwrap_or('복');

    for number in 1..=1295 {
        let candidate = format!("{prefix}{}", base36(number));
        if validate_alt_text(&candidate).is_ok() && !used_alt_texts.contains(&candidate) {
            used_alt_texts.insert(candidate.clone());
            return candidate;
        }
    }

    for number in 1..=999 {
        let candidate = format!("복{}", base36(number));
        if validate_alt_text(&candidate).is_ok() && !used_alt_texts.contains(&candidate) {
            used_alt_texts.insert(candidate.clone());
            return candidate;
        }
    }

    "복".to_string()
}

fn base36(mut number: usize) -> String {
    const DIGITS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut output = Vec::new();

    loop {
        output.push(DIGITS[number % 36] as char);
        number /= 36;
        if number == 0 {
            break;
        }
    }

    output.iter().rev().collect()
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

fn normalized_alt_text(alt_text: String) -> String {
    alt_text.trim().to_string()
}

fn validate_alt_text(alt_text: &str) -> AppResult<()> {
    let character_count = alt_text.chars().count();
    if !(1..=3).contains(&character_count) {
        return Err(AppError::new(
            "validation",
            "alt 값은 한글 기준 1~3글자여야 합니다.",
        ));
    }

    if !alt_text.chars().all(is_allowed_alt_character) {
        return Err(AppError::new(
            "validation",
            "alt 값에는 한글, 영문, 숫자, * ^ ! ~ + 만 사용할 수 있습니다.",
        ));
    }

    Ok(())
}

fn is_allowed_alt_character(character: char) -> bool {
    character.is_ascii_alphanumeric()
        || matches!(character, '*' | '^' | '!' | '~' | '+')
        || matches!(
            character as u32,
            0x1100..=0x11FF | 0x3130..=0x318F | 0xAC00..=0xD7A3
        )
}

#[cfg(test)]
mod tests {
    use rusqlite::{params, Connection};

    use crate::db::migrations;
    use crate::db::repositories::collections::create_collection;
    use crate::ids::create_id;

    use super::{delete_icons, duplicate_icon, list_icons, reorder_icons, update_icon_piece_alt};

    fn connection() -> Connection {
        let mut connection = Connection::open_in_memory().unwrap();
        migrations::run(&mut connection).unwrap();
        connection
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
                   20,
                   ?4,
                   strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 )",
                params![
                    source_file_id,
                    format!("{icon_id}.png"),
                    format!("C:/tmp/{icon_id}.png"),
                    icon_id,
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

        (icon_id, piece_id)
    }

    #[test]
    fn update_icon_piece_alt_rejects_duplicates() {
        let mut connection = connection();
        let collection =
            create_collection(&mut connection, Some("alt 테스트".to_string())).unwrap();
        let (_first_icon_id, _first_piece_id) = seed_icon(&connection, &collection.id, 0, "가");
        let (_second_icon_id, second_piece_id) = seed_icon(&connection, &collection.id, 1, "나");

        let duplicate = update_icon_piece_alt(
            &connection,
            &collection.id,
            &second_piece_id,
            "가".to_string(),
        );
        assert!(duplicate.is_err());

        let updated = update_icon_piece_alt(
            &connection,
            &collection.id,
            &second_piece_id,
            "다".to_string(),
        )
        .unwrap();
        assert_eq!(updated.pieces[0].alt_text, "다");
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
    fn duplicate_icon_creates_new_order_and_unique_alt() {
        let mut connection = connection();
        let collection =
            create_collection(&mut connection, Some("복제 테스트".to_string())).unwrap();
        let (icon_id, _) = seed_icon(&connection, &collection.id, 0, "가");

        let duplicated = duplicate_icon(&mut connection, &collection.id, &icon_id).unwrap();

        assert_ne!(duplicated.id, icon_id);
        assert_eq!(duplicated.order_index, 1);
        assert_ne!(duplicated.pieces[0].alt_text, "가");
        assert_eq!(list_icons(&connection, &collection.id).unwrap().len(), 2);
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
}
