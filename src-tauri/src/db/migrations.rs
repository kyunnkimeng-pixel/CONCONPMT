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
    (
        "012_ai_nondestructive_foundation",
        include_str!("../../migrations/012_ai_nondestructive_foundation.sql"),
    ),
    (
        "013_ai_effective_variants",
        include_str!("../../migrations/013_ai_effective_variants.sql"),
    ),
    (
        "014_ai_lineage_registry",
        include_str!("../../migrations/014_ai_lineage_registry.sql"),
    ),
    (
        "015_ai_request_snapshot_immutability",
        include_str!("../../migrations/015_ai_request_snapshot_immutability.sql"),
    ),
    (
        "016_ai_icon_root_creations",
        include_str!("../../migrations/016_ai_icon_root_creations.sql"),
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
            "icon_ai_lineages",
            "ai_requests",
            "ai_candidates",
            "icon_ai_versions",
            "icon_ai_state",
            "ai_icon_root_creations",
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

    #[test]
    fn ai_variant_migration_keeps_foreign_keys_clean_and_adds_digest_provenance() {
        let mut connection = Connection::open_in_memory().unwrap();
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .unwrap();

        run(&mut connection).unwrap();

        let violations = connection
            .prepare("PRAGMA foreign_key_check")
            .unwrap()
            .query_map([], |_| Ok(()))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert!(violations.is_empty());

        let columns = connection
            .prepare("PRAGMA table_info(processed_asset_variants)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>("name"))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert!(columns.contains(&"output_sha256".to_string()));

        let source_fk = connection
            .prepare("PRAGMA foreign_key_list(processed_asset_variants)")
            .unwrap()
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>("table")?,
                    row.get::<_, String>("from")?,
                    row.get::<_, String>("on_delete")?,
                ))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
            .into_iter()
            .find(|(_, from, _)| from == "source_file_id");
        assert_eq!(
            source_fk,
            Some((
                "source_files".to_string(),
                "source_file_id".to_string(),
                "RESTRICT".to_string(),
            ))
        );
    }

    #[test]
    fn ai_lineage_registry_has_required_foreign_keys_and_version_guard() {
        let mut connection = Connection::open_in_memory().unwrap();
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .unwrap();
        run(&mut connection).unwrap();

        let foreign_keys = connection
            .prepare("PRAGMA foreign_key_list(icon_ai_lineages)")
            .unwrap()
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>("table")?,
                    row.get::<_, String>("from")?,
                    row.get::<_, String>("on_delete")?,
                ))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert!(foreign_keys.contains(&(
            "icons".to_string(),
            "icon_id".to_string(),
            "CASCADE".to_string(),
        )));
        assert!(foreign_keys.contains(&(
            "source_files".to_string(),
            "original_source_file_id".to_string(),
            "RESTRICT".to_string(),
        )));

        let guard_sql: String = connection
            .query_row(
                "SELECT sql FROM sqlite_master
                 WHERE type = 'trigger'
                   AND name = 'trg_icon_ai_version_lineage_guard_before_insert'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(guard_sql.contains("icon_ai_lineages"));
        assert!(guard_sql.contains("original_source_file_id"));
    }

    #[test]
    fn ai_request_provenance_snapshots_are_guarded_by_database_trigger() {
        let mut connection = Connection::open_in_memory().unwrap();
        run(&mut connection).unwrap();

        let trigger_sql: String = connection
            .query_row(
                "SELECT sql FROM sqlite_master
                 WHERE type = 'trigger'
                   AND name = 'trg_ai_request_snapshots_immutable_before_update'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        for snapshot_column in [
            "adapter_contract_version",
            "capability_snapshot_json",
            "data_tier_snapshot_json",
            "consent_snapshot_json",
            "payload_input_signature",
            "request_recipe_signature",
            "activation_revision",
        ] {
            assert!(
                trigger_sql.contains(snapshot_column),
                "snapshot trigger must guard {snapshot_column}"
            );
        }
        assert!(trigger_sql.contains("RAISE(ABORT"));

        connection
            .execute_batch(
                "INSERT INTO ai_requests (
                   id, provider_mode, service_surface, provider, adapter_id,
                   adapter_contract_version, operation, provenance_trust,
                   original_lineage_id, original_lineage_generation,
                   original_source_sha256, effective_source_sha256,
                   payload_input_signature, request_recipe_signature,
                   activation_revision, status, created_at, updated_at
                 ) VALUES (
                   'request_guard_test', 'manual_web', 'other_manual',
                   'manual', 'manual-import', '1', 'image_edit',
                   'manual_unverified', 'lineage_guard_test', 0,
                   'original-hash', 'effective-hash', 'payload-signature',
                   'recipe-signature', 0, 'prepared',
                   '2026-07-27T00:00:00Z', '2026-07-27T00:00:00Z'
                 );",
            )
            .unwrap();
        connection
            .execute(
                "UPDATE ai_requests
                 SET status = 'completed',
                     updated_at = '2026-07-27T00:00:01Z'
                 WHERE id = 'request_guard_test'",
                [],
            )
            .unwrap();
        let immutable_update = connection.execute(
            "UPDATE ai_requests
             SET provider = 'rewritten'
             WHERE id = 'request_guard_test'",
            [],
        );
        assert!(immutable_update.is_err());
    }
    #[test]
    fn ai_icon_root_creation_provenance_has_expected_foreign_keys() {
        let mut connection = Connection::open_in_memory().unwrap();
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .unwrap();

        run(&mut connection).unwrap();
        run(&mut connection).unwrap();

        let foreign_keys = connection
            .prepare("PRAGMA foreign_key_list(ai_icon_root_creations)")
            .unwrap()
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>("table")?,
                    row.get::<_, String>("from")?,
                    row.get::<_, String>("to")?,
                    row.get::<_, String>("on_delete")?,
                ))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert!(foreign_keys.contains(&(
            "icons".to_string(),
            "icon_id".to_string(),
            "id".to_string(),
            "CASCADE".to_string(),
        )));
        assert!(foreign_keys.contains(&(
            "icons".to_string(),
            "source_icon_id".to_string(),
            "id".to_string(),
            "SET NULL".to_string(),
        )));
        assert!(foreign_keys.contains(&(
            "ai_candidates".to_string(),
            "candidate_id".to_string(),
            "id".to_string(),
            "RESTRICT".to_string(),
        )));

        let violations = connection
            .prepare("PRAGMA foreign_key_check")
            .unwrap()
            .query_map([], |_| Ok(()))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert!(violations.is_empty());
    }
}
