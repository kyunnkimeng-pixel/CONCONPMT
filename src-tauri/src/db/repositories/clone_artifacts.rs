use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use rusqlite::{params, Connection, OptionalExtension, Transaction};
use sha2::{Digest, Sha256};

use crate::db::repositories::ai as ai_repository;
use crate::db::repositories::ai_activation;
use crate::db::repositories::optimization as optimization_repository;
use crate::error::{AppError, AppResult};
use crate::ids::create_id;
use crate::imaging::import_limits::MAX_IMPORT_FILE_BYTES;
use crate::optimization::analyzer;
use crate::paths::AppPaths;

const CLONED_PREVIEW_DIRECTORY: &str = "cloned";
const CLONED_VARIANT_DIRECTORY: &str = "cloned";
pub(crate) fn validate_icon_clone_source(
    connection: &Connection,
    collection_id: &str,
    icon_id: &str,
) -> AppResult<()> {
    ai_repository::resolve_effective_visual_source(connection, collection_id, icon_id)?;
    Ok(())
}

pub(crate) fn validate_collection_clone_sources(
    connection: &Connection,
    collection_id: &str,
) -> AppResult<()> {
    let icon_ids = {
        let mut statement = connection.prepare(
            "SELECT id FROM icons
             WHERE collection_id = ?1 AND deleted_at IS NULL
             ORDER BY order_index ASC, created_at ASC, id ASC",
        )?;
        let rows = statement
            .query_map([collection_id], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        rows
    };
    for icon_id in icon_ids {
        validate_icon_clone_source(connection, collection_id, &icon_id)?;
    }
    Ok(())
}

pub(crate) fn validate_icon_clone_target(
    transaction: &Transaction<'_>,
    collection_id: &str,
    icon_id: &str,
) -> AppResult<()> {
    ai_repository::resolve_effective_visual_source(transaction, collection_id, icon_id)?;
    Ok(())
}

pub(crate) fn materialize_clone_native_preview(
    transaction: &Transaction<'_>,
    paths: &AppPaths,
    collection_id: &str,
    icon_id: &str,
) -> AppResult<()> {
    let operation_id = create_id("clone-preview");
    let operation_component = safe_component(&operation_id, "clone preview operation ID")?;
    let collection_component = safe_component(collection_id, "collection ID")?;
    let icon_component = safe_component(icon_id, "icon ID")?;
    let staging_dir = paths.ai_activation_staging_dir.join(operation_component);
    let final_icon_root = paths
        .ai_activation_previews_dir
        .join(collection_component)
        .join(icon_component);
    let final_dir = final_icon_root.join("native-clone");

    let result = (|| -> AppResult<()> {
        if final_dir.exists() {
            return Err(AppError::new(
                "clone_preview_conflict",
                "The target clone preview directory already exists.",
            ));
        }
        prepare_target_directory(&paths.ai_activation_staging_dir, &staging_dir)?;
        let generated = ai_activation::render_effective_preview_to_directory(
            transaction,
            &staging_dir,
            collection_id,
            icon_id,
        )?;

        let piece_ids = {
            let mut statement = transaction.prepare(
                "SELECT id FROM icon_pieces
                 WHERE icon_id = ?1
                 ORDER BY piece_index ASC",
            )?;
            let rows = statement
                .query_map([icon_id], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            rows
        };
        if generated.piece_paths.len() != piece_ids.len() {
            return Err(AppError::new(
                "clone_preview_structure",
                "The rendered clone preview piece count does not match the target icon.",
            ));
        }

        let final_parent = final_dir.parent().ok_or_else(|| {
            AppError::new(
                "clone_preview_path",
                "The clone preview parent path is invalid.",
            )
        })?;
        prepare_target_directory(&paths.ai_activation_previews_dir, final_parent)?;
        fs::rename(&staging_dir, &final_dir).map_err(|error| {
            AppError::new(
                "clone_preview_commit",
                format!("Failed to commit the clone preview directory: {error}"),
            )
        })?;

        let final_current =
            rebase_clone_preview_path(&generated.current_preview_path, &staging_dir, &final_dir)?;
        let final_pieces = generated
            .piece_paths
            .iter()
            .map(|path| rebase_clone_preview_path(path, &staging_dir, &final_dir))
            .collect::<AppResult<Vec<_>>>()?;
        let icon_rows = transaction.execute(
            "UPDATE icons
             SET current_preview_path = ?1,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?2 AND collection_id = ?3 AND deleted_at IS NULL",
            params![
                final_current.to_string_lossy().to_string(),
                icon_id,
                collection_id
            ],
        )?;
        if icon_rows != 1 {
            return Err(AppError::new(
                "clone_preview_state",
                "The target clone icon disappeared while linking its preview.",
            ));
        }
        for (piece_id, piece_path) in piece_ids.iter().zip(final_pieces.iter()) {
            let piece_rows = transaction.execute(
                "UPDATE icon_pieces
                 SET generated_preview_path = ?1,
                     export_status = 'ready',
                     updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE id = ?2 AND icon_id = ?3",
                params![piece_path.to_string_lossy().to_string(), piece_id, icon_id],
            )?;
            if piece_rows != 1 {
                return Err(AppError::new(
                    "clone_preview_state",
                    "A target clone piece disappeared while linking its preview.",
                ));
            }
        }
        Ok(())
    })();

    if result.is_err() {
        remove_directory_if_present(&paths.ai_activation_staging_dir, &staging_dir);
        remove_directory_if_present(&paths.ai_activation_previews_dir, &final_dir);
    }
    result
}

fn rebase_clone_preview_path(path: &Path, staging: &Path, final_dir: &Path) -> AppResult<PathBuf> {
    let relative = path.strip_prefix(staging).map_err(|_| {
        AppError::new(
            "clone_preview_path",
            "A rendered clone preview escaped its staging directory.",
        )
    })?;
    Ok(final_dir.join(relative))
}

/// Clone every source icon AI lineage onto a newly inserted icon.
///
/// Requests and candidates are immutable provenance/cost records and are therefore shared,
/// never duplicated. Version IDs, parent links, lineage fields, and the active state are owned
/// by the target icon so subsequent activation and rollback are independent. Ordinary icon
/// duplication pairs this with clone_source_free_root_provenance; AI new-icon creation does not.
pub(crate) fn clone_current_ai_lineage(
    transaction: &Transaction<'_>,
    source_icon_id: &str,
    target_icon_id: &str,
) -> AppResult<()> {
    let (
        source_file_id,
        source_lineage_id,
        source_generation,
        source_active_version,
        source_revision,
    ) = transaction
        .query_row(
            "SELECT i.source_file_id,
                        i.original_lineage_id,
                        i.original_lineage_generation,
                        st.active_version_id,
                        st.revision
                 FROM icons i
                 JOIN icon_ai_state st ON st.icon_id = i.id
                 WHERE i.id = ?1",
            [source_icon_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("복제할 AI 원본 아이콘을 찾을 수 없습니다."))?;
    let (
        target_source_file_id,
        mut target_lineage_id,
        mut target_generation,
        target_active,
        target_revision,
    ) = transaction
        .query_row(
            "SELECT i.source_file_id,
                        i.original_lineage_id,
                        i.original_lineage_generation,
                        st.active_version_id,
                        st.revision
                 FROM icons i
                 JOIN icon_ai_state st ON st.icon_id = i.id
                 WHERE i.id = ?1",
            [target_icon_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("AI 이력을 연결할 복제 아이콘을 찾을 수 없습니다."))?;

    if source_file_id != target_source_file_id {
        return Err(AppError::new(
            "ai_clone_source_mismatch",
            "AI 이력은 같은 원본 소스를 가진 아이콘에만 복제할 수 있습니다.",
        ));
    }
    if target_active.is_some() || target_revision != 0 {
        return Err(AppError::new(
            "ai_clone_target_not_fresh",
            "AI 이력을 복제할 대상 아이콘의 상태가 이미 변경되었습니다.",
        ));
    }
    if source_generation != target_generation {
        if source_generation <= target_generation {
            return Err(AppError::new(
                "ai_clone_generation_invalid",
                "The cloned AI lineage generation cannot be preserved.",
            ));
        }
        let previous_target_lineage_id = target_lineage_id.clone();
        let next_target_lineage_id = create_id("lineage");
        let updated = transaction.execute(
            "UPDATE icons
             SET original_lineage_id = ?1,
                 original_lineage_generation = ?2
             WHERE id = ?3
               AND original_lineage_id = ?4
               AND original_lineage_generation = ?5",
            params![
                next_target_lineage_id,
                source_generation,
                target_icon_id,
                previous_target_lineage_id,
                target_generation,
            ],
        )?;
        if updated != 1 {
            return Err(AppError::new(
                "ai_clone_state_conflict",
                "The clone AI lineage changed concurrently.",
            ));
        }
        let removed_transient = transaction.execute(
            "DELETE FROM icon_ai_lineages
             WHERE icon_id = ?1
               AND lineage_id = ?2
               AND lineage_generation = ?3
               AND original_source_file_id = ?4
               AND NOT EXISTS (
                 SELECT 1 FROM icon_ai_versions version
                 WHERE version.icon_id = icon_ai_lineages.icon_id
                   AND version.base_original_lineage_id = icon_ai_lineages.lineage_id
                   AND version.base_original_lineage_generation = icon_ai_lineages.lineage_generation
                   AND version.base_original_source_file_id = icon_ai_lineages.original_source_file_id
               )",
            params![
                target_icon_id,
                previous_target_lineage_id,
                target_generation,
                target_source_file_id,
            ],
        )?;
        if removed_transient != 1 {
            return Err(AppError::new(
                "ai_clone_lineage_conflict",
                "The transient clone lineage could not be removed.",
            ));
        }
        target_lineage_id = next_target_lineage_id;
        target_generation = source_generation;
    }

    let versions = {
        let mut statement = transaction.prepare(
            "SELECT id, parent_version_id,
                    base_original_source_file_id,
                    base_original_lineage_id,
                    base_original_lineage_generation
             FROM icon_ai_versions
             WHERE icon_id = ?1
             ORDER BY created_at ASC, id ASC",
        )?;
        let rows = statement
            .query_map([source_icon_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows
    };

    let source_lineages = {
        let mut statement = transaction.prepare(
            "SELECT original_source_file_id, lineage_id, lineage_generation
             FROM icon_ai_lineages
             WHERE icon_id = ?1
             ORDER BY lineage_generation ASC, created_at ASC, lineage_id ASC",
        )?;
        let rows = statement
            .query_map([source_icon_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows
    };

    let mut lineage_id_map = HashMap::new();
    for (base_source_file_id, base_lineage_id, base_generation) in &source_lineages {
        let key = (
            base_source_file_id.clone(),
            base_lineage_id.clone(),
            *base_generation,
        );
        if lineage_id_map.contains_key(&key) {
            continue;
        }

        let mapped_lineage_id = if base_source_file_id == &source_file_id
            && base_lineage_id == &source_lineage_id
            && *base_generation == source_generation
            && *base_generation == target_generation
        {
            target_lineage_id.clone()
        } else {
            let historical_lineage_id = create_id("lineage");
            transaction.execute(
                "INSERT INTO icon_ai_lineages (
                   icon_id, lineage_id, lineage_generation,
                   original_source_file_id, created_at
                 )
                 VALUES (
                   ?1, ?2, ?3, ?4,
                   strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 )",
                params![
                    target_icon_id,
                    historical_lineage_id,
                    base_generation,
                    base_source_file_id,
                ],
            )?;
            historical_lineage_id
        };
        lineage_id_map.insert(key, mapped_lineage_id);
    }

    let version_id_map = versions
        .iter()
        .map(|(source_version_id, _, _, _, _)| (source_version_id.clone(), create_id("ai_version")))
        .collect::<HashMap<_, _>>();

    for (
        source_version_id,
        source_parent_id,
        source_base_source_file_id,
        source_base_lineage_id,
        source_base_generation,
    ) in &versions
    {
        let target_version_id = version_id_map
            .get(source_version_id)
            .expect("AI version map was built from the same rows");
        let target_parent_id = source_parent_id
            .as_ref()
            .map(|parent_id| {
                version_id_map.get(parent_id).cloned().ok_or_else(|| {
                    AppError::new(
                        "ai_clone_dag_invalid",
                        "AI 버전 부모가 현재 원본 계보에 없어 복제할 수 없습니다.",
                    )
                })
            })
            .transpose()?;
        let target_base_lineage_id = lineage_id_map
            .get(&(
                source_base_source_file_id.clone(),
                source_base_lineage_id.clone(),
                *source_base_generation,
            ))
            .ok_or_else(|| {
                AppError::new(
                    "ai_clone_lineage_missing",
                    "The cloned AI version lineage was not registered.",
                )
            })?;

        let inserted = transaction.execute(
            "INSERT INTO icon_ai_versions (
               id, icon_id, candidate_id, base_original_source_file_id,
               base_original_lineage_id, base_original_lineage_generation,
               parent_version_id, effective_source_file_id, input_stage, apply_kind,
               provider_native_width, provider_native_height,
               target_canvas_width, target_canvas_height,
               normalization_recipe_json, normalization_recipe_hash,
               canvas_kind, animation_kind, payload_input_signature, created_at
             )
             SELECT
               ?1, ?2, candidate_id, ?3, ?4, ?5, ?6,
               effective_source_file_id, input_stage, apply_kind,
               provider_native_width, provider_native_height,
               target_canvas_width, target_canvas_height,
               normalization_recipe_json, normalization_recipe_hash,
               canvas_kind, animation_kind, payload_input_signature, created_at
             FROM icon_ai_versions
             WHERE id = ?7 AND icon_id = ?8",
            params![
                target_version_id,
                target_icon_id,
                source_base_source_file_id,
                target_base_lineage_id,
                source_base_generation,
                target_parent_id,
                source_version_id,
                source_icon_id,
            ],
        )?;
        if inserted != 1 {
            return Err(AppError::new(
                "ai_clone_version_missing",
                "복제할 AI 버전 행을 찾을 수 없습니다.",
            ));
        }
    }

    let target_active_version = source_active_version
        .as_ref()
        .map(|active_id| {
            version_id_map.get(active_id).cloned().ok_or_else(|| {
                AppError::new(
                    "ai_clone_active_version_invalid",
                    "활성 AI 버전이 현재 원본 계보에 없어 복제할 수 없습니다.",
                )
            })
        })
        .transpose()?;
    let cloned_revision = if target_active_version.is_some() {
        source_revision.max(1)
    } else {
        source_revision
    };
    let state_rows = transaction.execute(
        "UPDATE icon_ai_state
         SET active_version_id = ?1,
             revision = ?2,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE icon_id = ?3
           AND active_version_id IS NULL
           AND revision = 0",
        params![target_active_version, cloned_revision, target_icon_id],
    )?;
    if state_rows != 1 {
        return Err(AppError::new(
            "ai_clone_state_conflict",
            "복제 아이콘의 AI 상태가 동시에 변경되었습니다.",
        ));
    }

    Ok(())
}

/// Preserve source-free candidate ownership for an ordinary icon or collection clone.
/// Source-edit roots already remain discoverable through cloned icon AI versions.
pub(crate) fn clone_source_free_root_provenance(
    transaction: &Transaction<'_>,
    source_icon_id: &str,
    target_icon_id: &str,
) -> AppResult<()> {
    let inserted = transaction.execute(
        "INSERT INTO ai_icon_root_creations (
           icon_id, source_icon_id, candidate_id, request_item_id, creation_kind,
           normalization_recipe_hash, created_at
         )
         SELECT ?1, ?2, candidate_id, request_item_id, 'clone',
                normalization_recipe_hash, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         FROM ai_icon_root_creations
         WHERE icon_id = ?2
           AND creation_kind IN ('source_free', 'clone')
           AND request_item_id IS NOT NULL
           AND normalization_recipe_hash IS NULL",
        params![target_icon_id, source_icon_id],
    )?;
    if inserted > 1 {
        return Err(AppError::new(
            "ai_clone_root_conflict",
            "복제할 AI 루트 provenance가 하나보다 많습니다.",
        ));
    }
    Ok(())
}

pub(crate) fn clone_frame_sheet_gif_recipe(
    transaction: &Transaction<'_>,
    source_icon_id: &str,
    target_icon_id: &str,
) -> AppResult<()> {
    transaction.execute(
        "INSERT INTO frame_sheet_gif_recipes (
           id,
           generated_icon_id,
           original_sheet_filename,
           original_sheet_path,
           original_sheet_sha256,
           recipe_schema,
           grid_settings_json,
           frames_json,
           direction,
           loop_mode,
           loop_count,
           measured_byte_size,
           render_hash,
           created_at,
           updated_at
         )
         SELECT
           ?2,
           ?3,
           original_sheet_filename,
           original_sheet_path,
           original_sheet_sha256,
           recipe_schema,
           grid_settings_json,
           frames_json,
           direction,
           loop_mode,
           loop_count,
           measured_byte_size,
           render_hash,
           strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
           strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         FROM frame_sheet_gif_recipes
         WHERE generated_icon_id = ?1",
        rusqlite::params![
            source_icon_id,
            create_id("frame-gif-recipe"),
            target_icon_id
        ],
    )?;

    Ok(())
}
pub(crate) fn clone_effective_active_variants(
    transaction: &Transaction<'_>,
    paths: &AppPaths,
    target_collection_id: &str,
    source_icon_id: &str,
    target_icon_id: &str,
    piece_id_map: &HashMap<String, String>,
    profile_id_map: Option<&HashMap<String, String>>,
) -> AppResult<()> {
    let variants =
        optimization_repository::list_active_variants_for_icon(transaction, source_icon_id)?;
    let mut cloned_groups = HashSet::new();

    for variant in variants {
        let (Some(source_piece_id), Some(source_profile_id)) =
            (variant.piece_id.as_deref(), variant.profile_id.as_deref())
        else {
            continue;
        };
        let Some(target_piece_id) = piece_id_map.get(source_piece_id) else {
            return Err(AppError::new(
                "variant_clone_failed",
                "활성 최적화 결과의 조각 ID를 복제본에 연결할 수 없습니다.",
            ));
        };
        let target_profile_id = match profile_id_map {
            Some(profile_map) => profile_map.get(source_profile_id).ok_or_else(|| {
                AppError::new(
                    "variant_clone_failed",
                    "활성 최적화 결과의 프로필 ID를 복제본에 연결할 수 없습니다.",
                )
            })?,
            None => source_profile_id,
        };
        let variant_format = normalized_variant_format(&variant.format)?;
        let source_target = analyzer::load_target(
            transaction,
            source_icon_id,
            source_profile_id,
            Some(source_piece_id),
        )?;
        let source_is_compatible = variant.source_file_id.as_deref()
            == Some(source_target.source_file_id.as_str())
            && source_target.source_hash == variant.source_hash
            && source_target.crop_hash == variant.crop_hash
            && source_target.profile_hash == variant.profile_hash
            && source_target.output_format == variant_format
            && variant_artifact_at_path_matches(
                &variant,
                Path::new(&variant.path),
                &variant_format,
            )?;
        if !source_is_compatible {
            discard_incompatible_cloned_variant_preview(
                transaction,
                paths,
                target_collection_id,
                source_icon_id,
                target_icon_id,
                source_piece_id,
                target_piece_id,
                &variant.path,
            )?;
            continue;
        }

        let target = analyzer::load_target(
            transaction,
            target_icon_id,
            target_profile_id,
            Some(target_piece_id),
        )?;
        if target.source_file_id != source_target.source_file_id
            || target.source_hash != source_target.source_hash
            || target.crop_hash != source_target.crop_hash
            || !output_profiles_are_compatible(&source_target, &target)
            || variant.width != target.cell_width
            || variant.height != target.cell_height
        {
            discard_incompatible_cloned_variant_preview(
                transaction,
                paths,
                target_collection_id,
                source_icon_id,
                target_icon_id,
                source_piece_id,
                target_piece_id,
                &variant.path,
            )?;
            continue;
        }
        if !cloned_groups.insert((source_profile_id.to_string(), source_piece_id.to_string())) {
            continue;
        }

        let target_variant_id = create_id("variant");
        let Some(target_path) = clone_active_variant_file(
            paths,
            target_collection_id,
            target_icon_id,
            target_profile_id,
            target_piece_id,
            &target_variant_id,
            &variant_format,
            &variant.path,
        )?
        else {
            continue;
        };
        if !variant_artifact_at_path_matches(&variant, Path::new(&target_path), &variant_format)? {
            let _ = fs::remove_file(&target_path);
            discard_incompatible_cloned_variant_preview(
                transaction,
                paths,
                target_collection_id,
                source_icon_id,
                target_icon_id,
                source_piece_id,
                target_piece_id,
                &variant.path,
            )?;
            continue;
        }

        let inserted_variant = optimization_repository::insert_variant(
            transaction,
            &optimization_repository::NewProcessedAssetVariant {
                id: target_variant_id.clone(),
                icon_id: target_icon_id.to_string(),
                piece_id: Some(target_piece_id.clone()),
                profile_id: Some(target_profile_id.to_string()),
                source_file_id: Some(target.source_file_id.clone()),
                kind: variant.kind.clone(),
                preset: variant.preset.clone(),
                path: target_path.clone(),
                format: variant_format.clone(),
                width: variant.width,
                height: variant.height,
                byte_size: variant.byte_size,
                frame_count: variant.frame_count,
                duration_ms: variant.duration_ms,
                loop_mode: variant.loop_mode.clone(),
                settings_json: variant.settings_json.clone(),
                source_hash: target.source_hash,
                crop_hash: target.crop_hash,
                profile_hash: target.profile_hash,
                settings_hash: variant.settings_hash.clone(),
            },
        )?;
        if inserted_variant.output_sha256.as_deref() != variant.output_sha256.as_deref() {
            transaction.execute(
                "DELETE FROM processed_asset_variants WHERE id = ?1",
                [&target_variant_id],
            )?;
            let _ = fs::remove_file(&target_path);
            discard_incompatible_cloned_variant_preview(
                transaction,
                paths,
                target_collection_id,
                source_icon_id,
                target_icon_id,
                source_piece_id,
                target_piece_id,
                &variant.path,
            )?;
            continue;
        }
        transaction.execute(
            "UPDATE processed_asset_variants
             SET is_active_for_export = 0
             WHERE icon_id = ?1
               AND profile_id = ?2
               AND piece_id = ?3",
            params![target_icon_id, target_profile_id, target_piece_id],
        )?;
        let activated = transaction.execute(
            "UPDATE processed_asset_variants
             SET is_active_for_export = 1
             WHERE id = ?1
               AND icon_id = ?2
               AND profile_id = ?3
               AND piece_id = ?4",
            params![
                target_variant_id,
                target_icon_id,
                target_profile_id,
                target_piece_id
            ],
        )?;
        if activated != 1 {
            return Err(AppError::new(
                "variant_clone_failed",
                "복제한 최적화 결과를 활성 상태로 전환할 수 없습니다.",
            ));
        }
        repoint_valid_cloned_variant_preview(
            transaction,
            paths,
            target_collection_id,
            source_icon_id,
            target_icon_id,
            source_piece_id,
            target_piece_id,
            &variant.path,
            &target_path,
        )?;
    }

    Ok(())
}
fn variant_artifact_at_path_matches(
    variant: &optimization_repository::ProcessedAssetVariantRecord,
    path: &Path,
    format: &str,
) -> AppResult<bool> {
    let Some(expected_sha256) = variant.output_sha256.as_deref() else {
        return Ok(false);
    };
    let Ok(metadata) = fs::metadata(path) else {
        return Ok(false);
    };
    if i64::try_from(metadata.len()).unwrap_or(i64::MAX) != variant.byte_size {
        return Ok(false);
    }
    let Ok(analysis) = analyzer::analyze_file(path, format) else {
        return Ok(false);
    };
    if analysis.width != variant.width || analysis.height != variant.height {
        return Ok(false);
    }
    Ok(sha256_file(path)? == expected_sha256)
}

fn output_profiles_are_compatible(
    source: &analyzer::OptimizationTarget,
    target: &analyzer::OptimizationTarget,
) -> bool {
    source.output_format == target.output_format
        && source.cell_width == target.cell_width
        && source.cell_height == target.cell_height
        && source.profile.target_format == target.profile.target_format
        && source.profile.target_cell_width == target.profile.target_cell_width
        && source.profile.target_cell_height == target.profile.target_cell_height
        && source.profile.max_bytes == target.profile.max_bytes
        && source.profile.allowed_formats == target.profile.allowed_formats
}

#[allow(clippy::too_many_arguments)]
fn repoint_valid_cloned_variant_preview(
    transaction: &Transaction<'_>,
    paths: &AppPaths,
    collection_id: &str,
    source_icon_id: &str,
    target_icon_id: &str,
    source_piece_id: &str,
    target_piece_id: &str,
    source_variant_path: &str,
    target_variant_path: &str,
) -> AppResult<()> {
    let source_current_preview: Option<String> = transaction.query_row(
        "SELECT current_preview_path FROM icons WHERE id = ?1",
        [source_icon_id],
        |row| row.get(0),
    )?;
    if source_current_preview.as_deref() == Some(source_variant_path) {
        let detached_target_preview: Option<String> = transaction.query_row(
            "SELECT current_preview_path FROM icons WHERE id = ?1",
            [target_icon_id],
            |row| row.get(0),
        )?;
        transaction.execute(
            "UPDATE icons
             SET current_preview_path = ?1,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?2",
            params![target_variant_path, target_icon_id],
        )?;
        remove_cloned_preview_file(
            paths,
            collection_id,
            target_icon_id,
            detached_target_preview.as_deref(),
        );
    }

    let source_piece_preview: Option<String> = transaction.query_row(
        "SELECT generated_preview_path
         FROM icon_pieces
         WHERE id = ?1 AND icon_id = ?2",
        params![source_piece_id, source_icon_id],
        |row| row.get(0),
    )?;
    if source_piece_preview.as_deref() == Some(source_variant_path) {
        let detached_target_preview: Option<String> = transaction.query_row(
            "SELECT generated_preview_path
             FROM icon_pieces
             WHERE id = ?1 AND icon_id = ?2",
            params![target_piece_id, target_icon_id],
            |row| row.get(0),
        )?;
        transaction.execute(
            "UPDATE icon_pieces
             SET generated_preview_path = ?1,
                 export_status = 'ready',
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?2 AND icon_id = ?3",
            params![target_variant_path, target_piece_id, target_icon_id],
        )?;
        remove_cloned_preview_file(
            paths,
            collection_id,
            target_icon_id,
            detached_target_preview.as_deref(),
        );
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn discard_incompatible_cloned_variant_preview(
    _transaction: &Transaction<'_>,
    _paths: &AppPaths,
    _collection_id: &str,
    _source_icon_id: &str,
    _target_icon_id: &str,
    _source_piece_id: &str,
    _target_piece_id: &str,
    _variant_path: &str,
) -> AppResult<()> {
    // The target already owns a freshly rendered effective preview. An unavailable or
    // incompatible promoted optimization is optional cache state and must not erase it.
    Ok(())
}
fn remove_cloned_preview_file(
    paths: &AppPaths,
    collection_id: &str,
    icon_id: &str,
    path: Option<&str>,
) {
    let (Some(path), Ok(preview_root)) = (
        path,
        icon_preview_root(paths, collection_id, icon_id)
            .map(|root| root.join(CLONED_PREVIEW_DIRECTORY)),
    ) else {
        return;
    };
    let (Ok(canonical_root), Ok(canonical_path)) =
        (preview_root.canonicalize(), Path::new(path).canonicalize())
    else {
        return;
    };
    if canonical_path.starts_with(canonical_root) && canonical_path.is_file() {
        let _ = fs::remove_file(canonical_path);
    }
}

fn sha256_file(path: &Path) -> AppResult<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

pub(crate) fn cleanup_cloned_icon_previews(paths: &AppPaths, collection_id: &str, icon_id: &str) {
    if let Ok(target) = icon_preview_root(paths, collection_id, icon_id) {
        remove_directory_if_present(
            &paths.collection_previews_dir,
            &target.join(CLONED_PREVIEW_DIRECTORY),
        );
    }
    if let Ok(target) = cloned_variant_icon_root(paths, collection_id, icon_id) {
        remove_directory_if_present(&paths.processed_variants_dir, &target);
    }
    if let (Ok(collection_component), Ok(icon_component)) = (
        safe_component(collection_id, "collection ID"),
        safe_component(icon_id, "icon ID"),
    ) {
        let target = paths
            .ai_activation_previews_dir
            .join(collection_component)
            .join(icon_component);
        remove_directory_if_present(&paths.ai_activation_previews_dir, &target);
    }
}

pub(crate) fn cleanup_cloned_collection_previews(paths: &AppPaths, collection_id: &str) {
    let Ok(collection_component) = safe_component(collection_id, "collection ID") else {
        return;
    };
    let preview_target = paths.collection_previews_dir.join(collection_component);
    remove_directory_if_present(&paths.collection_previews_dir, &preview_target);
    let variant_target = paths
        .processed_variants_dir
        .join(CLONED_VARIANT_DIRECTORY)
        .join(collection_component);
    remove_directory_if_present(&paths.processed_variants_dir, &variant_target);
    let ai_preview_target = paths.ai_activation_previews_dir.join(collection_component);
    remove_directory_if_present(&paths.ai_activation_previews_dir, &ai_preview_target);
}
#[allow(clippy::too_many_arguments)]
fn clone_active_variant_file(
    paths: &AppPaths,
    collection_id: &str,
    icon_id: &str,
    profile_id: &str,
    piece_id: &str,
    variant_id: &str,
    format: &str,
    source_path: &str,
) -> AppResult<Option<String>> {
    let source = PathBuf::from(source_path);
    if !source.is_file() {
        return Ok(None);
    }
    let canonical_source = source.canonicalize().map_err(|error| {
        AppError::new(
            "variant_clone_failed",
            format!("복제할 활성 최적화 파일을 열 수 없습니다: {error}"),
        )
    })?;
    let canonical_root = paths.root.canonicalize()?;
    if !canonical_source.starts_with(&canonical_root) || !canonical_source.is_file() {
        return Err(AppError::new(
            "variant_clone_failed",
            "앱 라이브러리 밖의 활성 최적화 파일은 복제할 수 없습니다.",
        ));
    }
    let byte_size = fs::metadata(&canonical_source)?.len();
    if byte_size > MAX_IMPORT_FILE_BYTES as u64 {
        return Err(AppError::new(
            "variant_clone_failed",
            "복제할 활성 최적화 파일이 64MB 안전 한도를 초과합니다.",
        ));
    }

    let extension = normalized_variant_format(format)?;
    let profile_component = safe_component(profile_id, "프로필 ID")?;
    let piece_component = safe_component(piece_id, "조각 ID")?;
    let variant_component = safe_component(variant_id, "variant ID")?;
    let target_directory = cloned_variant_icon_root(paths, collection_id, icon_id)?
        .join(profile_component)
        .join(piece_component);
    prepare_target_directory(&paths.processed_variants_dir, &target_directory)?;
    let target = target_directory.join(format!("{variant_component}.{extension}"));
    if target.exists() {
        return Err(AppError::new(
            "variant_clone_failed",
            "복제할 활성 최적화 파일의 대상 경로가 이미 존재합니다.",
        ));
    }

    fs::copy(&canonical_source, &target).map_err(|error| {
        AppError::new(
            "variant_clone_failed",
            format!("활성 최적화 파일을 복제하지 못했습니다: {error}"),
        )
    })?;
    Ok(Some(target.to_string_lossy().to_string()))
}

fn cloned_variant_icon_root(
    paths: &AppPaths,
    collection_id: &str,
    icon_id: &str,
) -> AppResult<PathBuf> {
    let collection_component = safe_component(collection_id, "모음 ID")?;
    let icon_component = safe_component(icon_id, "아이콘 ID")?;
    Ok(paths
        .processed_variants_dir
        .join(CLONED_VARIANT_DIRECTORY)
        .join(collection_component)
        .join(icon_component))
}

fn normalized_variant_format(format: &str) -> AppResult<String> {
    match format.trim().to_ascii_lowercase().as_str() {
        "gif" => Ok("gif".to_string()),
        "png" => Ok("png".to_string()),
        "jpg" | "jpeg" => Ok("jpg".to_string()),
        _ => Err(AppError::new(
            "variant_clone_failed",
            "복제할 활성 최적화 파일 형식이 지원되지 않습니다.",
        )),
    }
}
fn icon_preview_root(paths: &AppPaths, collection_id: &str, icon_id: &str) -> AppResult<PathBuf> {
    let collection_component = safe_component(collection_id, "모음 ID")?;
    let icon_component = safe_component(icon_id, "아이콘 ID")?;
    Ok(paths
        .collection_previews_dir
        .join(collection_component)
        .join(icon_component))
}

fn safe_component<'a>(value: &'a str, label: &str) -> AppResult<&'a str> {
    let mut components = Path::new(value).components();
    let is_single_normal_component =
        matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none();
    if value.is_empty() || !is_single_normal_component {
        return Err(AppError::new(
            "preview_clone_failed",
            format!("{label}가 안전한 경로 구성 요소가 아닙니다."),
        ));
    }
    Ok(value)
}

fn prepare_target_directory(allowed_root: &Path, target: &Path) -> AppResult<()> {
    fs::create_dir_all(allowed_root)?;
    let canonical_root = allowed_root.canonicalize()?;
    let relative = target.strip_prefix(allowed_root).map_err(|_| {
        AppError::new(
            "preview_clone_failed",
            "복제 미리보기 대상 경로가 허용된 저장 폴더 밖에 있습니다.",
        )
    })?;
    let mut current = allowed_root.to_path_buf();
    let mut expected = canonical_root.clone();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return Err(AppError::new(
                "preview_clone_failed",
                "복제 미리보기 대상 경로에 안전하지 않은 구성 요소가 있습니다.",
            ));
        };
        current.push(component);
        expected.push(component);
        if current.exists() {
            let canonical_current = current.canonicalize()?;
            if canonical_current != expected || !canonical_current.is_dir() {
                return Err(AppError::new(
                    "preview_clone_failed",
                    "복제 미리보기 대상 경로에 안전하지 않은 링크 또는 파일이 포함되어 있습니다.",
                ));
            }
        } else {
            fs::create_dir(&current)?;
        }
    }

    Ok(())
}

fn remove_directory_if_present(allowed_root: &Path, target: &Path) {
    let (Ok(canonical_root), Ok(canonical_target)) =
        (allowed_root.canonicalize(), target.canonicalize())
    else {
        return;
    };
    if canonical_target != canonical_root
        && canonical_target.starts_with(&canonical_root)
        && canonical_target.is_dir()
    {
        let _ = fs::remove_dir_all(canonical_target);
    }
}
