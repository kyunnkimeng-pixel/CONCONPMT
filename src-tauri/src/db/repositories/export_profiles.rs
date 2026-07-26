use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::error::{AppError, AppResult};
use crate::ids::create_id;
use crate::imaging::import_limits::validate_import_dimensions;
use crate::models::{ExportProfileDto, ExportRequestPayload};

pub fn list_export_profiles(
    connection: &Connection,
    collection_id: &str,
) -> AppResult<Vec<ExportProfileDto>> {
    ensure_collection_exists(connection, collection_id)?;
    ensure_default_profiles(connection, collection_id)?;

    let mut statement = connection.prepare(
        "SELECT
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
         FROM export_profiles
         WHERE collection_id = ?1
         ORDER BY
           CASE profile_type
             WHEN 'dcinside' THEN 0
             WHEN 'custom' THEN 1
             ELSE 2
           END,
           created_at ASC",
    )?;

    let profiles = statement
        .query_map(params![collection_id], profile_from_row)?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(profiles)
}

pub fn get_export_profile(
    connection: &Connection,
    collection_id: &str,
    profile_id: &str,
) -> AppResult<ExportProfileDto> {
    ensure_collection_exists(connection, collection_id)?;

    connection
        .query_row(
            "SELECT
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
             FROM export_profiles
             WHERE id = ?1
               AND collection_id = ?2",
            params![profile_id, collection_id],
            profile_from_row,
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("내보내기 프로필을 찾을 수 없습니다."))
}

pub fn update_export_profile_settings(
    connection: &Connection,
    collection_id: &str,
    payload: &ExportRequestPayload,
) -> AppResult<ExportProfileDto> {
    validate_profile_settings(payload)?;
    let profile = get_export_profile(connection, collection_id, &payload.profile_id)?;
    let allowed_formats_json = if profile.profile_type == "dcinside" {
        "[\"jpg\",\"jpeg\",\"png\",\"gif\"]".to_string()
    } else {
        profile.allowed_formats_as_json()
    };

    let changed = connection.execute(
        "UPDATE export_profiles
         SET target_format = ?1,
             target_cell_width = ?2,
             target_cell_height = ?3,
             max_bytes = ?4,
             filename_mode = ?5,
             include_alt_txt = ?6,
             strict_warnings = ?7,
             allowed_formats_json = ?8,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?9
           AND collection_id = ?10",
        params![
            normalized_target_format(&payload.target_format),
            payload.target_cell_width,
            payload.target_cell_height,
            payload.max_bytes,
            payload.filename_mode,
            bool_to_i64(payload.include_alt_txt),
            bool_to_i64(payload.strict_warnings),
            allowed_formats_json,
            payload.profile_id,
            collection_id,
        ],
    )?;

    if changed == 0 {
        return Err(AppError::not_found(
            "저장할 내보내기 프로필을 찾을 수 없습니다.",
        ));
    }

    get_export_profile(connection, collection_id, &payload.profile_id)
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
        Err(AppError::not_found("내보낼 모음을 찾을 수 없습니다."))
    }
}

fn ensure_default_profiles(connection: &Connection, collection_id: &str) -> AppResult<()> {
    let has_dcinside = profile_type_exists(connection, collection_id, "dcinside")?;
    if !has_dcinside {
        connection.execute(
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
            params![create_id("profile"), collection_id],
        )?;
    }

    let has_custom = profile_type_exists(connection, collection_id, "custom")?;
    if !has_custom {
        let custom_defaults = connection.query_row(
            "SELECT
               default_cell_width,
               default_cell_height,
               preview_width,
               preview_height,
               export_format,
               max_bytes,
               allowed_formats_json
             FROM collections
             WHERE id = ?1
               AND deleted_at IS NULL",
            params![collection_id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, String>(6)?,
                ))
            },
        )?;

        connection.execute(
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
               created_at,
               updated_at
             )
             VALUES (
               ?1,
               ?2,
               'Custom',
               'custom',
               ?3,
               ?4,
               ?5,
               ?6,
               ?7,
               ?8,
               ?9,
               strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
               strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             )",
            params![
                create_id("profile"),
                collection_id,
                normalized_target_format(&custom_defaults.4),
                custom_defaults.0,
                custom_defaults.1,
                custom_defaults.2,
                custom_defaults.3,
                custom_defaults.5,
                custom_defaults.6,
            ],
        )?;
    }

    Ok(())
}

fn profile_type_exists(
    connection: &Connection,
    collection_id: &str,
    profile_type: &str,
) -> AppResult<bool> {
    Ok(connection
        .query_row(
            "SELECT id
             FROM export_profiles
             WHERE collection_id = ?1
               AND profile_type = ?2
             LIMIT 1",
            params![collection_id, profile_type],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .is_some())
}

fn profile_from_row(row: &Row<'_>) -> rusqlite::Result<ExportProfileDto> {
    let allowed_formats_json: String = row.get("allowed_formats_json")?;
    let include_alt_txt: i64 = row.get("include_alt_txt")?;
    let strict_warnings: i64 = row.get("strict_warnings")?;

    Ok(ExportProfileDto {
        id: row.get("id")?,
        collection_id: row.get("collection_id")?,
        name: row.get("name")?,
        profile_type: row.get("profile_type")?,
        target_format: normalized_target_format(&row.get::<_, String>("target_format")?),
        target_cell_width: row.get("target_cell_width")?,
        target_cell_height: row.get("target_cell_height")?,
        preview_width: row.get("preview_width")?,
        preview_height: row.get("preview_height")?,
        max_bytes: row.get("max_bytes")?,
        allowed_formats: allowed_formats_from_json(&allowed_formats_json),
        filename_mode: row.get("filename_mode")?,
        include_alt_txt: include_alt_txt != 0,
        strict_warnings: strict_warnings != 0,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn validate_profile_settings(payload: &ExportRequestPayload) -> AppResult<()> {
    match payload.target_format.as_str() {
        "jpg" | "jpeg" | "png" | "gif" | "source" => {}
        _ => {
            return Err(AppError::new(
                "validation",
                "지원하지 않는 내보내기 형식입니다.",
            ));
        }
    }

    match payload.filename_mode.as_str() {
        "sequence" | "alt" => {}
        _ => {
            return Err(AppError::new(
                "validation",
                "지원하지 않는 파일명 방식입니다.",
            ));
        }
    }

    let target_width = u32::try_from(payload.target_cell_width)
        .map_err(|_| AppError::new("validation", "프로필 기준 너비가 올바르지 않습니다."))?;
    let target_height = u32::try_from(payload.target_cell_height)
        .map_err(|_| AppError::new("validation", "프로필 기준 높이가 올바르지 않습니다."))?;
    validate_import_dimensions(target_width, target_height)?;

    if payload.max_bytes <= 0 {
        return Err(AppError::new(
            "validation",
            "파일 용량 제한은 1바이트 이상이어야 합니다.",
        ));
    }

    Ok(())
}

fn allowed_formats_from_json(value: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(value)
        .unwrap_or_else(|_| {
            vec![
                "jpg".to_string(),
                "jpeg".to_string(),
                "png".to_string(),
                "gif".to_string(),
            ]
        })
        .into_iter()
        .map(|format| normalized_target_format(&format))
        .collect()
}

fn normalized_target_format(format: &str) -> String {
    match format.trim().to_ascii_lowercase().as_str() {
        "jpeg" => "jpg".to_string(),
        "jpg" => "jpg".to_string(),
        "gif" => "gif".to_string(),
        "source" => "source".to_string(),
        _ => "png".to_string(),
    }
}

fn bool_to_i64(value: bool) -> i64 {
    if value {
        1
    } else {
        0
    }
}

impl ExportProfileDto {
    fn allowed_formats_as_json(&self) -> String {
        serde_json::to_string(&self.allowed_formats)
            .unwrap_or_else(|_| "[\"jpg\",\"jpeg\",\"png\",\"gif\"]".to_string())
    }
}
#[cfg(test)]
mod tests {
    use crate::models::ExportRequestPayload;

    use super::validate_profile_settings;

    fn payload(width: i64, height: i64) -> ExportRequestPayload {
        ExportRequestPayload {
            profile_id: "profile_test".to_string(),
            target_format: "png".to_string(),
            target_cell_width: width,
            target_cell_height: height,
            max_bytes: 2_000_000,
            filename_mode: "sequence".to_string(),
            include_alt_txt: true,
            strict_warnings: false,
            output_directory: None,
            open_folder_after_export: false,
            open_alt_txt_after_export: false,
            excluded_piece_ids: Vec::new(),
            resize_filter: "lanczos3".to_string(),
        }
    }

    #[test]
    fn profile_settings_reject_extreme_dimensions_and_keep_normal_sizes() {
        assert!(validate_profile_settings(&payload(200, 200)).is_ok());
        assert!(validate_profile_settings(&payload(i64::MAX, 1)).is_err());
        assert!(validate_profile_settings(&payload(8_000, 8_000)).is_err());
    }
}
