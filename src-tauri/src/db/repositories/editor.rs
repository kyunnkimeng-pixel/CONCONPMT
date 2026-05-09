use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OptionalExtension, Transaction};

use crate::db::repositories::icons as icon_repository;
use crate::error::{AppError, AppResult};
use crate::ids::create_id;
use crate::imaging::geometry::{piece_roles, viewport_size};
use crate::imaging::preview::{
    generate_icon_preview, CropRect, GeneratePreviewRequest, GeneratedPreview,
};
use crate::models::{
    ApplyIconCropPayload, CropSettingsDto, IconDto, IconEditorStateDto, SourceFileDto,
};
use crate::paths::AppPaths;

pub fn get_icon_editor_state(
    connection: &Connection,
    collection_id: &str,
    icon_id: &str,
) -> AppResult<IconEditorStateDto> {
    let icon = icon_repository::get_icon(connection, collection_id, icon_id)?;
    let source = source_file_for_icon(connection, collection_id, icon_id)?;
    let crop = crop_settings_for_icon(connection, icon_id)?;

    Ok(IconEditorStateDto { icon, source, crop })
}

pub fn apply_icon_crop(
    connection: &mut Connection,
    paths: &AppPaths,
    collection_id: &str,
    payload: ApplyIconCropPayload,
) -> AppResult<IconDto> {
    validate_apply_payload(&payload)?;
    let apply_record = apply_record_for_icon(connection, collection_id, &payload.icon_id)?;
    let viewport = viewport_size(&payload.shape, payload.cell_width, payload.cell_height)?;
    let source_path = PathBuf::from(&apply_record.original_path_in_library);

    let preview = generate_icon_preview(
        paths,
        GeneratePreviewRequest {
            collection_id,
            icon_id: &payload.icon_id,
            source_path: &source_path,
            source_extension: &apply_record.original_extension,
            shape: &payload.shape,
            crop: CropRect {
                x: payload.crop_x,
                y: payload.crop_y,
                width: payload.crop_w,
                height: payload.crop_h,
            },
            cell_width: payload.cell_width,
            cell_height: payload.cell_height,
            gif_loop_mode: &payload.gif_loop_mode,
            gif_loop_count: payload.gif_loop_count,
            source_gif_loop_mode: Some(&apply_record.original_loop_mode),
            source_gif_loop_count: apply_record.original_loop_count,
        },
    )?;
    validate_generated_piece_outputs(&preview, apply_record.max_bytes)?;

    let transaction = connection.transaction()?;
    ensure_icon_still_editable(&transaction, collection_id, &payload.icon_id)?;
    update_icon_record(
        &transaction,
        collection_id,
        &payload,
        &apply_record,
        preview.current_preview_path.to_string_lossy().as_ref(),
    )?;
    upsert_crop_settings(
        &transaction,
        &payload,
        apply_record.source_width,
        apply_record.source_height,
        viewport.width,
        viewport.height,
    )?;
    reconcile_icon_pieces(&transaction, collection_id, &payload, &preview.piece_paths)?;
    transaction.commit()?;

    icon_repository::get_icon(connection, collection_id, &payload.icon_id)
}

#[derive(Debug)]
struct ApplyRecord {
    source_width: i64,
    source_height: i64,
    original_path_in_library: String,
    original_extension: String,
    original_loop_mode: String,
    original_loop_count: Option<i64>,
    default_cell_width: i64,
    default_cell_height: i64,
    max_bytes: i64,
}

#[derive(Debug)]
struct PieceRecord {
    id: String,
    piece_index: i64,
    alt_text: String,
}

fn validate_apply_payload(payload: &ApplyIconCropPayload) -> AppResult<()> {
    match payload.shape.as_str() {
        "single" | "horizontal_double" | "vertical_double" => {}
        _ => {
            return Err(AppError::new(
                "validation",
                "지원하지 않는 아이콘 모양입니다.",
            ));
        }
    }

    match payload.crop_mode.as_str() {
        "free" | "fixed" => {}
        _ => {
            return Err(AppError::new(
                "validation",
                "지원하지 않는 크롭 모드입니다.",
            ));
        }
    }

    match payload.preset_position.as_str() {
        "center" | "top_left" | "top" | "top_right" | "left" | "right" | "bottom_left"
        | "bottom" | "bottom_right" | "custom" => {}
        _ => {
            return Err(AppError::new(
                "validation",
                "지원하지 않는 크롭 위치입니다.",
            ));
        }
    }

    match payload.gif_loop_mode.as_str() {
        "preserve" | "infinite" | "once" | "count" => {}
        _ => {
            return Err(AppError::new(
                "validation",
                "지원하지 않는 GIF 반복 설정입니다.",
            ));
        }
    }

    if payload.gif_loop_mode == "count" && payload.gif_loop_count.unwrap_or(0) <= 0 {
        return Err(AppError::new(
            "validation",
            "사용자 지정 반복 횟수는 1 이상이어야 합니다.",
        ));
    }

    if payload.cell_width <= 0 || payload.cell_height <= 0 {
        return Err(AppError::new(
            "validation",
            "셀 크기는 1px 이상이어야 합니다.",
        ));
    }

    if payload.crop_w <= 0.0 || payload.crop_h <= 0.0 {
        return Err(AppError::new(
            "validation",
            "크롭 영역은 1px 이상이어야 합니다.",
        ));
    }

    Ok(())
}

fn apply_record_for_icon(
    connection: &Connection,
    collection_id: &str,
    icon_id: &str,
) -> AppResult<ApplyRecord> {
    connection
        .query_row(
            "SELECT
               s.width,
               s.height,
               s.original_path_in_library,
               s.original_extension,
               COALESCE(s.original_loop_mode, 'preserve') AS original_loop_mode,
               s.original_loop_count,
               c.default_cell_width,
               c.default_cell_height,
               c.max_bytes
             FROM icons i
             JOIN source_files s ON s.id = i.source_file_id
             JOIN collections c ON c.id = i.collection_id
             WHERE i.id = ?1
               AND i.collection_id = ?2
               AND i.deleted_at IS NULL
               AND c.deleted_at IS NULL",
            params![icon_id, collection_id],
            |row| {
                Ok(ApplyRecord {
                    source_width: row.get("width")?,
                    source_height: row.get("height")?,
                    original_path_in_library: row.get("original_path_in_library")?,
                    original_extension: row.get("original_extension")?,
                    original_loop_mode: row.get("original_loop_mode")?,
                    original_loop_count: row.get("original_loop_count")?,
                    default_cell_width: row.get("default_cell_width")?,
                    default_cell_height: row.get("default_cell_height")?,
                    max_bytes: row.get("max_bytes")?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("편집할 아이콘을 찾을 수 없습니다."))
}

fn validate_generated_piece_outputs(preview: &GeneratedPreview, max_bytes: i64) -> AppResult<()> {
    let max_bytes = u64::try_from(max_bytes.max(1)).unwrap_or(u64::MAX);

    for path in &preview.piece_paths {
        validate_generated_piece_format(path)?;
        let byte_size = fs::metadata(path)?.len();
        if byte_size > max_bytes {
            return Err(AppError::new(
                "validation",
                format!(
                    "처리된 이미지가 모음 용량 제한 {}를 초과했습니다.",
                    format_bytes(max_bytes),
                ),
            ));
        }
    }

    Ok(())
}

fn validate_generated_piece_format(path: &Path) -> AppResult<()> {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    match extension.as_str() {
        "png" | "gif" => Ok(()),
        _ => Err(AppError::new(
            "validation",
            "처리된 미리보기는 png 또는 gif 형식이어야 합니다.",
        )),
    }
}

fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{bytes}B");
    }

    if bytes < 1024 * 1024 {
        return format!("{:.1}KB", bytes as f64 / 1024.0);
    }

    format!("{:.1}MB", bytes as f64 / (1024.0 * 1024.0))
}

fn source_file_for_icon(
    connection: &Connection,
    collection_id: &str,
    icon_id: &str,
) -> AppResult<SourceFileDto> {
    connection
        .query_row(
            "SELECT
               s.id,
               s.original_filename,
               s.original_path_in_library,
               s.mime_type,
               s.width,
               s.height,
               s.byte_size,
               s.is_animated,
               s.frame_count,
               COALESCE(s.original_loop_mode, 'preserve') AS original_loop_mode,
               s.original_loop_count
             FROM source_files s
             JOIN icons i ON i.source_file_id = s.id
             WHERE i.id = ?1
               AND i.collection_id = ?2
               AND i.deleted_at IS NULL",
            params![icon_id, collection_id],
            |row| {
                let is_animated: i64 = row.get("is_animated")?;
                Ok(SourceFileDto {
                    id: row.get("id")?,
                    original_filename: row.get("original_filename")?,
                    original_image_url: row.get("original_path_in_library")?,
                    mime_type: row.get("mime_type")?,
                    width: row.get("width")?,
                    height: row.get("height")?,
                    byte_size: row.get("byte_size")?,
                    is_animated: is_animated != 0,
                    frame_count: row.get("frame_count")?,
                    original_loop_mode: row.get("original_loop_mode")?,
                    original_loop_count: row.get("original_loop_count")?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("원본 이미지를 찾을 수 없습니다."))
}

fn crop_settings_for_icon(connection: &Connection, icon_id: &str) -> AppResult<CropSettingsDto> {
    connection
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
               viewport_height_at_apply,
               updated_at
             FROM crop_settings
             WHERE icon_id = ?1",
            params![icon_id],
            |row| {
                Ok(CropSettingsDto {
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
                    updated_at: row.get("updated_at")?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("크롭 설정을 찾을 수 없습니다."))
}

fn ensure_icon_still_editable(
    transaction: &Transaction<'_>,
    collection_id: &str,
    icon_id: &str,
) -> AppResult<()> {
    let exists = transaction
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
        Err(AppError::not_found("편집할 아이콘을 찾을 수 없습니다."))
    }
}

fn update_icon_record(
    transaction: &Transaction<'_>,
    collection_id: &str,
    payload: &ApplyIconCropPayload,
    apply_record: &ApplyRecord,
    current_preview_path: &str,
) -> AppResult<()> {
    let cell_width_override = if payload.cell_width == apply_record.default_cell_width {
        None
    } else {
        Some(payload.cell_width)
    };
    let cell_height_override = if payload.cell_height == apply_record.default_cell_height {
        None
    } else {
        Some(payload.cell_height)
    };
    let gif_loop_count = if payload.gif_loop_mode == "count" {
        payload.gif_loop_count
    } else {
        None
    };

    transaction.execute(
        "UPDATE icons
         SET shape = ?1,
             cell_width_override = ?2,
             cell_height_override = ?3,
             current_preview_path = ?4,
             gif_loop_mode = ?5,
             gif_loop_count = ?6,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?7
           AND collection_id = ?8
           AND deleted_at IS NULL",
        params![
            payload.shape,
            cell_width_override,
            cell_height_override,
            current_preview_path,
            payload.gif_loop_mode,
            gif_loop_count,
            payload.icon_id,
            collection_id,
        ],
    )?;
    transaction.execute(
        "UPDATE collections
         SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?1
           AND cover_icon_id = ?2
           AND deleted_at IS NULL",
        params![collection_id, payload.icon_id],
    )?;

    Ok(())
}

fn upsert_crop_settings(
    transaction: &Transaction<'_>,
    payload: &ApplyIconCropPayload,
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
         )
         ON CONFLICT(icon_id) DO UPDATE SET
           crop_mode = excluded.crop_mode,
           crop_x = excluded.crop_x,
           crop_y = excluded.crop_y,
           crop_w = excluded.crop_w,
           crop_h = excluded.crop_h,
           preset_position = excluded.preset_position,
           source_width_at_apply = excluded.source_width_at_apply,
           source_height_at_apply = excluded.source_height_at_apply,
           viewport_width_at_apply = excluded.viewport_width_at_apply,
           viewport_height_at_apply = excluded.viewport_height_at_apply,
           updated_at = excluded.updated_at",
        params![
            create_id("crop"),
            payload.icon_id,
            payload.crop_mode,
            payload.crop_x,
            payload.crop_y,
            payload.crop_w,
            payload.crop_h,
            payload.preset_position,
            source_width,
            source_height,
            viewport_width,
            viewport_height,
        ],
    )?;

    Ok(())
}

fn reconcile_icon_pieces(
    transaction: &Transaction<'_>,
    collection_id: &str,
    payload: &ApplyIconCropPayload,
    piece_paths: &[PathBuf],
) -> AppResult<()> {
    let roles = piece_roles(&payload.shape)?;
    if roles.len() != piece_paths.len() {
        return Err(AppError::new(
            "validation",
            "아이콘 조각 수와 생성된 미리보기 수가 일치하지 않습니다.",
        ));
    }

    let existing_pieces = pieces_for_icon(transaction, &payload.icon_id)?;
    let mut used_alt_texts = collection_alt_texts(transaction, collection_id, &payload.icon_id)?;
    for piece in &existing_pieces {
        used_alt_texts.insert(piece.alt_text.clone());
    }

    for (piece_index, role) in roles.iter().enumerate() {
        let path = piece_paths[piece_index].to_string_lossy().to_string();

        if let Some(existing) = existing_pieces
            .iter()
            .find(|piece| piece.piece_index == piece_index as i64)
        {
            transaction.execute(
                "UPDATE icon_pieces
                 SET piece_role = ?1,
                     generated_preview_path = ?2,
                     export_status = 'ready',
                     updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE id = ?3",
                params![role, path, existing.id],
            )?;
        } else {
            let alt_text = next_unique_alt(&mut used_alt_texts, role);
            transaction.execute(
                "INSERT INTO icon_pieces (
                   id,
                   icon_id,
                   piece_index,
                   piece_role,
                   alt_text,
                   generated_preview_path,
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
                   'ready',
                   strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                   strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 )",
                params![
                    create_id("piece"),
                    payload.icon_id,
                    piece_index as i64,
                    role,
                    alt_text,
                    path,
                ],
            )?;
        }
    }

    transaction.execute(
        "DELETE FROM icon_pieces
         WHERE icon_id = ?1
           AND piece_index >= ?2",
        params![payload.icon_id, roles.len() as i64],
    )?;

    Ok(())
}

fn pieces_for_icon(transaction: &Transaction<'_>, icon_id: &str) -> AppResult<Vec<PieceRecord>> {
    let mut statement = transaction.prepare(
        "SELECT id, piece_index, alt_text
         FROM icon_pieces
         WHERE icon_id = ?1
         ORDER BY piece_index ASC",
    )?;

    let pieces = statement
        .query_map(params![icon_id], |row| {
            Ok(PieceRecord {
                id: row.get("id")?,
                piece_index: row.get("piece_index")?,
                alt_text: row.get("alt_text")?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(pieces)
}

fn collection_alt_texts(
    transaction: &Transaction<'_>,
    collection_id: &str,
    edited_icon_id: &str,
) -> AppResult<HashSet<String>> {
    let mut statement = transaction.prepare(
        "SELECT p.alt_text
         FROM icon_pieces p
         JOIN icons i ON i.id = p.icon_id
         WHERE i.collection_id = ?1
           AND i.id <> ?2
           AND i.deleted_at IS NULL",
    )?;

    let values = statement
        .query_map(params![collection_id, edited_icon_id], |row| {
            row.get::<_, String>(0)
        })?
        .collect::<Result<HashSet<_>, _>>()?;

    Ok(values)
}

fn next_unique_alt(used_alt_texts: &mut HashSet<String>, role: &str) -> String {
    let preferred = match role {
        "left" => ["좌", "왼", "가"],
        "right" => ["우", "오", "나"],
        "top" => ["상", "위", "가"],
        "bottom" => ["하", "아", "나"],
        _ => ["가", "나", "다"],
    };

    for candidate in preferred {
        if !used_alt_texts.contains(candidate) {
            used_alt_texts.insert(candidate.to_string());
            return candidate.to_string();
        }
    }

    for character in [
        "다", "라", "마", "바", "사", "자", "차", "카", "타", "파", "A", "B", "C",
    ] {
        if !used_alt_texts.contains(character) {
            used_alt_texts.insert(character.to_string());
            return character.to_string();
        }
    }

    "가".to_string()
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};
    use rusqlite::Connection;

    use crate::db::migrations;
    use crate::db::repositories::collections::create_collection;
    use crate::db::repositories::imports::import_image_files;
    use crate::models::{ApplyIconCropPayload, ImportImageFilePayload};
    use crate::paths::AppPaths;

    use super::{apply_icon_crop, get_icon_editor_state};

    #[derive(Debug)]
    struct GifSummary {
        repeat: gif::Repeat,
        frame_sizes: Vec<(u16, u16)>,
        delays: Vec<u16>,
    }

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
        AppPaths::prepare(std::env::temp_dir().join(format!("pmtconcon-editor-{suffix}"))).unwrap()
    }

    fn png_bytes() -> Vec<u8> {
        let image = ImageBuffer::from_pixel(40, 20, Rgba([0, 255, 0, 255]));
        let mut cursor = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image)
            .write_to(&mut cursor, ImageFormat::Png)
            .unwrap();
        cursor.into_inner()
    }

    fn animated_gif_bytes(repeat: gif::Repeat) -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut encoder = gif::Encoder::new(&mut bytes, 12, 8, &[]).unwrap();
            encoder.set_repeat(repeat).unwrap();

            for (color, delay) in [([255, 0, 0, 255], 5_u16), ([0, 0, 255, 255], 7_u16)] {
                let mut pixels = Vec::with_capacity(12 * 8 * 4);
                for _ in 0..(12 * 8) {
                    pixels.extend_from_slice(&color);
                }
                let mut frame = gif::Frame::from_rgba_speed(12, 8, &mut pixels, 10);
                frame.delay = delay;
                encoder.write_frame(&frame).unwrap();
            }
        }

        bytes
    }

    fn gif_summary(path: &Path) -> GifSummary {
        let mut options = gif::DecodeOptions::new();
        options.set_color_output(gif::ColorOutput::RGBA);
        let file = std::fs::File::open(path).unwrap();
        let mut reader = options.read_info(file).unwrap();
        let repeat = reader.repeat();
        let mut frame_sizes = Vec::new();
        let mut delays = Vec::new();

        while let Some(frame) = reader.read_next_frame().unwrap() {
            frame_sizes.push((frame.width, frame.height));
            delays.push(frame.delay);
        }

        GifSummary {
            repeat,
            frame_sizes,
            delays,
        }
    }

    #[test]
    fn apply_crop_updates_metadata_and_generates_preview_without_touching_original() {
        let mut connection = connection();
        let paths = temp_paths();
        let collection =
            create_collection(&mut connection, Some("편집 테스트".to_string())).unwrap();
        let imported = import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![ImportImageFilePayload {
                original_filename: "source.png".to_string(),
                bytes: png_bytes(),
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
        assert!(std::path::Path::new(&original_path).exists());

        let updated = apply_icon_crop(
            &mut connection,
            &paths,
            &collection.id,
            ApplyIconCropPayload {
                icon_id: icon_id.clone(),
                shape: "horizontal_double".to_string(),
                crop_mode: "fixed".to_string(),
                crop_x: 0.0,
                crop_y: 0.0,
                crop_w: 40.0,
                crop_h: 20.0,
                preset_position: "center".to_string(),
                cell_width: 20,
                cell_height: 20,
                gif_loop_mode: "preserve".to_string(),
                gif_loop_count: None,
            },
        )
        .unwrap();

        assert_eq!(updated.shape, "horizontal_double");
        assert_eq!(updated.pieces.len(), 2);
        assert_eq!(updated.pieces[0].piece_role, "left");
        assert_eq!(updated.pieces[1].piece_role, "right");
        assert!(updated.current_preview_url.is_some());
        assert!(std::path::Path::new(updated.current_preview_url.as_ref().unwrap()).exists());
        assert!(std::path::Path::new(&original_path).exists());

        let state = get_icon_editor_state(&connection, &collection.id, &icon_id).unwrap();
        assert_eq!(state.crop.crop_mode, "fixed");
        assert_eq!(state.crop.viewport_width_at_apply, 40);
        assert_eq!(state.crop.viewport_height_at_apply, 20);

        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn apply_gif_crop_generates_animated_preview_with_loop_metadata() {
        let mut connection = connection();
        let paths = temp_paths();
        let collection =
            create_collection(&mut connection, Some("GIF 편집 테스트".to_string())).unwrap();
        let source_bytes = animated_gif_bytes(gif::Repeat::Infinite);
        let imported = import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![ImportImageFilePayload {
                original_filename: "source.gif".to_string(),
                bytes: source_bytes.clone(),
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

        let updated = apply_icon_crop(
            &mut connection,
            &paths,
            &collection.id,
            ApplyIconCropPayload {
                icon_id: icon_id.clone(),
                shape: "horizontal_double".to_string(),
                crop_mode: "fixed".to_string(),
                crop_x: 0.0,
                crop_y: 0.0,
                crop_w: 12.0,
                crop_h: 8.0,
                preset_position: "center".to_string(),
                cell_width: 6,
                cell_height: 8,
                gif_loop_mode: "count".to_string(),
                gif_loop_count: Some(2),
            },
        )
        .unwrap();

        let preview_path = Path::new(updated.current_preview_url.as_ref().unwrap());
        let preview = gif_summary(preview_path);
        assert_eq!(preview.repeat, gif::Repeat::Finite(2));
        assert_eq!(preview.frame_sizes, vec![(12, 8), (12, 8)]);
        assert_eq!(preview.delays, vec![5, 7]);

        let piece_path: String = connection
            .query_row(
                "SELECT generated_preview_path
                 FROM icon_pieces
                 WHERE icon_id = ?1
                   AND piece_index = 0",
                [&icon_id],
                |row| row.get(0),
            )
            .unwrap();
        let piece = gif_summary(Path::new(&piece_path));
        assert_eq!(piece.repeat, gif::Repeat::Finite(2));
        assert_eq!(piece.frame_sizes, vec![(6, 8), (6, 8)]);

        assert_eq!(std::fs::read(original_path).unwrap(), source_bytes);

        std::fs::remove_dir_all(paths.root).unwrap();
    }
}
