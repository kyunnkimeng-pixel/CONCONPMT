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
}
