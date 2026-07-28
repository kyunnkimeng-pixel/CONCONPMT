use rusqlite::{params, Connection, OptionalExtension};

use crate::error::{AppError, AppResult};

const FOREIGN_KEY_REBUILD_MIGRATION: &str = "018_ai_collection_grid_foundation";

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
    (
        "017_ai_web_handoff_packages",
        include_str!("../../migrations/017_ai_web_handoff_packages.sql"),
    ),
    (
        "018_ai_collection_grid_foundation",
        include_str!("../../migrations/018_ai_collection_grid_foundation.sql"),
    ),
    (
        "019_ai_grid_payload_retention",
        include_str!("../../migrations/019_ai_grid_payload_retention.sql"),
    ),
    (
        "020_ai_reference_result_normalization",
        include_str!("../../migrations/020_ai_reference_result_normalization.sql"),
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

        if *version == FOREIGN_KEY_REBUILD_MIGRATION {
            apply_foreign_key_rebuild_migration(connection, version, sql)?;
        } else {
            apply_transactional_migration(connection, version, sql)?;
        }
    }

    Ok(())
}

fn apply_transactional_migration(
    connection: &mut Connection,
    version: &str,
    sql: &str,
) -> AppResult<()> {
    let transaction = connection.transaction()?;
    transaction.execute_batch(sql)?;
    transaction.execute(
        "INSERT INTO schema_migrations (version, applied_at)
         VALUES (?1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
        params![version],
    )?;
    transaction.commit()?;
    Ok(())
}

fn apply_foreign_key_rebuild_migration(
    connection: &mut Connection,
    version: &str,
    sql: &str,
) -> AppResult<()> {
    let foreign_keys_were_enabled = foreign_keys_enabled(connection)?;
    if foreign_keys_were_enabled {
        connection.pragma_update(None, "foreign_keys", "OFF")?;
    }

    let migration_result = (|| -> AppResult<()> {
        let transaction = connection.transaction()?;
        transaction.execute_batch(sql)?;
        ensure_foreign_keys_clean(&transaction)?;
        transaction.execute(
            "INSERT INTO schema_migrations (version, applied_at)
             VALUES (?1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
            params![version],
        )?;
        transaction.commit()?;
        Ok(())
    })();

    let restore_result = if foreign_keys_were_enabled {
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .map_err(AppError::from)
            .and_then(|_| {
                if foreign_keys_enabled(connection)? {
                    Ok(())
                } else {
                    Err(AppError::new(
                        "migration_foreign_keys",
                        "AI grid migration 뒤 SQLite foreign key 검사를 다시 켤 수 없습니다.",
                    ))
                }
            })
    } else {
        Ok(())
    };

    match (migration_result, restore_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(migration_error), Ok(())) => Err(migration_error),
        (Ok(()), Err(restore_error)) => Err(restore_error),
        (Err(migration_error), Err(restore_error)) => Err(AppError::new(
            "migration_foreign_keys",
            format!("{migration_error}; SQLite foreign key 복원도 실패했습니다: {restore_error}"),
        )),
    }
}

fn foreign_keys_enabled(connection: &Connection) -> AppResult<bool> {
    Ok(connection.query_row("PRAGMA foreign_keys", [], |row| row.get::<_, i64>(0))? != 0)
}

fn ensure_foreign_keys_clean(connection: &Connection) -> AppResult<()> {
    let violation: Option<(String, Option<i64>, String, i64)> = {
        let mut statement = connection.prepare("PRAGMA foreign_key_check")?;
        let mut rows = statement.query([])?;
        match rows.next()? {
            Some(row) => Some((
                row.get::<_, String>(0)?,
                row.get::<_, Option<i64>>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
            )),
            None => None,
        }
    };
    if let Some((table, row_id, parent, foreign_key_index)) = violation {
        return Err(AppError::new(
            "migration_foreign_key_check",
            format!(
                "AI grid migration foreign key 검사에 실패했습니다: table={table}, row={row_id:?}, parent={parent}, fk={foreign_key_index}"
            ),
        ));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use rusqlite::{params, Connection};

    use super::{apply_transactional_migration, run, FOREIGN_KEY_REBUILD_MIGRATION, MIGRATIONS};

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
            "ai_web_handoff_packages",
            "ai_request_items",
            "ai_request_artifacts",
            "ai_grid_payload_retention",
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
    fn migration_adds_monotonic_ai_grid_payload_retention() {
        let mut connection = Connection::open_in_memory().unwrap();
        run(&mut connection).unwrap();

        let columns = connection
            .prepare("PRAGMA table_info(ai_grid_payload_retention)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>("name"))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(
            columns,
            [
                "request_id",
                "expires_at",
                "cleanup_requested_at",
                "payload_deleted_at",
                "created_at",
                "updated_at",
            ]
        );
        let trigger_sql: String = connection
            .query_row(
                "SELECT sql FROM sqlite_master
                 WHERE type = 'trigger'
                   AND name = 'trg_ai_grid_payload_retention_immutable'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(trigger_sql.contains("payload_deleted_at"));
        assert!(trigger_sql.contains("cleanup_requested_at"));
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
    fn run_through_ai_web_handoff(connection: &mut Connection) {
        connection
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS schema_migrations (
                   version TEXT PRIMARY KEY,
                   applied_at TEXT NOT NULL
                 );",
            )
            .unwrap();
        for (version, sql) in MIGRATIONS {
            if *version == FOREIGN_KEY_REBUILD_MIGRATION {
                break;
            }
            apply_transactional_migration(connection, version, sql).unwrap();
        }
    }

    fn insert_legacy_ai_upgrade_fixture(connection: &Connection) {
        connection
            .execute_batch(
                "INSERT INTO source_files (
                   id, original_filename, original_path_in_library,
                   original_extension, mime_type, width, height, byte_size,
                   sha256, is_animated, frame_count, has_alpha, created_at
                 ) VALUES
                   (
                     'legacy_original_source', 'original.png', 'C:/legacy/original.png',
                     'png', 'image/png', 200, 200, 4,
                     'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
                     0, NULL, 1, '2026-07-01T00:00:00Z'
                   ),
                   (
                     'legacy_candidate_source', 'candidate.png', 'C:/legacy/candidate.png',
                     'png', 'image/png', 200, 200, 4,
                     'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
                     0, NULL, 1, '2026-07-01T00:00:00Z'
                   );

                 INSERT INTO collections (
                   id, name, order_index, created_at, updated_at
                 ) VALUES (
                   'legacy_collection', '이전 모음', 0,
                   '2026-07-01T00:00:00Z', '2026-07-01T00:00:00Z'
                 );

                 INSERT INTO icons (
                   id, collection_id, source_file_id, display_name,
                   shape, order_index, created_at, updated_at
                 ) VALUES
                   (
                     'legacy_origin_icon', 'legacy_collection',
                     'legacy_original_source', '원본', 'single', 0,
                     '2026-07-01T00:00:00Z', '2026-07-01T00:00:00Z'
                   ),
                   (
                     'legacy_created_icon', 'legacy_collection',
                     'legacy_original_source', 'AI 복사', 'single', 1,
                     '2026-07-01T00:00:00Z', '2026-07-01T00:00:00Z'
                   );",
            )
            .unwrap();

        let (lineage_id, generation): (String, i64) = connection
            .query_row(
                "SELECT original_lineage_id, original_lineage_generation
                 FROM icons WHERE id = 'legacy_origin_icon'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();

        connection
            .execute(
                "INSERT INTO ai_requests (
                   id, origin_collection_id, origin_icon_id,
                   origin_collection_name_snapshot, origin_icon_name_snapshot,
                   provider_mode, service_surface, provider, adapter_id,
                   adapter_contract_version, operation, provenance_trust,
                   original_lineage_id, original_lineage_generation,
                   original_source_sha256, effective_source_sha256,
                   payload_input_signature, request_recipe_signature,
                   activation_revision, status, completed_at, created_at, updated_at
                 ) VALUES (
                   'legacy_candidate_request', 'legacy_collection', 'legacy_origin_icon',
                   '이전 모음', '원본', 'manual_web', 'other_manual',
                   'manual', 'legacy-import', '1', 'image_edit_result_import',
                   'manual_unverified', ?1, ?2,
                   'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
                   'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
                   'legacy-payload-signature', 'legacy-recipe-signature', 0,
                   'completed', '2026-07-01T00:00:01Z',
                   '2026-07-01T00:00:00Z', '2026-07-01T00:00:01Z'
                 )",
                params![lineage_id, generation],
            )
            .unwrap();
        connection
            .execute_batch(
                "INSERT INTO ai_candidates (
                   id, request_id, candidate_index, raw_source_file_id,
                   raw_source_sha256, output_format, width, height,
                   is_animated, has_alpha, created_at
                 ) VALUES (
                   'legacy_candidate', 'legacy_candidate_request', 0,
                   'legacy_candidate_source',
                   'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
                   'png', 200, 200, 0, 1, '2026-07-01T00:00:01Z'
                 );

                 INSERT INTO ai_icon_root_creations (
                   icon_id, source_icon_id, candidate_id,
                   normalization_recipe_hash, created_at
                 ) VALUES (
                   'legacy_created_icon', 'legacy_origin_icon', 'legacy_candidate',
                   'legacy-normalization-hash', '2026-07-01T00:00:02Z'
                 );",
            )
            .unwrap();

        connection
            .execute(
                "INSERT INTO ai_requests (
                   id, origin_collection_id, origin_icon_id,
                   origin_collection_name_snapshot, origin_icon_name_snapshot,
                   provider_mode, service_surface, provider, adapter_id,
                   adapter_contract_version, operation, provenance_trust,
                   input_package_sha256, original_lineage_id,
                   original_lineage_generation, original_source_sha256,
                   effective_source_sha256, payload_input_signature,
                   request_recipe_signature, activation_revision, status,
                   expires_at, created_at, updated_at
                 ) VALUES (
                   'legacy_handoff_request', 'legacy_collection', 'legacy_origin_icon',
                   '이전 모음', '원본', 'manual_web', 'other_manual',
                   'manual', 'pmtcon-web-handoff', '1',
                   'static_image_edit_web_handoff', 'manual_unverified',
                   'cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc',
                   ?1, ?2,
                   'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
                   'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
                   'legacy-handoff-payload', 'legacy-handoff-recipe', 0,
                   'awaiting_result', '2026-07-08T00:00:00Z',
                   '2026-07-01T00:00:00Z', '2026-07-01T00:00:00Z'
                 )",
                params![lineage_id, generation],
            )
            .unwrap();
        connection
            .execute_batch(
                "INSERT INTO ai_web_handoff_packages (
                   request_id, handoff_kind, layout_mode, operation,
                   service_surface, upload_file_name, upload_sha256,
                   manifest_file_name, manifest_sha256,
                   prompt_file_name, prompt_sha256,
                   expected_width, expected_height, expected_has_alpha,
                   created_at, expires_at, updated_at
                 ) VALUES (
                   'legacy_handoff_request', 'static_icon_sheet', 'single', 'edit',
                   'other_manual', 'upload.png',
                   'cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc',
                   'manifest.json',
                   'dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd',
                   'prompt.txt',
                   'eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee',
                   200, 200, 1,
                   '2026-07-01T00:00:00Z', '2026-07-08T00:00:00Z',
                   '2026-07-01T00:00:00Z'
                 );",
            )
            .unwrap();
    }

    fn foreign_key_violations(connection: &Connection) -> i64 {
        connection
            .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
                row.get(0)
            })
            .unwrap()
    }

    #[test]
    fn grid_foundation_upgrade_preserves_legacy_ai_rows_and_foreign_keys() {
        let mut connection = Connection::open_in_memory().unwrap();
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .unwrap();
        run_through_ai_web_handoff(&mut connection);
        insert_legacy_ai_upgrade_fixture(&connection);

        run(&mut connection).unwrap();

        let foreign_keys_enabled: i64 = connection
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .unwrap();
        assert_eq!(foreign_keys_enabled, 1);
        assert_eq!(foreign_key_violations(&connection), 0);

        let request_scope: String = connection
            .query_row(
                "SELECT request_scope FROM ai_requests
                 WHERE id = 'legacy_candidate_request'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(request_scope, "icon_edit");

        let request_item_id: Option<String> = connection
            .query_row(
                "SELECT request_item_id FROM ai_candidates
                 WHERE id = 'legacy_candidate'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(request_item_id, None);

        let root: (String, Option<String>, String) = connection
            .query_row(
                "SELECT creation_kind, request_item_id, normalization_recipe_hash
                 FROM ai_icon_root_creations
                 WHERE icon_id = 'legacy_created_icon'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(
            root,
            (
                "source_edit".to_string(),
                None,
                "legacy-normalization-hash".to_string()
            )
        );

        let handoff_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM ai_web_handoff_packages
                 WHERE request_id = 'legacy_handoff_request'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(handoff_count, 1);

        let immutable_scope = connection.execute(
            "UPDATE ai_requests SET request_scope = 'grid_edit'
             WHERE id = 'legacy_candidate_request'",
            [],
        );
        assert!(immutable_scope.is_err());
    }

    #[test]
    fn grid_foundation_enforces_scope_lifecycle_and_source_free_atomic_links() {
        let mut connection = Connection::open_in_memory().unwrap();
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .unwrap();
        run(&mut connection).unwrap();

        connection
            .execute_batch(
                "INSERT INTO source_files (
                   id, original_filename, original_path_in_library,
                   original_extension, mime_type, width, height, byte_size,
                   sha256, is_animated, frame_count, has_alpha, created_at
                 ) VALUES (
                   'grid_output_source', 'grid-output.png', 'C:/grid/output.png',
                   'png', 'image/png', 200, 200, 4,
                   'ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff',
                   0, NULL, 1, '2026-07-29T00:00:00Z'
                 );

                 INSERT INTO collections (
                   id, name, order_index, created_at, updated_at
                 ) VALUES (
                   'grid_collection', '그리드 모음', 0,
                   '2026-07-29T00:00:00Z', '2026-07-29T00:00:00Z'
                 );

                 INSERT INTO ai_requests (
                   id, request_scope, origin_collection_id,
                   origin_collection_name_snapshot,
                   provider_mode, service_surface, provider, adapter_id,
                   adapter_contract_version, operation, provenance_trust,
                   payload_input_signature, status, created_at, updated_at
                 ) VALUES (
                   'grid_generate_request', 'single_generate', 'grid_collection',
                   '그리드 모음', 'manual_web', 'other_manual', 'manual',
                   'pmtcon-grid-foundation', '1', 'single_image_generate',
                   'manual_unverified', 'grid-generate-payload', 'draft',
                   '2026-07-29T00:00:00Z', '2026-07-29T00:00:00Z'
                 );

                 INSERT INTO ai_request_items (
                   id, request_id, request_scope, item_index,
                   target_name_snapshot, shape, row_index, column_index,
                   input_cell_x, input_cell_y, cell_width, cell_height,
                   review_status, created_at, updated_at
                 ) VALUES (
                   'grid_generate_item', 'grid_generate_request',
                   'single_generate', 0, '새 아이콘', 'single', 0, 0,
                   0, 0, 200, 200, 'pending',
                   '2026-07-29T00:00:00Z', '2026-07-29T00:00:00Z'
                 );",
            )
            .unwrap();

        let skipped_prepare = connection.execute(
            "UPDATE ai_requests SET status = 'awaiting_result'
             WHERE id = 'grid_generate_request'",
            [],
        );
        assert!(skipped_prepare.is_err());

        connection
            .execute(
                "UPDATE ai_requests SET status = 'prepared'
                 WHERE id = 'grid_generate_request'",
                [],
            )
            .unwrap();
        connection
            .execute(
                "UPDATE ai_requests SET status = 'awaiting_result'
                 WHERE id = 'grid_generate_request'",
                [],
            )
            .unwrap();
        connection
            .execute_batch(
                "INSERT INTO ai_request_artifacts (
                   request_id, role, source_file_id, sha256,
                   manifest_json, created_at
                 ) VALUES (
                   'grid_generate_request', 'output_sheet', 'grid_output_source',
                   'ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff',
                   '{\"schema\":\"pmtcon-ai-grid-v1\",\"rows\":1,\"columns\":1}',
                   '2026-07-29T00:00:01Z'
                 );

                 UPDATE ai_requests
                 SET status = 'layout_review_pending'
                 WHERE id = 'grid_generate_request';

                 UPDATE ai_request_items
                 SET review_status = 'included'
                 WHERE id = 'grid_generate_item';

                 INSERT INTO ai_candidates (
                   id, request_id, request_item_id, candidate_index,
                   raw_source_file_id, raw_source_sha256, output_format,
                   width, height, is_animated, has_alpha, created_at
                 ) VALUES (
                   'grid_generate_candidate', 'grid_generate_request',
                   'grid_generate_item', 0, 'grid_output_source',
                   'ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff',
                   'png', 200, 200, 0, 1, '2026-07-29T00:00:02Z'
                 );

                 INSERT INTO icons (
                   id, collection_id, source_file_id, display_name,
                   icon_kind, readiness, shape, order_index,
                   created_at, updated_at
                 ) VALUES (
                   'grid_generated_icon', 'grid_collection', 'grid_output_source',
                   '새 아이콘', 'image', 'working', 'single', 0,
                   '2026-07-29T00:00:03Z', '2026-07-29T00:00:03Z'
                 );

                 INSERT INTO ai_icon_root_creations (
                   icon_id, source_icon_id, candidate_id, request_item_id,
                   creation_kind, normalization_recipe_hash, created_at
                 ) VALUES (
                   'grid_generated_icon', NULL, 'grid_generate_candidate',
                   'grid_generate_item', 'source_free', NULL,
                   '2026-07-29T00:00:03Z'
                 );

                 UPDATE ai_request_items
                 SET output_candidate_id = 'grid_generate_candidate'
                 WHERE id = 'grid_generate_item';

                 UPDATE ai_request_items
                 SET review_status = 'icon_created'
                 WHERE id = 'grid_generate_item';

                 UPDATE ai_requests
                 SET status = 'completed', completed_at = '2026-07-29T00:00:04Z'
                 WHERE id = 'grid_generate_request';",
            )
            .unwrap();

        let completed: String = connection
            .query_row(
                "SELECT status FROM ai_requests WHERE id = 'grid_generate_request'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(completed, "completed");
        assert_eq!(foreign_key_violations(&connection), 0);

        let retry_completed = connection.execute(
            "INSERT INTO ai_requests (
               id, request_scope, retry_of_request_id, origin_collection_id,
               origin_collection_name_snapshot, provider_mode, service_surface,
               provider, adapter_id, adapter_contract_version, operation,
               provenance_trust, payload_input_signature, status, created_at, updated_at
             ) VALUES (
               'grid_generate_retry_completed', 'single_generate',
               'grid_generate_request', 'grid_collection', '그리드 모음',
               'manual_web', 'other_manual', 'manual',
               'pmtcon-grid-foundation', '1', 'single_image_generate',
               'manual_unverified', 'retry-completed-payload', 'draft',
               '2026-07-29T00:00:05Z', '2026-07-29T00:00:05Z'
             )",
            [],
        );
        assert!(retry_completed.is_err());

        let immutable_item = connection.execute(
            "UPDATE ai_request_items SET cell_width = 201
             WHERE id = 'grid_generate_item'",
            [],
        );
        assert!(immutable_item.is_err());
        let immutable_artifact = connection.execute(
            "UPDATE ai_request_artifacts SET manifest_json = '{}'
             WHERE request_id = 'grid_generate_request' AND role = 'output_sheet'",
            [],
        );
        assert!(immutable_artifact.is_err());
        let terminal_restart = connection.execute(
            "UPDATE ai_requests SET status = 'awaiting_result'
             WHERE id = 'grid_generate_request'",
            [],
        );
        assert!(terminal_restart.is_err());
    }

    #[test]
    fn grid_foundation_failed_rebuild_rolls_back_and_restores_foreign_keys() {
        let mut connection = Connection::open_in_memory().unwrap();
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .unwrap();
        run_through_ai_web_handoff(&mut connection);
        connection
            .execute_batch("CREATE TABLE ai_request_items (id TEXT PRIMARY KEY);")
            .unwrap();

        let result = run(&mut connection);
        assert!(result.is_err());

        let foreign_keys_enabled: i64 = connection
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .unwrap();
        assert_eq!(foreign_keys_enabled, 1);
        assert_eq!(foreign_key_violations(&connection), 0);

        let migration_recorded: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM schema_migrations WHERE version = ?1",
                [FOREIGN_KEY_REBUILD_MIGRATION],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(migration_recorded, 0);
        let request_scope_column: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('ai_requests')
                 WHERE name = 'request_scope'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(request_scope_column, 0);
        let leaked_rebuild_table: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'table' AND name = 'ai_requests_v2'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(leaked_rebuild_table, 0);
    }

    #[test]
    fn ai_generation_reference_artifacts_support_optional_reference_sheets() {
        let mut connection = Connection::open_in_memory().unwrap();
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .unwrap();
        run(&mut connection).unwrap();

        connection
            .execute_batch(
                "INSERT INTO collections (
                   id, name, order_index, created_at, updated_at
                 ) VALUES (
                   'reference_collection', 'reference collection', 0,
                   '2026-07-29T00:00:00Z', '2026-07-29T00:00:00Z'
                 );

                 INSERT INTO source_files (
                   id, original_filename, original_path_in_library,
                   original_extension, mime_type, width, height, byte_size,
                   sha256, is_animated, frame_count, has_alpha, created_at
                 ) VALUES
                   (
                     'reference_sheet_one', 'reference-one.png',
                     'C:/reference/reference-one.png', 'png', 'image/png',
                     1024, 1024, 4,
                     '1111111111111111111111111111111111111111111111111111111111111111',
                     0, NULL, 1, '2026-07-29T00:00:00Z'
                   ),
                   (
                     'reference_sheet_two', 'reference-two.png',
                     'C:/reference/reference-two.png', 'png', 'image/png',
                     1024, 1024, 4,
                     '2222222222222222222222222222222222222222222222222222222222222222',
                     0, NULL, 1, '2026-07-29T00:00:00Z'
                   );

                 INSERT INTO ai_requests (
                   id, request_scope, origin_collection_id,
                   origin_collection_name_snapshot, provider_mode,
                   service_surface, provider, adapter_id,
                   adapter_contract_version, operation, provenance_trust,
                   input_package_sha256, reference_package_sha256,
                   payload_input_signature, status, created_at, updated_at
                 ) VALUES
                   (
                     'single_generate_reference', 'single_generate',
                     'reference_collection', 'reference collection',
                     'manual_web', 'other_manual', 'manual',
                     'pmtcon-grid-foundation', '1', 'single_image_generate',
                     'manual_unverified',
                     '1111111111111111111111111111111111111111111111111111111111111111',
                     '1111111111111111111111111111111111111111111111111111111111111111',
                     'single-reference-payload', 'draft',
                     '2026-07-29T00:00:01Z', '2026-07-29T00:00:01Z'
                   ),
                   (
                     'grid_generate_no_reference', 'grid_generate',
                     'reference_collection', 'reference collection',
                     'manual_web', 'other_manual', 'manual',
                     'pmtcon-grid-foundation', '1', 'grid_image_generate',
                     'manual_unverified', NULL, NULL,
                     'grid-no-reference-payload', 'draft',
                     '2026-07-29T00:00:02Z', '2026-07-29T00:00:02Z'
                   ),
                   (
                     'single_generate_mismatch', 'single_generate',
                     'reference_collection', 'reference collection',
                     'manual_web', 'other_manual', 'manual',
                     'pmtcon-grid-foundation', '1', 'single_image_generate',
                     'manual_unverified',
                     '1111111111111111111111111111111111111111111111111111111111111111',
                     '3333333333333333333333333333333333333333333333333333333333333333',
                     'single-mismatch-payload', 'draft',
                     '2026-07-29T00:00:03Z', '2026-07-29T00:00:03Z'
                   ),
                   (
                     'single_generate_wrong_kind', 'single_generate',
                     'reference_collection', 'reference collection',
                     'manual_web', 'other_manual', 'manual',
                     'pmtcon-grid-foundation', '1', 'single_image_generate',
                     'manual_unverified',
                     '2222222222222222222222222222222222222222222222222222222222222222',
                     '2222222222222222222222222222222222222222222222222222222222222222',
                     'single-wrong-kind-payload', 'draft',
                     '2026-07-29T00:00:04Z', '2026-07-29T00:00:04Z'
                   ),
                   (
                     'grid_edit_reference_rejected', 'grid_edit',
                     'reference_collection', 'reference collection',
                     'manual_web', 'other_manual', 'manual',
                     'pmtcon-grid-foundation', '1', 'grid_image_edit',
                     'manual_unverified',
                     '1111111111111111111111111111111111111111111111111111111111111111',
                     '1111111111111111111111111111111111111111111111111111111111111111',
                     'grid-edit-reference-payload', 'draft',
                     '2026-07-29T00:00:05Z', '2026-07-29T00:00:05Z'
                   ),
                   (
                     'grid_edit_without_reference', 'grid_edit',
                     'reference_collection', 'reference collection',
                     'manual_web', 'other_manual', 'manual',
                     'pmtcon-grid-foundation', '1', 'grid_image_edit',
                     'manual_unverified',
                     '1111111111111111111111111111111111111111111111111111111111111111',
                     NULL, 'grid-edit-no-reference-payload', 'draft',
                     '2026-07-29T00:00:06Z', '2026-07-29T00:00:06Z'
                   );

                 INSERT INTO ai_request_items (
                   id, request_id, request_scope, item_index,
                   target_name_snapshot, shape, row_index, column_index,
                   input_cell_x, input_cell_y, cell_width, cell_height,
                   review_status, created_at, updated_at
                 ) VALUES
                   (
                     'single_reference_item', 'single_generate_reference',
                     'single_generate', 0, 'generated icon', 'single', 0, 0,
                     0, 0, 200, 200, 'pending',
                     '2026-07-29T00:00:01Z', '2026-07-29T00:00:01Z'
                   ),
                   (
                     'grid_no_reference_item_0', 'grid_generate_no_reference',
                     'grid_generate', 0, 'generated icon 1', 'single', 0, 0,
                     0, 0, 200, 200, 'pending',
                     '2026-07-29T00:00:02Z', '2026-07-29T00:00:02Z'
                   ),
                   (
                     'grid_no_reference_item_1', 'grid_generate_no_reference',
                     'grid_generate', 1, 'generated icon 2', 'single', 0, 1,
                     200, 0, 200, 200, 'pending',
                     '2026-07-29T00:00:02Z', '2026-07-29T00:00:02Z'
                   ),
                   (
                     'single_mismatch_item', 'single_generate_mismatch',
                     'single_generate', 0, 'mismatched reference', 'single', 0, 0,
                     0, 0, 200, 200, 'pending',
                     '2026-07-29T00:00:03Z', '2026-07-29T00:00:03Z'
                   ),
                   (
                     'single_wrong_kind_item', 'single_generate_wrong_kind',
                     'single_generate', 0, 'wrong reference kind', 'single', 0, 0,
                     0, 0, 200, 200, 'pending',
                     '2026-07-29T00:00:04Z', '2026-07-29T00:00:04Z'
                   );

                 INSERT INTO ai_request_artifacts (
                   request_id, role, source_file_id, sha256,
                   manifest_json, created_at
                 ) VALUES (
                   'single_generate_reference', 'input_sheet',
                   'reference_sheet_one',
                   '1111111111111111111111111111111111111111111111111111111111111111',
                   json_object(
                     'schema', 'pmtcon-ai-grid-v1',
                     'kind', 'generation_reference',
                     'inputSheetSha256',
                     '1111111111111111111111111111111111111111111111111111111111111111'
                   ),
                   '2026-07-29T00:00:01Z'
                 );

                 INSERT INTO ai_request_artifacts (
                   request_id, role, source_file_id, sha256,
                   manifest_json, created_at
                 ) VALUES (
                   'grid_edit_without_reference', 'input_sheet',
                   'reference_sheet_one',
                   '1111111111111111111111111111111111111111111111111111111111111111',
                   json_object(
                     'schema', 'pmtcon-ai-grid-v1',
                     'kind', 'selected_icon_edit'
                   ),
                   '2026-07-29T00:00:06Z'
                 );

                 UPDATE ai_requests
                 SET status = 'prepared'
                 WHERE id = 'single_generate_reference';

                 UPDATE ai_requests
                 SET status = 'prepared'
                 WHERE id = 'grid_generate_no_reference';",
            )
            .unwrap();

        for request_id in ["single_generate_reference", "grid_generate_no_reference"] {
            let status: String = connection
                .query_row(
                    "SELECT status FROM ai_requests WHERE id = ?1",
                    [request_id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(status, "prepared");
        }

        let mismatched_hash = connection.execute(
            "INSERT INTO ai_request_artifacts (
               request_id, role, source_file_id, sha256,
               manifest_json, created_at
             ) VALUES (
               'single_generate_mismatch', 'input_sheet',
               'reference_sheet_one',
               '1111111111111111111111111111111111111111111111111111111111111111',
               json_object(
                 'schema', 'pmtcon-ai-grid-v1',
                 'kind', 'generation_reference',
                 'inputSheetSha256',
                 '1111111111111111111111111111111111111111111111111111111111111111'
               ),
               '2026-07-29T00:00:03Z'
             )",
            [],
        );
        assert!(mismatched_hash.is_err());

        let wrong_kind = connection.execute(
            "INSERT INTO ai_request_artifacts (
               request_id, role, source_file_id, sha256,
               manifest_json, created_at
             ) VALUES (
               'single_generate_wrong_kind', 'input_sheet',
               'reference_sheet_two',
               '2222222222222222222222222222222222222222222222222222222222222222',
               json_object(
                 'schema', 'pmtcon-ai-grid-v1',
                 'kind', 'selected_icon_edit',
                 'inputSheetSha256',
                 '2222222222222222222222222222222222222222222222222222222222222222'
               ),
               '2026-07-29T00:00:04Z'
             )",
            [],
        );
        assert!(wrong_kind.is_err());

        let grid_edit_reference = connection.execute(
            "INSERT INTO ai_request_artifacts (
               request_id, role, source_file_id, sha256,
               manifest_json, created_at
             ) VALUES (
               'grid_edit_reference_rejected', 'input_sheet',
               'reference_sheet_one',
               '1111111111111111111111111111111111111111111111111111111111111111',
               json_object(
                 'schema', 'pmtcon-ai-grid-v1',
                 'kind', 'selected_icon_edit'
               ),
               '2026-07-29T00:00:05Z'
             )",
            [],
        );
        assert!(grid_edit_reference.is_err());

        let missing_reference_artifact = connection.execute(
            "UPDATE ai_requests SET status = 'prepared'
             WHERE id = 'single_generate_mismatch'",
            [],
        );
        assert!(missing_reference_artifact.is_err());

        let prepared_guard_sql: String = connection
            .query_row(
                "SELECT sql FROM sqlite_master
                 WHERE type = 'trigger'
                   AND name = 'trg_ai_grid_request_prepared_guard_before_update'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(prepared_guard_sql.contains("OLD.reference_package_sha256 IS NOT NULL"));
        assert!(prepared_guard_sql.contains("generation_reference"));
        assert_eq!(foreign_key_violations(&connection), 0);
    }

    #[test]
    fn ai_web_handoff_result_accepts_same_aspect_ratio_and_preserves_safety_checks() {
        let mut connection = Connection::open_in_memory().unwrap();
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .unwrap();
        run(&mut connection).unwrap();

        connection
            .execute_batch(
                "INSERT INTO source_files (
                   id, original_filename, original_path_in_library,
                   original_extension, mime_type, width, height, byte_size,
                   sha256, is_animated, frame_count, has_alpha, created_at
                 ) VALUES
                   (
                     'ratio_pass_source', 'ratio-pass.png', 'C:/results/ratio-pass.png',
                     'png', 'image/png', 400, 400, 4,
                     'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
                     0, NULL, 1, '2026-07-29T00:00:00Z'
                   ),
                   (
                     'ratio_fail_source', 'ratio-fail.png', 'C:/results/ratio-fail.png',
                     'png', 'image/png', 400, 300, 4,
                     'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
                     0, NULL, 1, '2026-07-29T00:00:00Z'
                   ),
                   (
                     'alpha_fail_source', 'alpha-fail.png', 'C:/results/alpha-fail.png',
                     'png', 'image/png', 400, 400, 4,
                     'cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc',
                     0, NULL, 0, '2026-07-29T00:00:00Z'
                   ),
                   (
                     'animated_fail_source', 'animated-fail.gif',
                     'C:/results/animated-fail.gif', 'gif', 'image/gif', 400, 400, 4,
                     'dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd',
                     1, 2, 1, '2026-07-29T00:00:00Z'
                   );

                 INSERT INTO ai_requests (
                   id, request_scope, provider_mode, service_surface,
                   provider, adapter_id, adapter_contract_version, operation,
                   provenance_trust, input_package_sha256,
                   original_lineage_id, original_lineage_generation,
                   original_source_sha256, effective_source_sha256,
                   payload_input_signature, request_recipe_signature,
                   activation_revision, status, expires_at, created_at, updated_at
                 ) VALUES
                   (
                     'ratio_pass_request', 'icon_edit', 'manual_web', 'other_manual',
                     'manual', 'pmtcon-web-handoff', '1',
                     'static_image_edit_web_handoff', 'manual_unverified',
                     'eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee',
                     'ratio-pass-lineage', 0,
                     '0000000000000000000000000000000000000000000000000000000000000000',
                     '0000000000000000000000000000000000000000000000000000000000000000',
                     'ratio-pass-payload', 'ratio-pass-recipe', 0,
                     'awaiting_result', '2026-08-05T00:00:00Z',
                     '2026-07-29T00:00:00Z', '2026-07-29T00:00:00Z'
                   ),
                   (
                     'ratio_fail_request', 'icon_edit', 'manual_web', 'other_manual',
                     'manual', 'pmtcon-web-handoff', '1',
                     'static_image_edit_web_handoff', 'manual_unverified',
                     'eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee',
                     'ratio-fail-lineage', 0,
                     '0000000000000000000000000000000000000000000000000000000000000000',
                     '0000000000000000000000000000000000000000000000000000000000000000',
                     'ratio-fail-payload', 'ratio-fail-recipe', 0,
                     'awaiting_result', '2026-08-05T00:00:00Z',
                     '2026-07-29T00:00:00Z', '2026-07-29T00:00:00Z'
                   ),
                   (
                     'alpha_fail_request', 'icon_edit', 'manual_web', 'other_manual',
                     'manual', 'pmtcon-web-handoff', '1',
                     'static_image_edit_web_handoff', 'manual_unverified',
                     'eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee',
                     'alpha-fail-lineage', 0,
                     '0000000000000000000000000000000000000000000000000000000000000000',
                     '0000000000000000000000000000000000000000000000000000000000000000',
                     'alpha-fail-payload', 'alpha-fail-recipe', 0,
                     'awaiting_result', '2026-08-05T00:00:00Z',
                     '2026-07-29T00:00:00Z', '2026-07-29T00:00:00Z'
                   ),
                   (
                     'animated_fail_request', 'icon_edit', 'manual_web', 'other_manual',
                     'manual', 'pmtcon-web-handoff', '1',
                     'static_image_edit_web_handoff', 'manual_unverified',
                     'eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee',
                     'animated-fail-lineage', 0,
                     '0000000000000000000000000000000000000000000000000000000000000000',
                     '0000000000000000000000000000000000000000000000000000000000000000',
                     'animated-fail-payload', 'animated-fail-recipe', 0,
                     'awaiting_result', '2026-08-05T00:00:00Z',
                     '2026-07-29T00:00:00Z', '2026-07-29T00:00:00Z'
                   );

                 INSERT INTO ai_web_handoff_packages (
                   request_id, handoff_kind, layout_mode, operation,
                   service_surface, upload_file_name, upload_sha256,
                   manifest_file_name, manifest_sha256,
                   prompt_file_name, prompt_sha256,
                   expected_width, expected_height, expected_has_alpha,
                   created_at, expires_at, updated_at
                 ) VALUES
                   (
                     'ratio_pass_request', 'static_icon_sheet', 'single', 'edit',
                     'other_manual', 'upload.png',
                     'eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee',
                     'manifest.json',
                     'ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff',
                     'prompt.txt',
                     '1111111111111111111111111111111111111111111111111111111111111111',
                     200, 200, 1, '2026-07-29T00:00:00Z',
                     '2026-08-05T00:00:00Z', '2026-07-29T00:00:00Z'
                   ),
                   (
                     'ratio_fail_request', 'static_icon_sheet', 'single', 'edit',
                     'other_manual', 'upload.png',
                     'eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee',
                     'manifest.json',
                     'ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff',
                     'prompt.txt',
                     '1111111111111111111111111111111111111111111111111111111111111111',
                     200, 200, 1, '2026-07-29T00:00:00Z',
                     '2026-08-05T00:00:00Z', '2026-07-29T00:00:00Z'
                   ),
                   (
                     'alpha_fail_request', 'static_icon_sheet', 'single', 'edit',
                     'other_manual', 'upload.png',
                     'eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee',
                     'manifest.json',
                     'ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff',
                     'prompt.txt',
                     '1111111111111111111111111111111111111111111111111111111111111111',
                     200, 200, 1, '2026-07-29T00:00:00Z',
                     '2026-08-05T00:00:00Z', '2026-07-29T00:00:00Z'
                   ),
                   (
                     'animated_fail_request', 'static_icon_sheet', 'single', 'edit',
                     'other_manual', 'upload.png',
                     'eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee',
                     'manifest.json',
                     'ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff',
                     'prompt.txt',
                     '1111111111111111111111111111111111111111111111111111111111111111',
                     200, 200, 1, '2026-07-29T00:00:00Z',
                     '2026-08-05T00:00:00Z', '2026-07-29T00:00:00Z'
                   );

                 INSERT INTO ai_candidates (
                   id, request_id, candidate_index, raw_source_file_id,
                   raw_source_sha256, output_format, width, height,
                   is_animated, has_alpha, created_at
                 ) VALUES
                   (
                     'ratio_pass_candidate', 'ratio_pass_request', 0,
                     'ratio_pass_source',
                     'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
                     'png', 400, 400, 0, 1, '2026-07-29T00:00:01Z'
                   ),
                   (
                     'ratio_fail_candidate', 'ratio_fail_request', 0,
                     'ratio_fail_source',
                     'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
                     'png', 400, 300, 0, 1, '2026-07-29T00:00:01Z'
                   ),
                   (
                     'alpha_fail_candidate', 'alpha_fail_request', 0,
                     'alpha_fail_source',
                     'cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc',
                     'png', 400, 400, 0, 0, '2026-07-29T00:00:01Z'
                   ),
                   (
                     'animated_fail_candidate', 'animated_fail_request', 0,
                     'animated_fail_source',
                     'dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd',
                     'gif', 400, 400, 1, 1, '2026-07-29T00:00:01Z'
                   );

                 UPDATE ai_requests
                 SET status = 'completed', completed_at = '2026-07-29T00:00:02Z',
                     updated_at = '2026-07-29T00:00:02Z'
                 WHERE id IN (
                   'ratio_pass_request', 'ratio_fail_request',
                   'alpha_fail_request', 'animated_fail_request'
                 );

                 UPDATE ai_web_handoff_packages
                 SET cleanup_requested_at = '2026-07-29T00:00:03Z',
                     updated_at = '2026-07-29T00:00:03Z';

                 UPDATE ai_web_handoff_packages
                 SET candidate_id = 'ratio_pass_candidate',
                     result_sha256 =
                       'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
                     result_received_at = '2026-07-29T00:00:04Z',
                     updated_at = '2026-07-29T00:00:04Z'
                 WHERE request_id = 'ratio_pass_request';",
            )
            .unwrap();

        let accepted_candidate: String = connection
            .query_row(
                "SELECT candidate_id FROM ai_web_handoff_packages
                 WHERE request_id = 'ratio_pass_request'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(accepted_candidate, "ratio_pass_candidate");

        for (request_id, candidate_id, result_sha256) in [
            (
                "ratio_fail_request",
                "ratio_fail_candidate",
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            ),
            (
                "alpha_fail_request",
                "alpha_fail_candidate",
                "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
            ),
            (
                "animated_fail_request",
                "animated_fail_candidate",
                "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
            ),
        ] {
            let rejected = connection.execute(
                "UPDATE ai_web_handoff_packages
                 SET candidate_id = ?2, result_sha256 = ?3,
                     result_received_at = '2026-07-29T00:00:04Z',
                     updated_at = '2026-07-29T00:00:04Z'
                 WHERE request_id = ?1",
                params![request_id, candidate_id, result_sha256],
            );
            assert!(
                rejected.is_err(),
                "unsafe result must be rejected: {request_id}"
            );
        }

        assert_eq!(foreign_key_violations(&connection), 0);
    }
}
