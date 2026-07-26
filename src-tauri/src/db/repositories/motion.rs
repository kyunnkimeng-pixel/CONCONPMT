use rusqlite::{params, Connection, OptionalExtension, Transaction};

use crate::error::{AppError, AppResult};
use crate::imaging::motion::{
    motion_recipe_json, parse_motion_recipe_json, MotionRecipe, MOTION_RECIPE_SCHEMA,
};

#[derive(Debug, Clone)]
pub struct StoredMotionRecipe {
    pub recipe: MotionRecipe,
    pub revision: i64,
}

pub fn motion_recipe_for_icon(
    connection: &Connection,
    collection_id: &str,
    icon_id: &str,
) -> AppResult<StoredMotionRecipe> {
    let stored = connection
        .query_row(
            "SELECT
               mr.recipe_schema,
               mr.revision,
               mr.motion_json
             FROM icons i
             LEFT JOIN icon_motion_recipes mr ON mr.icon_id = i.id
             WHERE i.id = ?1
               AND i.collection_id = ?2
               AND i.deleted_at IS NULL",
            params![icon_id, collection_id],
            |row| {
                Ok((
                    row.get::<_, Option<String>>("recipe_schema")?,
                    row.get::<_, Option<i64>>("revision")?,
                    row.get::<_, Option<String>>("motion_json")?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("모션을 편집할 아이콘을 찾을 수 없습니다."))?;

    decode_stored_recipe(stored.0, stored.1, stored.2)
}

pub fn upsert_motion_recipe(
    transaction: &Transaction<'_>,
    collection_id: &str,
    icon_id: &str,
    expected_revision: i64,
    recipe: &MotionRecipe,
) -> AppResult<i64> {
    if expected_revision < 0 {
        return Err(AppError::new(
            "validation",
            "모션 recipe revision이 올바르지 않습니다.",
        ));
    }

    let current_revision = transaction
        .query_row(
            "SELECT mr.revision
             FROM icons i
             LEFT JOIN icon_motion_recipes mr ON mr.icon_id = i.id
             WHERE i.id = ?1
               AND i.collection_id = ?2
               AND i.deleted_at IS NULL",
            params![icon_id, collection_id],
            |row| row.get::<_, Option<i64>>(0),
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("모션을 저장할 아이콘을 찾을 수 없습니다."))?
        .unwrap_or(0);

    if current_revision != expected_revision {
        return Err(AppError::new(
            "conflict",
            "다른 편집에서 모션이 먼저 변경되었습니다. 저장된 모션을 다시 불러와 주세요.",
        ));
    }

    let next_revision = current_revision
        .checked_add(1)
        .ok_or_else(|| AppError::new("validation", "모션 revision이 너무 큽니다."))?;
    let motion_json = motion_recipe_json(recipe)?;
    transaction.execute(
        "INSERT INTO icon_motion_recipes (
           icon_id,
           recipe_schema,
           revision,
           motion_json,
           created_at,
           updated_at
         )
         VALUES (
           ?1,
           ?2,
           ?3,
           ?4,
           strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
           strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         )
         ON CONFLICT(icon_id) DO UPDATE SET
           recipe_schema = excluded.recipe_schema,
           revision = excluded.revision,
           motion_json = excluded.motion_json,
           updated_at = excluded.updated_at",
        params![icon_id, MOTION_RECIPE_SCHEMA, next_revision, motion_json],
    )?;

    Ok(next_revision)
}

fn decode_stored_recipe(
    recipe_schema: Option<String>,
    revision: Option<i64>,
    motion_json: Option<String>,
) -> AppResult<StoredMotionRecipe> {
    let Some(recipe_schema) = recipe_schema else {
        return Ok(StoredMotionRecipe {
            recipe: MotionRecipe::default(),
            revision: 0,
        });
    };
    if recipe_schema != MOTION_RECIPE_SCHEMA {
        return Err(AppError::new(
            "validation",
            format!("지원하지 않는 모션 recipe 형식입니다: {recipe_schema}"),
        ));
    }

    let revision = revision.unwrap_or(0);
    if revision < 1 {
        return Err(AppError::new(
            "validation",
            "저장된 모션 recipe revision이 올바르지 않습니다.",
        ));
    }
    let motion_json = motion_json
        .ok_or_else(|| AppError::new("validation", "저장된 모션 recipe 내용이 비어 있습니다."))?;
    Ok(StoredMotionRecipe {
        recipe: parse_motion_recipe_json(&motion_json)?,
        revision,
    })
}

#[cfg(test)]
mod tests {
    use rusqlite::{params, Connection};

    use crate::db::migrations;
    use crate::imaging::motion::{MotionRecipe, SpatialMotion};

    use super::{motion_recipe_for_icon, upsert_motion_recipe};

    fn connection_with_icon() -> (Connection, String, String) {
        let mut connection = Connection::open_in_memory().unwrap();
        migrations::run(&mut connection).unwrap();
        let collection_id = "collection_motion".to_string();
        let source_id = "source_motion".to_string();
        let icon_id = "icon_motion".to_string();
        connection
            .execute(
                "INSERT INTO source_files (
                   id, original_filename, original_path_in_library, original_extension,
                   mime_type, width, height, byte_size, sha256, created_at
                 )
                 VALUES (?1, 'motion.png', 'C:/motion/source.png', 'png', 'image/png',
                         16, 16, 1024, 'motion-source',
                         strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
                [&source_id],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO collections (
                   id, name, created_at, updated_at
                 )
                 VALUES (?1, '모션', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                         strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
                [&collection_id],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO icons (
                   id, collection_id, source_file_id, display_name, shape, order_index,
                   created_at, updated_at
                 )
                 VALUES (?1, ?2, ?3, '모션 아이콘', 'single', 0,
                         strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                         strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
                params![icon_id, collection_id, source_id],
            )
            .unwrap();
        (connection, collection_id, icon_id)
    }

    #[test]
    fn recipe_defaults_then_saves_with_revision_guard() {
        let (mut connection, collection_id, icon_id) = connection_with_icon();
        let initial = motion_recipe_for_icon(&connection, &collection_id, &icon_id).unwrap();
        assert_eq!(initial.revision, 0);
        assert_eq!(initial.recipe, MotionRecipe::default());

        let recipe = MotionRecipe {
            spatial: Some(SpatialMotion::Breathe {
                enabled: true,
                cycles_per_loop: 2,
                scale_percent: 12,
            }),
            ..MotionRecipe::default()
        };
        let transaction = connection.transaction().unwrap();
        let revision =
            upsert_motion_recipe(&transaction, &collection_id, &icon_id, 0, &recipe).unwrap();
        transaction.commit().unwrap();
        assert_eq!(revision, 1);

        let stored = motion_recipe_for_icon(&connection, &collection_id, &icon_id).unwrap();
        assert_eq!(stored.revision, 1);
        assert_eq!(stored.recipe, recipe);

        let transaction = connection.transaction().unwrap();
        let stale =
            upsert_motion_recipe(&transaction, &collection_id, &icon_id, 0, &recipe).unwrap_err();
        assert_eq!(stale.code, "conflict");
    }

    #[test]
    fn icon_delete_cascades_motion_recipe() {
        let (mut connection, collection_id, icon_id) = connection_with_icon();
        let transaction = connection.transaction().unwrap();
        upsert_motion_recipe(
            &transaction,
            &collection_id,
            &icon_id,
            0,
            &MotionRecipe::default(),
        )
        .unwrap();
        transaction.commit().unwrap();

        connection
            .execute("DELETE FROM icons WHERE id = ?1", [&icon_id])
            .unwrap();
        let rows: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM icon_motion_recipes WHERE icon_id = ?1",
                [&icon_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(rows, 0);
    }
}
