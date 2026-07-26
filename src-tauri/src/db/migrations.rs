use rusqlite::{params, Connection, OptionalExtension};

use crate::error::AppResult;

const MIGRATIONS: &[(&str, &str)] = &[
    (
        "001_app_data",
        include_str!("../../migrations/001_app_data.sql"),
    ),
    (
        "002_consolidated_missing_features",
        include_str!("../../migrations/002_consolidated_missing_features.sql"),
    ),
    (
        "003_icon_readiness_placeholders",
        include_str!("../../migrations/003_icon_readiness_placeholders.sql"),
    ),
    (
        "004_processed_asset_variants",
        include_str!("../../migrations/004_processed_asset_variants.sql"),
    ),
    (
        "005_gif_pingpong",
        include_str!("../../migrations/005_gif_pingpong.sql"),
    ),
    (
        "006_icon_text_overlay",
        include_str!("../../migrations/006_icon_text_overlay.sql"),
    ),
    (
        "007_context_menu_sheet_workflows",
        include_str!("../../migrations/007_context_menu_sheet_workflows.sql"),
    ),
    (
        "008_icon_transforms",
        include_str!("../../migrations/008_icon_transforms.sql"),
    ),
    (
        "009_frame_sheet_gif_recipes",
        include_str!("../../migrations/009_frame_sheet_gif_recipes.sql"),
    ),
    (
        "010_icon_effect_recipes",
        include_str!("../../migrations/010_icon_effect_recipes.sql"),
    ),
    (
        "011_icon_motion_recipes",
        include_str!("../../migrations/011_icon_motion_recipes.sql"),
    ),
];

pub fn run(connection: &mut Connection) -> AppResult<()> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
          version TEXT PRIMARY KEY,
          applied_at TEXT NOT NULL
        );",
    )?;

    for (version, sql) in MIGRATIONS {
        let is_applied = connection
            .query_row(
                "SELECT version FROM schema_migrations WHERE version = ?1",
                params![version],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .is_some();

        if is_applied {
            continue;
        }

        let transaction = connection.transaction()?;
        transaction.execute_batch(sql)?;
        transaction.execute(
            "INSERT INTO schema_migrations (version, applied_at)
             VALUES (?1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
            params![version],
        )?;
        transaction.commit()?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::run;

    #[test]
    fn migration_creates_required_tables() {
        let mut connection = Connection::open_in_memory().unwrap();

        run(&mut connection).unwrap();

        for table_name in [
            "source_files",
            "collections",
            "icons",
            "crop_settings",
            "icon_pieces",
            "export_profiles",
            "processed_asset_variants",
            "optimization_jobs",
            "icon_notes",
            "sheet_grid_presets",
            "frame_sheet_gif_recipes",
            "icon_effect_recipes",
            "icon_motion_recipes",
            "app_settings",
        ] {
            let exists: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                    [table_name],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(exists, 1, "missing table {table_name}");
        }
    }

    #[test]
    fn migration_is_idempotent() {
        let mut connection = Connection::open_in_memory().unwrap();

        run(&mut connection).unwrap();
        run(&mut connection).unwrap();

        let app_settings_rows: i64 = connection
            .query_row("SELECT COUNT(*) FROM app_settings", [], |row| row.get(0))
            .unwrap();
        assert_eq!(app_settings_rows, 1);
    }

    #[test]
    fn migration_adds_persistent_icon_transform_columns() {
        let mut connection = Connection::open_in_memory().unwrap();

        run(&mut connection).unwrap();

        let mut statement = connection.prepare("PRAGMA table_info(icons)").unwrap();
        let columns = statement
            .query_map([], |row| row.get::<_, String>("name"))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert!(columns.contains(&"transform_quarter_turns".to_string()));
        assert!(columns.contains(&"transform_flip_horizontal".to_string()));
        assert!(columns.contains(&"transform_flip_vertical".to_string()));
    }

    #[test]
    fn migration_adds_frame_sheet_gif_recipe_provenance_schema() {
        let mut connection = Connection::open_in_memory().unwrap();

        run(&mut connection).unwrap();

        let mut statement = connection
            .prepare("PRAGMA table_info(frame_sheet_gif_recipes)")
            .unwrap();
        let columns = statement
            .query_map([], |row| row.get::<_, String>("name"))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(
            columns,
            [
                "id",
                "generated_icon_id",
                "original_sheet_filename",
                "original_sheet_path",
                "original_sheet_sha256",
                "recipe_schema",
                "grid_settings_json",
                "frames_json",
                "direction",
                "loop_mode",
                "loop_count",
                "measured_byte_size",
                "render_hash",
                "created_at",
                "updated_at",
            ]
        );

        let foreign_key = connection
            .query_row(
                "PRAGMA foreign_key_list(frame_sheet_gif_recipes)",
                [],
                |row| {
                    Ok((
                        row.get::<_, String>("table")?,
                        row.get::<_, String>("from")?,
                        row.get::<_, String>("to")?,
                        row.get::<_, String>("on_delete")?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(
            foreign_key,
            (
                "icons".to_string(),
                "generated_icon_id".to_string(),
                "id".to_string(),
                "CASCADE".to_string(),
            )
        );
    }

    #[test]
    fn migration_adds_revisioned_icon_effect_recipe_schema() {
        let mut connection = Connection::open_in_memory().unwrap();

        run(&mut connection).unwrap();

        let mut statement = connection
            .prepare("PRAGMA table_info(icon_effect_recipes)")
            .unwrap();
        let columns = statement
            .query_map([], |row| row.get::<_, String>("name"))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(
            columns,
            [
                "icon_id",
                "recipe_schema",
                "revision",
                "effects_json",
                "created_at",
                "updated_at",
            ]
        );

        let foreign_key = connection
            .query_row("PRAGMA foreign_key_list(icon_effect_recipes)", [], |row| {
                Ok((
                    row.get::<_, String>("table")?,
                    row.get::<_, String>("from")?,
                    row.get::<_, String>("to")?,
                    row.get::<_, String>("on_delete")?,
                ))
            })
            .unwrap();
        assert_eq!(
            foreign_key,
            (
                "icons".to_string(),
                "icon_id".to_string(),
                "id".to_string(),
                "CASCADE".to_string(),
            )
        );
    }
    #[test]
    fn migration_adds_revisioned_icon_motion_recipe_schema() {
        let mut connection = Connection::open_in_memory().unwrap();

        run(&mut connection).unwrap();

        let mut statement = connection
            .prepare("PRAGMA table_info(icon_motion_recipes)")
            .unwrap();
        let columns = statement
            .query_map([], |row| row.get::<_, String>("name"))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(
            columns,
            [
                "icon_id",
                "recipe_schema",
                "revision",
                "motion_json",
                "created_at",
                "updated_at",
            ]
        );

        let foreign_key = connection
            .query_row("PRAGMA foreign_key_list(icon_motion_recipes)", [], |row| {
                Ok((
                    row.get::<_, String>("table")?,
                    row.get::<_, String>("from")?,
                    row.get::<_, String>("to")?,
                    row.get::<_, String>("on_delete")?,
                ))
            })
            .unwrap();
        assert_eq!(
            foreign_key,
            (
                "icons".to_string(),
                "icon_id".to_string(),
                "id".to_string(),
                "CASCADE".to_string(),
            )
        );
    }
}
