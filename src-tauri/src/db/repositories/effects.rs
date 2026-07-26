use rusqlite::{params, Connection, OptionalExtension, Transaction};

use crate::error::{AppError, AppResult};
use crate::imaging::effects::{
    effect_recipe_json, parse_effect_recipe_json, EffectRecipe, EFFECT_RECIPE_SCHEMA,
};

#[derive(Debug, Clone)]
pub struct StoredEffectRecipe {
    pub recipe: EffectRecipe,
    pub revision: i64,
}

pub fn effect_recipe_for_icon(
    connection: &Connection,
    collection_id: &str,
    icon_id: &str,
) -> AppResult<StoredEffectRecipe> {
    let stored = connection
        .query_row(
            "SELECT
               er.recipe_schema,
               er.revision,
               er.effects_json
             FROM icons i
             LEFT JOIN icon_effect_recipes er ON er.icon_id = i.id
             WHERE i.id = ?1
               AND i.collection_id = ?2
               AND i.deleted_at IS NULL",
            params![icon_id, collection_id],
            |row| {
                Ok((
                    row.get::<_, Option<String>>("recipe_schema")?,
                    row.get::<_, Option<i64>>("revision")?,
                    row.get::<_, Option<String>>("effects_json")?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("효과를 편집할 아이콘을 찾을 수 없습니다."))?;

    decode_stored_recipe(stored.0, stored.1, stored.2)
}

pub fn upsert_effect_recipe(
    transaction: &Transaction<'_>,
    collection_id: &str,
    icon_id: &str,
    expected_revision: i64,
    recipe: &EffectRecipe,
) -> AppResult<i64> {
    if expected_revision < 0 {
        return Err(AppError::new(
            "validation",
            "효과 recipe revision이 올바르지 않습니다.",
        ));
    }

    let current_revision = transaction
        .query_row(
            "SELECT er.revision
             FROM icons i
             LEFT JOIN icon_effect_recipes er ON er.icon_id = i.id
             WHERE i.id = ?1
               AND i.collection_id = ?2
               AND i.deleted_at IS NULL",
            params![icon_id, collection_id],
            |row| row.get::<_, Option<i64>>(0),
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("효과를 저장할 아이콘을 찾을 수 없습니다."))?
        .unwrap_or(0);

    if current_revision != expected_revision {
        return Err(AppError::new(
            "conflict",
            "다른 편집에서 효과가 먼저 변경되었습니다. 저장된 효과를 다시 불러와 주세요.",
        ));
    }

    let next_revision = current_revision
        .checked_add(1)
        .ok_or_else(|| AppError::new("validation", "효과 revision이 너무 큽니다."))?;
    let effects_json = effect_recipe_json(recipe)?;
    transaction.execute(
        "INSERT INTO icon_effect_recipes (
           icon_id,
           recipe_schema,
           revision,
           effects_json,
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
           effects_json = excluded.effects_json,
           updated_at = excluded.updated_at",
        params![icon_id, EFFECT_RECIPE_SCHEMA, next_revision, effects_json],
    )?;

    Ok(next_revision)
}

fn decode_stored_recipe(
    recipe_schema: Option<String>,
    revision: Option<i64>,
    effects_json: Option<String>,
) -> AppResult<StoredEffectRecipe> {
    let Some(recipe_schema) = recipe_schema else {
        return Ok(StoredEffectRecipe {
            recipe: EffectRecipe::default(),
            revision: 0,
        });
    };
    if recipe_schema != EFFECT_RECIPE_SCHEMA {
        return Err(AppError::new(
            "validation",
            format!("지원하지 않는 효과 recipe 형식입니다: {recipe_schema}"),
        ));
    }

    let revision = revision.unwrap_or(0);
    if revision < 1 {
        return Err(AppError::new(
            "validation",
            "저장된 효과 recipe revision이 올바르지 않습니다.",
        ));
    }
    let effects_json = effects_json
        .ok_or_else(|| AppError::new("validation", "저장된 효과 recipe 내용이 비어 있습니다."))?;
    Ok(StoredEffectRecipe {
        recipe: parse_effect_recipe_json(&effects_json)?,
        revision,
    })
}

#[cfg(test)]
mod tests {
    use rusqlite::{params, Connection};

    use crate::db::migrations;
    use crate::imaging::effects::{EffectRecipe, EffectStep, EFFECT_RECIPE_VERSION};

    use super::{effect_recipe_for_icon, upsert_effect_recipe};

    fn connection_with_icon() -> (Connection, String, String) {
        let mut connection = Connection::open_in_memory().unwrap();
        migrations::run(&mut connection).unwrap();
        let collection_id = "collection_effect".to_string();
        let source_id = "source_effect".to_string();
        let icon_id = "icon_effect".to_string();
        connection
            .execute(
                "INSERT INTO source_files (
                   id, original_filename, original_path_in_library, original_extension,
                   mime_type, width, height, byte_size, sha256, created_at
                 )
                 VALUES (?1, 'a.png', 'C:/effect/a.png', 'png', 'image/png',
                         1, 1, 4, 'effect-source',
                         strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
                [&source_id],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO collections (
                   id, name, created_at, updated_at
                 )
                 VALUES (?1, '효과', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
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
                 VALUES (?1, ?2, ?3, '아이콘', 'single', 0,
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
        let initial = effect_recipe_for_icon(&connection, &collection_id, &icon_id).unwrap();
        assert_eq!(initial.revision, 0);
        assert!(initial.recipe.effects.is_empty());

        let recipe = EffectRecipe {
            version: EFFECT_RECIPE_VERSION,
            effects: vec![EffectStep::Pixelate {
                id: "pixel".to_string(),
                enabled: true,
                block_size: 4,
            }],
        };
        let transaction = connection.transaction().unwrap();
        let revision =
            upsert_effect_recipe(&transaction, &collection_id, &icon_id, 0, &recipe).unwrap();
        transaction.commit().unwrap();
        assert_eq!(revision, 1);

        let stored = effect_recipe_for_icon(&connection, &collection_id, &icon_id).unwrap();
        assert_eq!(stored.revision, 1);
        assert_eq!(stored.recipe, recipe);

        let transaction = connection.transaction().unwrap();
        let stale =
            upsert_effect_recipe(&transaction, &collection_id, &icon_id, 0, &recipe).unwrap_err();
        assert_eq!(stale.code, "conflict");
    }
}
