use rusqlite::{params, Connection, OptionalExtension};

use crate::error::{AppError, AppResult};
use crate::models::{AppSettingsDto, SaveAppSettingsPayload};

pub fn get_app_settings(connection: &Connection) -> AppResult<AppSettingsDto> {
    ensure_settings_row(connection)?;

    connection
        .query_row(
            "SELECT last_open_collection_id, last_view_mode
             FROM app_settings
             WHERE id = 1",
            [],
            |row| {
                Ok(AppSettingsDto {
                    last_open_collection_id: row.get("last_open_collection_id")?,
                    last_view_mode: row.get("last_view_mode")?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("앱 설정을 찾을 수 없습니다."))
}

pub fn save_app_settings(
    connection: &Connection,
    payload: SaveAppSettingsPayload,
) -> AppResult<AppSettingsDto> {
    validate_view_mode(&payload.last_view_mode)?;
    ensure_settings_row(connection)?;

    let collection_id = match payload.last_open_collection_id {
        Some(collection_id) if collection_exists(connection, &collection_id)? => {
            Some(collection_id)
        }
        _ => None,
    };

    connection.execute(
        "UPDATE app_settings
         SET last_open_collection_id = ?1,
             last_view_mode = ?2,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = 1",
        params![collection_id, payload.last_view_mode],
    )?;

    get_app_settings(connection)
}

fn ensure_settings_row(connection: &Connection) -> AppResult<()> {
    connection.execute(
        "INSERT OR IGNORE INTO app_settings (id, created_at, updated_at)
         VALUES (
           1,
           strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
           strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         )",
        [],
    )?;

    Ok(())
}

fn collection_exists(connection: &Connection, collection_id: &str) -> AppResult<bool> {
    Ok(connection
        .query_row(
            "SELECT id
             FROM collections
             WHERE id = ?1
               AND deleted_at IS NULL",
            [collection_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .is_some())
}

fn validate_view_mode(value: &str) -> AppResult<()> {
    match value {
        "explorer" | "usagePreview" => Ok(()),
        _ => Err(AppError::new(
            "validation",
            "지원하지 않는 보기 모드입니다.",
        )),
    }
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use crate::db::migrations;
    use crate::db::repositories::collections::create_collection;
    use crate::models::SaveAppSettingsPayload;

    use super::{get_app_settings, save_app_settings};

    fn connection() -> Connection {
        let mut connection = Connection::open_in_memory().unwrap();
        migrations::run(&mut connection).unwrap();
        connection
    }

    #[test]
    fn save_app_settings_persists_last_collection_and_view() {
        let mut connection = connection();
        let collection =
            create_collection(&mut connection, Some("복구 테스트".to_string())).unwrap();

        let saved = save_app_settings(
            &connection,
            SaveAppSettingsPayload {
                last_open_collection_id: Some(collection.id.clone()),
                last_view_mode: "usagePreview".to_string(),
            },
        )
        .unwrap();

        assert_eq!(saved.last_open_collection_id, Some(collection.id));
        assert_eq!(saved.last_view_mode, "usagePreview");
        assert_eq!(
            get_app_settings(&connection).unwrap().last_view_mode,
            "usagePreview"
        );
    }

    #[test]
    fn save_app_settings_drops_stale_collection_id() {
        let connection = connection();

        let saved = save_app_settings(
            &connection,
            SaveAppSettingsPayload {
                last_open_collection_id: Some("missing".to_string()),
                last_view_mode: "explorer".to_string(),
            },
        )
        .unwrap();

        assert_eq!(saved.last_open_collection_id, None);
    }
}
