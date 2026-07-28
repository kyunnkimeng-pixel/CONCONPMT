use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

use crate::db::repositories::ai::{
    ai_repair_required, get_ai_review_state, load_and_validate_source,
    resolve_effective_visual_source, VisualSourceRecord,
};
use crate::db::repositories::ai_candidate_normalization::{self, PreparedCandidateNormalization};
use crate::db::repositories::ai_managed_artifacts;
use crate::db::repositories::editor as editor_repository;
use crate::db::repositories::effects as effect_repository;
use crate::db::repositories::motion as motion_repository;
use crate::error::{AppError, AppResult};
use crate::ids::create_id;
use crate::imaging::effects::EffectRecipe;
use crate::imaging::export_render::ExportCropRect;
use crate::imaging::geometry::piece_roles;
use crate::imaging::motion::MotionRecipe;
use crate::imaging::preview::{
    generate_icon_preview_in_directory, CropRect, GeneratePreviewRequest, GeneratedPreview,
};
use crate::imaging::text_overlay::{text_overlay_from_fields, TextOverlayRenderSpec};
use crate::imaging::transform::ImageTransform;
use crate::models::{
    ActivateAiCandidatePayload, AiSourceMutationResultDto, RepairAiToOriginalPayload,
    RestoreAiVersionPayload,
};
use crate::optimization::cache::{hash_text, render_recipe_crop_hash};
use crate::paths::AppPaths;

#[derive(Debug, Clone)]
struct RenderRecipe {
    shape: String,
    crop: CropRect,
    cell_width: i64,
    cell_height: i64,
    transform: ImageTransform,
    gif_loop_mode: String,
    gif_loop_count: Option<i64>,
    text_overlay: Option<TextOverlayRenderSpec>,
    effects: EffectRecipe,
    motion: MotionRecipe,
    piece_ids: Vec<String>,
    max_bytes: i64,
    signature: String,
}

#[derive(Debug)]
pub(crate) struct PreparedSourcePreview {
    collection_id: String,
    output_icon_id: String,
    render_source: VisualSourceRecord,
    recipe: RenderRecipe,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RenderedPreviewInspection {
    pub final_render_width: i64,
    pub final_render_height: i64,
    pub piece_width: i64,
    pub piece_height: i64,
    pub largest_piece_bytes: u64,
    pub max_piece_bytes: u64,
}

#[derive(Debug, Clone)]
struct CandidateActivation {
    candidate_id: String,
    payload_input_signature: String,
    source: VisualSourceRecord,
}

#[derive(Debug, Clone)]
struct ActivationState {
    original_source: VisualSourceRecord,
    original_lineage_id: String,
    original_lineage_generation: i64,
    active_version_id: Option<String>,
    activation_revision: i64,
}

pub(crate) fn cleanup_crash_orphans(connection: &Connection, paths: &AppPaths) -> AppResult<usize> {
    const CRASH_ORPHAN_AGE: Duration = Duration::from_secs(24 * 60 * 60);
    let referenced_paths = {
        let mut statement = connection.prepare(
            "SELECT current_preview_path AS path FROM icons WHERE current_preview_path IS NOT NULL
             UNION
             SELECT generated_preview_path AS path
             FROM icon_pieces WHERE generated_preview_path IS NOT NULL",
        )?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<HashSet<_>, _>>()?;
        rows
    };
    let now = SystemTime::now();
    let mut removed = 0_usize;

    for directory in direct_child_directories(&paths.ai_activation_staging_dir) {
        if is_older_than(&directory, now, CRASH_ORPHAN_AGE)
            && ai_managed_artifacts::remove_owned_directory_if_present(
                &paths.root,
                &paths.ai_activation_staging_dir,
                &directory,
            )
            .is_ok()
        {
            removed += 1;
        }
    }

    for collection_dir in direct_child_directories(&paths.ai_activation_previews_dir) {
        for icon_dir in direct_child_directories(&collection_dir) {
            for operation_dir in direct_child_directories(&icon_dir) {
                let is_referenced = referenced_paths
                    .iter()
                    .any(|path| Path::new(path).starts_with(&operation_dir));
                if !is_referenced
                    && is_older_than(&operation_dir, now, CRASH_ORPHAN_AGE)
                    && ai_managed_artifacts::remove_owned_directory_if_present(
                        &paths.root,
                        &paths.ai_activation_previews_dir,
                        &operation_dir,
                    )
                    .is_ok()
                {
                    removed += 1;
                }
            }
        }
    }
    Ok(removed)
}

fn direct_child_directories(root: &Path) -> Vec<PathBuf> {
    fs::read_dir(root)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let metadata = fs::symlink_metadata(entry.path()).ok()?;
            (metadata.is_dir() && !metadata.file_type().is_symlink()).then(|| entry.path())
        })
        .collect()
}

fn is_older_than(path: &Path, now: SystemTime, minimum_age: Duration) -> bool {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| now.duration_since(modified).ok())
        .is_some_and(|age| age >= minimum_age)
}

pub(crate) fn activate_candidate(
    connection: &mut Connection,
    paths: &AppPaths,
    collection_id: &str,
    payload: &ActivateAiCandidatePayload,
) -> AppResult<AiSourceMutationResultDto> {
    let operation_id = create_id("ai_normalization_apply");
    let normalization_dir = paths.ai_activation_staging_dir.join(operation_id);
    let result = (|| {
        let normalization = ai_candidate_normalization::prepare_candidate_normalization(
            connection,
            paths,
            collection_id,
            &payload.icon_id,
            &payload.candidate_id,
            payload.expected_revision,
            &payload.normalization,
            &normalization_dir,
        )?;
        ai_candidate_normalization::ensure_preview_signature(
            payload.expected_preview_signature.as_deref(),
            &normalization.preview_signature,
        )?;
        ai_candidate_normalization::ensure_current_icon_compatible(
            &normalization.current_icon_compatibility,
        )?;
        if normalization.is_current_recipe {
            return Err(AppError::new(
                "ai_candidate_already_active",
                "이 후보와 크기 맞춤 설정은 이미 현재 편집 소스로 적용되어 있습니다.",
            ));
        }
        if let Some(version_id) = normalization.existing_version_id.as_deref() {
            let current = load_activation_state(connection, collection_id, &payload.icon_id)?;
            let target = load_version_source(
                connection,
                &payload.icon_id,
                version_id,
                &current.original_lineage_id,
                current.original_lineage_generation,
            )?;
            return activate_source(
                connection,
                paths,
                collection_id,
                &payload.icon_id,
                payload.expected_revision,
                target,
                None,
                Some(version_id),
                Some(&normalization.native_recipe_signature),
                None,
            );
        }
        let candidate = CandidateActivation {
            candidate_id: normalization.candidate_id.clone(),
            payload_input_signature: normalization.payload_input_signature.clone(),
            source: normalization.raw_source.clone(),
        };
        activate_source(
            connection,
            paths,
            collection_id,
            &payload.icon_id,
            payload.expected_revision,
            normalization.effective_source.clone(),
            Some((create_id("ai_version"), candidate)),
            None,
            Some(&normalization.native_recipe_signature),
            Some(&normalization),
        )
    })();
    let _ = ai_managed_artifacts::remove_owned_directory_if_present(
        &paths.root,
        &paths.ai_activation_staging_dir,
        &normalization_dir,
    );
    result
}

pub(crate) fn restore_version(
    connection: &mut Connection,
    paths: &AppPaths,
    collection_id: &str,
    payload: &RestoreAiVersionPayload,
) -> AppResult<AiSourceMutationResultDto> {
    let current = load_activation_state(connection, collection_id, &payload.icon_id)?;
    ensure_expected_revision(current.activation_revision, payload.expected_revision)?;
    let target = match payload.version_id.as_deref() {
        None => current.original_source.clone(),
        Some(version_id) => load_version_source(
            connection,
            &payload.icon_id,
            version_id,
            &current.original_lineage_id,
            current.original_lineage_generation,
        )?,
    };
    activate_source(
        connection,
        paths,
        collection_id,
        &payload.icon_id,
        payload.expected_revision,
        target,
        None,
        payload.version_id.as_deref(),
        None,
        None,
    )
}

pub(crate) fn repair_to_original(
    connection: &mut Connection,
    paths: &AppPaths,
    collection_id: &str,
    payload: &RepairAiToOriginalPayload,
) -> AppResult<()> {
    let current = load_activation_state(connection, collection_id, &payload.icon_id)?;
    restore_version(
        connection,
        paths,
        collection_id,
        &RestoreAiVersionPayload {
            icon_id: payload.icon_id.clone(),
            version_id: None,
            expected_revision: current.activation_revision,
        },
    )
    .map(|_| ())
}

pub(crate) fn current_recipe_signature(
    connection: &Connection,
    collection_id: &str,
    icon_id: &str,
    source: &VisualSourceRecord,
    activation_revision: i64,
) -> AppResult<String> {
    let lineage = connection.query_row(
        "SELECT original_lineage_id, original_lineage_generation
         FROM icons
         WHERE id = ?1
           AND collection_id = ?2
           AND deleted_at IS NULL",
        params![icon_id, collection_id],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
    )?;
    Ok(load_render_recipe(
        connection,
        collection_id,
        icon_id,
        source,
        activation_revision,
        &lineage.0,
        lineage.1,
    )?
    .signature)
}

pub(crate) fn render_effective_preview_to_directory(
    connection: &Connection,
    staging_dir: &Path,
    collection_id: &str,
    icon_id: &str,
) -> AppResult<GeneratedPreview> {
    let effective = resolve_effective_visual_source(connection, collection_id, icon_id)?;
    let recipe = load_render_recipe(
        connection,
        collection_id,
        icon_id,
        &effective.render_source,
        effective.activation_revision,
        &effective.original_lineage_id,
        effective.original_lineage_generation,
    )?;
    let generated = render_preview(
        staging_dir,
        collection_id,
        icon_id,
        &effective.render_source,
        &recipe,
    )?;
    validate_preview_structure(&generated, &recipe)?;
    Ok(generated)
}
pub(crate) fn prepare_source_preview(
    connection: &Connection,
    collection_id: &str,
    recipe_icon_id: &str,
    output_icon_id: &str,
    render_source: &VisualSourceRecord,
    native_source: &VisualSourceRecord,
    activation_revision: i64,
    lineage_id: &str,
    lineage_generation: i64,
    expected_native_recipe_signature: &str,
) -> AppResult<PreparedSourcePreview> {
    let recipe = load_render_recipe(
        connection,
        collection_id,
        recipe_icon_id,
        native_source,
        activation_revision,
        lineage_id,
        lineage_generation,
    )?;
    if recipe.signature != expected_native_recipe_signature {
        return Err(AppError::new(
            "ai_normalization_preview_stale",
            "편집값이 변경되었습니다. 현재 설정으로 규격화 미리보기를 다시 확인해 주세요.",
        ));
    }
    Ok(PreparedSourcePreview {
        collection_id: collection_id.to_string(),
        output_icon_id: output_icon_id.to_string(),
        render_source: render_source.clone(),
        recipe,
    })
}

pub(crate) fn render_prepared_source_preview(
    staging_dir: &Path,
    prepared: &PreparedSourcePreview,
) -> AppResult<GeneratedPreview> {
    let generated = render_preview(
        staging_dir,
        &prepared.collection_id,
        &prepared.output_icon_id,
        &prepared.render_source,
        &prepared.recipe,
    )?;
    validate_preview_structure(&generated, &prepared.recipe)?;
    Ok(generated)
}

pub(crate) fn inspect_prepared_source_preview(
    generated: &GeneratedPreview,
    prepared: &PreparedSourcePreview,
) -> AppResult<RenderedPreviewInspection> {
    validate_preview_structure(generated, &prepared.recipe)?;
    let (final_render_width, final_render_height) =
        image::image_dimensions(&generated.current_preview_path)?;
    let first_piece = generated.piece_paths.first().ok_or_else(|| {
        AppError::new(
            "ai_activation_preview",
            "생성된 AI 미리보기 조각을 확인할 수 없습니다.",
        )
    })?;
    let (piece_width, piece_height) = image::image_dimensions(first_piece)?;
    let mut largest_piece_bytes = 0_u64;
    for path in &generated.piece_paths {
        let dimensions = image::image_dimensions(path)?;
        if dimensions != (piece_width, piece_height) {
            return Err(AppError::new(
                "ai_activation_preview",
                "생성된 AI 미리보기 조각의 크기가 서로 다릅니다.",
            ));
        }
        largest_piece_bytes = largest_piece_bytes.max(fs::metadata(path)?.len());
    }
    Ok(RenderedPreviewInspection {
        final_render_width: i64::from(final_render_width),
        final_render_height: i64::from(final_render_height),
        piece_width: i64::from(piece_width),
        piece_height: i64::from(piece_height),
        largest_piece_bytes,
        max_piece_bytes: u64::try_from(prepared.recipe.max_bytes.max(1)).unwrap_or(u64::MAX),
    })
}

pub(crate) fn repair_effective_preview(
    connection: &Connection,
    paths: &AppPaths,
    collection_id: &str,
    icon_id: &str,
    deactivate_variant_id: Option<&str>,
) -> AppResult<bool> {
    match try_repair_effective_preview(
        connection,
        paths,
        collection_id,
        icon_id,
        deactivate_variant_id,
    ) {
        Ok(()) => Ok(true),
        Err(_) => {
            clear_effective_preview_state(connection, icon_id, deactivate_variant_id)?;
            Ok(false)
        }
    }
}

fn try_repair_effective_preview(
    connection: &Connection,
    paths: &AppPaths,
    collection_id: &str,
    icon_id: &str,
    deactivate_variant_id: Option<&str>,
) -> AppResult<()> {
    let effective = resolve_effective_visual_source(connection, collection_id, icon_id)?;
    let recipe = load_render_recipe(
        connection,
        collection_id,
        icon_id,
        &effective.render_source,
        effective.activation_revision,
        &effective.original_lineage_id,
        effective.original_lineage_generation,
    )?;
    let operation_id = create_id("effective_preview_repair");
    let staging_dir = paths.ai_activation_staging_dir.join(&operation_id);
    let final_dir = paths
        .ai_activation_previews_dir
        .join(collection_id)
        .join(icon_id)
        .join(&operation_id);
    ai_managed_artifacts::remove_owned_directory_if_present(
        &paths.root,
        &paths.ai_activation_staging_dir,
        &staging_dir,
    )?;
    let prepared_staging_dir = ai_managed_artifacts::prepare_owned_directory(
        &paths.root,
        &paths.ai_activation_staging_dir,
        &staging_dir,
    )?;
    let generated = match render_preview(
        &prepared_staging_dir,
        collection_id,
        icon_id,
        &effective.render_source,
        &recipe,
    )
    .and_then(|generated| {
        validate_preview_structure(&generated, &recipe)?;
        Ok(generated)
    }) {
        Ok(generated) => generated,
        Err(error) => {
            let _ = ai_managed_artifacts::remove_owned_directory_if_present(
                &paths.root,
                &paths.ai_activation_staging_dir,
                &staging_dir,
            );
            return Err(error);
        }
    };

    let transaction = match connection.unchecked_transaction() {
        Ok(transaction) => transaction,
        Err(error) => {
            let _ = ai_managed_artifacts::remove_owned_directory_if_present(
                &paths.root,
                &paths.ai_activation_staging_dir,
                &staging_dir,
            );
            return Err(error.into());
        }
    };
    let commit_result = (|| -> AppResult<()> {
        let current = resolve_effective_visual_source(&transaction, collection_id, icon_id)?;
        let current_recipe = load_render_recipe(
            &transaction,
            collection_id,
            icon_id,
            &current.render_source,
            current.activation_revision,
            &current.original_lineage_id,
            current.original_lineage_generation,
        )?;
        if current.render_source.id != effective.render_source.id
            || current.render_source.sha256 != effective.render_source.sha256
            || current_recipe.signature != recipe.signature
        {
            return Err(AppError::new(
                "effective_preview_conflict",
                "미리보기를 복구하는 동안 유효 소스 또는 편집 설정이 변경되었습니다.",
            ));
        }

        let promoted_dir = ai_managed_artifacts::promote_owned_directory(
            &paths.root,
            &paths.ai_activation_staging_dir,
            &paths.ai_activation_previews_dir,
            &staging_dir,
            &final_dir,
        )?;
        let final_preview = rebase_artifact_path(
            &generated.current_preview_path,
            &prepared_staging_dir,
            &promoted_dir,
        )?;
        let final_pieces = generated
            .piece_paths
            .iter()
            .map(|path| rebase_artifact_path(path, &prepared_staging_dir, &promoted_dir))
            .collect::<AppResult<Vec<_>>>()?;

        if let Some(variant_id) = deactivate_variant_id {
            transaction.execute(
                "UPDATE processed_asset_variants
                 SET is_active_for_export = 0
                 WHERE id = ?1
                   AND icon_id = ?2",
                params![variant_id, icon_id],
            )?;
        }
        let icon_rows = transaction.execute(
            "UPDATE icons
             SET current_preview_path = ?1,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?2
               AND collection_id = ?3
               AND deleted_at IS NULL",
            params![path_string(&final_preview), icon_id, collection_id],
        )?;
        if icon_rows != 1 {
            return Err(ai_repair_required());
        }
        for (piece_id, piece_path) in recipe.piece_ids.iter().zip(final_pieces.iter()) {
            let piece_rows = transaction.execute(
                "UPDATE icon_pieces
                 SET generated_preview_path = ?1,
                     last_export_path = NULL,
                     export_status = 'ready',
                     updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE id = ?2
                   AND icon_id = ?3",
                params![path_string(piece_path), piece_id, icon_id],
            )?;
            if piece_rows != 1 {
                return Err(ai_repair_required());
            }
        }
        transaction.commit()?;
        Ok(())
    })();

    if commit_result.is_err() {
        let _ = ai_managed_artifacts::remove_owned_directory_if_present(
            &paths.root,
            &paths.ai_activation_staging_dir,
            &staging_dir,
        );
        let _ = ai_managed_artifacts::remove_owned_directory_if_present(
            &paths.root,
            &paths.ai_activation_previews_dir,
            &final_dir,
        );
    }
    commit_result
}

fn clear_effective_preview_state(
    connection: &Connection,
    icon_id: &str,
    deactivate_variant_id: Option<&str>,
) -> AppResult<()> {
    let transaction = connection.unchecked_transaction()?;
    if let Some(variant_id) = deactivate_variant_id {
        transaction.execute(
            "UPDATE processed_asset_variants
             SET is_active_for_export = 0
             WHERE id = ?1
               AND icon_id = ?2",
            params![variant_id, icon_id],
        )?;
    }
    transaction.execute(
        "UPDATE icons
         SET current_preview_path = NULL,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?1
           AND deleted_at IS NULL",
        [icon_id],
    )?;
    transaction.execute(
        "UPDATE icon_pieces
         SET generated_preview_path = NULL,
             last_export_path = NULL,
             export_status = 'not_exported',
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE icon_id = ?1",
        [icon_id],
    )?;
    transaction.commit()?;
    Ok(())
}

fn activate_source(
    connection: &mut Connection,
    paths: &AppPaths,
    collection_id: &str,
    icon_id: &str,
    expected_revision: i64,
    target_source: VisualSourceRecord,
    new_version: Option<(String, CandidateActivation)>,
    existing_version_id: Option<&str>,
    expected_native_recipe_signature: Option<&str>,
    new_normalization: Option<&PreparedCandidateNormalization>,
) -> AppResult<AiSourceMutationResultDto> {
    let current = load_activation_state(connection, collection_id, icon_id)?;
    ensure_expected_revision(current.activation_revision, expected_revision)?;
    if expected_native_recipe_signature.is_some() {
        let effective = resolve_effective_visual_source(connection, collection_id, icon_id)?;
        ensure_native_recipe_signature(
            connection,
            collection_id,
            icon_id,
            &effective,
            expected_native_recipe_signature,
        )?;
    }
    let recipe = load_render_recipe(
        connection,
        collection_id,
        icon_id,
        &target_source,
        expected_revision,
        &current.original_lineage_id,
        current.original_lineage_generation,
    )?;
    let operation_id = create_id("ai_activation");
    let staging_dir = paths.ai_activation_staging_dir.join(&operation_id);
    let final_dir = paths
        .ai_activation_previews_dir
        .join(collection_id)
        .join(icon_id)
        .join(&operation_id);
    ai_managed_artifacts::remove_owned_directory_if_present(
        &paths.root,
        &paths.ai_activation_staging_dir,
        &staging_dir,
    )?;
    let prepared_staging_dir = ai_managed_artifacts::prepare_owned_directory(
        &paths.root,
        &paths.ai_activation_staging_dir,
        &staging_dir,
    )?;
    let generated = match render_preview(
        &prepared_staging_dir,
        collection_id,
        icon_id,
        &target_source,
        &recipe,
    )
    .and_then(|generated| {
        validate_outputs(&generated, &recipe)?;
        Ok(generated)
    }) {
        Ok(generated) => generated,
        Err(error) => {
            let _ = ai_managed_artifacts::remove_owned_directory_if_present(
                &paths.root,
                &paths.ai_activation_staging_dir,
                &staging_dir,
            );
            return Err(error);
        }
    };
    let source_artifact_snapshot = match new_normalization {
        Some(normalization) => normalization.source_artifact_snapshot(connection, paths)?,
        None => None,
    };

    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let commit_result = (|| -> AppResult<AiSourceMutationResultDto> {
        let transaction_current = load_activation_state(&transaction, collection_id, icon_id)?;
        ensure_expected_revision(transaction_current.activation_revision, expected_revision)?;
        if expected_native_recipe_signature.is_some() {
            let effective = resolve_effective_visual_source(&transaction, collection_id, icon_id)?;
            ensure_native_recipe_signature(
                &transaction,
                collection_id,
                icon_id,
                &effective,
                expected_native_recipe_signature,
            )?;
        }
        let transaction_recipe = load_render_recipe(
            &transaction,
            collection_id,
            icon_id,
            &target_source,
            expected_revision,
            &transaction_current.original_lineage_id,
            transaction_current.original_lineage_generation,
        )?;
        if transaction_recipe.signature != recipe.signature {
            return Err(AppError::new(
                "ai_activation_conflict",
                "적용 준비 중 편집값이 변경되었습니다. 후보를 다시 확인해 주세요.",
            ));
        }

        if let Some((version_id, candidate)) = new_version.as_ref() {
            let normalization = new_normalization.ok_or_else(|| {
                AppError::new(
                    "ai_normalization_state",
                    "AI 후보 버전의 크기 맞춤 정보를 찾을 수 없습니다.",
                )
            })?;
            let committed_source = normalization.commit_effective_source(&transaction, paths)?;
            if committed_source.id != target_source.id
                || committed_source.sha256 != target_source.sha256
            {
                return Err(AppError::new(
                    "ai_normalization_source_conflict",
                    "AI 후보 정규화 소스가 적용 전에 변경되었습니다.",
                ));
            }
            transaction.execute(
                "INSERT INTO icon_ai_versions (
                   id, icon_id, candidate_id, base_original_source_file_id,
                   base_original_lineage_id, base_original_lineage_generation,
                   parent_version_id, effective_source_file_id, input_stage,
                   apply_kind, provider_native_width, provider_native_height,
                   target_canvas_width, target_canvas_height,
                   normalization_recipe_json, normalization_recipe_hash,
                   canvas_kind, animation_kind, payload_input_signature, created_at
                 ) VALUES (
                   ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'base_source',
                   'active_source', ?9, ?10, ?11, ?12, ?13, ?14,
                   'source', ?15, ?16,
                   strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 )",
                params![
                    version_id,
                    icon_id,
                    candidate.candidate_id,
                    transaction_current.original_source.id,
                    transaction_current.original_lineage_id,
                    transaction_current.original_lineage_generation,
                    transaction_current.active_version_id,
                    target_source.id,
                    candidate.source.width,
                    candidate.source.height,
                    target_source.width,
                    target_source.height,
                    normalization.normalization_recipe_json,
                    normalization.normalization_recipe_hash,
                    if target_source.is_animated {
                        "animated"
                    } else {
                        "static"
                    },
                    candidate.payload_input_signature,
                ],
            )?;
        }

        let promoted_dir = ai_managed_artifacts::promote_owned_directory(
            &paths.root,
            &paths.ai_activation_staging_dir,
            &paths.ai_activation_previews_dir,
            &staging_dir,
            &final_dir,
        )?;
        let final_preview = rebase_artifact_path(
            &generated.current_preview_path,
            &prepared_staging_dir,
            &promoted_dir,
        )?;
        let final_pieces = generated
            .piece_paths
            .iter()
            .map(|path| rebase_artifact_path(path, &prepared_staging_dir, &promoted_dir))
            .collect::<AppResult<Vec<_>>>()?;
        let target_version_id = new_version
            .as_ref()
            .map(|(version_id, _)| version_id.as_str())
            .or(existing_version_id);

        let state_rows = transaction.execute(
            "UPDATE icon_ai_state
             SET active_version_id = ?1,
                 revision = revision + 1,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE icon_id = ?2
               AND revision = ?3",
            params![target_version_id, icon_id, expected_revision],
        )?;
        if state_rows != 1 {
            return Err(AppError::new(
                "ai_revision_conflict",
                "AI 적용 상태가 동시에 변경되었습니다.",
            ));
        }
        let icon_rows = transaction.execute(
            "UPDATE icons
             SET current_preview_path = ?1,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?2
               AND collection_id = ?3
               AND deleted_at IS NULL",
            params![path_string(&final_preview), icon_id, collection_id],
        )?;
        if icon_rows != 1 {
            return Err(AppError::not_found(
                "AI 후보를 적용할 아이콘을 찾을 수 없습니다.",
            ));
        }
        for (piece_id, piece_path) in recipe.piece_ids.iter().zip(final_pieces.iter()) {
            let piece_rows = transaction.execute(
                "UPDATE icon_pieces
                 SET generated_preview_path = ?1,
                     last_export_path = NULL,
                     export_status = 'ready',
                     updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE id = ?2
                   AND icon_id = ?3",
                params![path_string(piece_path), piece_id, icon_id],
            )?;
            if piece_rows != 1 {
                return Err(ai_repair_required());
            }
        }
        transaction.execute(
            "UPDATE processed_asset_variants
             SET is_active_for_export = 0
             WHERE icon_id = ?1
               AND is_active_for_export = 1",
            [icon_id],
        )?;
        let review_state = get_ai_review_state(&transaction, collection_id, icon_id)?;
        let editor_state =
            editor_repository::get_icon_editor_state(&transaction, collection_id, icon_id)?;
        let result = AiSourceMutationResultDto {
            review_state,
            editor_state,
        };
        transaction.commit()?;
        Ok(result)
    })();

    if commit_result.is_err() {
        let _ = ai_managed_artifacts::remove_owned_directory_if_present(
            &paths.root,
            &paths.ai_activation_staging_dir,
            &staging_dir,
        );
        let _ = ai_managed_artifacts::remove_owned_directory_if_present(
            &paths.root,
            &paths.ai_activation_previews_dir,
            &final_dir,
        );
        if let Some(snapshot) = source_artifact_snapshot.as_ref() {
            let _ = snapshot.cleanup_if_unreferenced(connection);
        }
    }
    commit_result
}

fn render_preview(
    staging_dir: &Path,
    collection_id: &str,
    icon_id: &str,
    source: &VisualSourceRecord,
    recipe: &RenderRecipe,
) -> AppResult<GeneratedPreview> {
    generate_icon_preview_in_directory(
        staging_dir,
        GeneratePreviewRequest {
            collection_id,
            icon_id,
            source_path: Path::new(&source.path),
            source_extension: &source.extension,
            shape: &recipe.shape,
            crop: recipe.crop,
            cell_width: recipe.cell_width,
            cell_height: recipe.cell_height,
            transform: recipe.transform,
            gif_loop_mode: &recipe.gif_loop_mode,
            gif_loop_count: recipe.gif_loop_count,
            source_gif_loop_mode: Some(&source.original_loop_mode),
            source_gif_loop_count: source.original_loop_count,
            text_overlay: recipe.text_overlay.clone(),
            effects: recipe.effects.clone(),
            motion: recipe.motion.clone(),
        },
    )
}

fn validate_outputs(generated: &GeneratedPreview, recipe: &RenderRecipe) -> AppResult<()> {
    validate_preview_structure(generated, recipe)?;
    if generated.piece_paths.len() != recipe.piece_ids.len() {
        return Err(AppError::new(
            "ai_activation_preview",
            "AI 적용 미리보기 조각 수가 현재 아이콘과 일치하지 않습니다.",
        ));
    }
    for path in &generated.piece_paths {
        if fs::metadata(path)?.len() > u64::try_from(recipe.max_bytes.max(1)).unwrap_or(u64::MAX) {
            return Err(AppError::new(
                "validation",
                "AI 적용 미리보기가 모음의 파일 용량 제한을 초과했습니다.",
            ));
        }
    }
    Ok(())
}

fn validate_preview_structure(
    generated: &GeneratedPreview,
    recipe: &RenderRecipe,
) -> AppResult<()> {
    if generated.piece_paths.len() != recipe.piece_ids.len() {
        return Err(AppError::new(
            "ai_activation_preview",
            "생성된 미리보기 조각 수가 현재 아이콘과 일치하지 않습니다.",
        ));
    }
    for path in std::iter::once(&generated.current_preview_path).chain(generated.piece_paths.iter())
    {
        if !path.is_file() {
            return Err(AppError::new(
                "ai_activation_preview",
                "생성된 미리보기 파일을 확인할 수 없습니다.",
            ));
        }
    }
    Ok(())
}

fn load_render_recipe(
    connection: &Connection,
    collection_id: &str,
    icon_id: &str,
    source: &VisualSourceRecord,
    activation_revision: i64,
    lineage_id: &str,
    lineage_generation: i64,
) -> AppResult<RenderRecipe> {
    let mut recipe = connection
        .query_row(
            "SELECT
               i.shape,
               COALESCE(i.cell_width_override, c.default_cell_width) AS cell_width,
               COALESCE(i.cell_height_override, c.default_cell_height) AS cell_height,
               i.transform_quarter_turns,
               i.transform_flip_horizontal,
               i.transform_flip_vertical,
               CASE WHEN i.gif_pingpong = 1 THEN 'pingpong' ELSE i.gif_loop_mode END AS gif_loop_mode,
               i.gif_loop_count,
               i.text_overlay_enabled,
               i.text_overlay_text,
               i.text_overlay_font_path,
               i.text_overlay_font_size,
               i.text_overlay_x,
               i.text_overlay_y,
               i.text_overlay_color,
               i.text_overlay_stroke_color,
               i.text_overlay_stroke_width,
               cs.crop_x,
               cs.crop_y,
               cs.crop_w,
               cs.crop_h,
               c.max_bytes
             FROM icons i
             JOIN collections c ON c.id = i.collection_id
             JOIN crop_settings cs ON cs.icon_id = i.id
             WHERE i.id = ?1
               AND i.collection_id = ?2
               AND i.deleted_at IS NULL
               AND c.deleted_at IS NULL",
            params![icon_id, collection_id],
            |row| {
                let transform = ImageTransform::new(
                    row.get("transform_quarter_turns")?,
                    row.get::<_, i64>("transform_flip_horizontal")? != 0,
                    row.get::<_, i64>("transform_flip_vertical")? != 0,
                )
                .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
                let text_overlay = text_overlay_from_fields(
                    row.get::<_, i64>("text_overlay_enabled")? != 0,
                    Some(row.get("text_overlay_text")?),
                    row.get("text_overlay_font_path")?,
                    Some(row.get("text_overlay_font_size")?),
                    Some(row.get("text_overlay_x")?),
                    Some(row.get("text_overlay_y")?),
                    Some(row.get("text_overlay_color")?),
                    Some(row.get("text_overlay_stroke_color")?),
                    Some(row.get("text_overlay_stroke_width")?),
                )
                .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
                Ok(RenderRecipe {
                    shape: row.get("shape")?,
                    crop: CropRect {
                        x: row.get("crop_x")?,
                        y: row.get("crop_y")?,
                        width: row.get("crop_w")?,
                        height: row.get("crop_h")?,
                    },
                    cell_width: row.get("cell_width")?,
                    cell_height: row.get("cell_height")?,
                    transform,
                    gif_loop_mode: row.get("gif_loop_mode")?,
                    gif_loop_count: row.get("gif_loop_count")?,
                    text_overlay,
                    effects: EffectRecipe::default(),
                    motion: MotionRecipe::default(),
                    piece_ids: Vec::new(),
                    max_bytes: row.get("max_bytes")?,
                    signature: String::new(),
                })
            },
        )
        .optional()?
        .ok_or_else(|| {
            AppError::not_found("AI 후보를 적용할 편집 설정을 찾을 수 없습니다.")
        })?;
    recipe.effects =
        effect_repository::effect_recipe_for_icon(connection, collection_id, icon_id)?.recipe;
    recipe.motion =
        motion_repository::motion_recipe_for_icon(connection, collection_id, icon_id)?.recipe;
    recipe.piece_ids = ordered_piece_ids(connection, icon_id)?;
    if recipe.piece_ids.len() != piece_roles(&recipe.shape)?.len() {
        return Err(ai_repair_required());
    }
    recipe.signature = recipe_signature(
        &recipe,
        source,
        activation_revision,
        lineage_id,
        lineage_generation,
    )?;
    Ok(recipe)
}

fn recipe_signature(
    recipe: &RenderRecipe,
    source: &VisualSourceRecord,
    activation_revision: i64,
    lineage_id: &str,
    lineage_generation: i64,
) -> AppResult<String> {
    let crop = ExportCropRect {
        x: recipe.crop.x,
        y: recipe.crop.y,
        width: recipe.crop.width,
        height: recipe.crop.height,
    };
    let mut parts = vec![
        "pmtcon-ai-activation-recipe-v1".to_string(),
        source.id.clone(),
        source.sha256.clone(),
        source.width.to_string(),
        source.height.to_string(),
        source.original_loop_mode.clone(),
        source.original_loop_count.unwrap_or_default().to_string(),
        activation_revision.to_string(),
        lineage_id.to_string(),
        lineage_generation.to_string(),
        recipe.max_bytes.to_string(),
    ];
    for (index, piece_id) in recipe.piece_ids.iter().enumerate() {
        parts.push(piece_id.clone());
        parts.push(render_recipe_crop_hash(
            &recipe.shape,
            &crop,
            recipe.cell_width,
            recipe.cell_height,
            index,
            recipe.transform,
            &recipe.gif_loop_mode,
            recipe.gif_loop_count,
            recipe.text_overlay.as_ref(),
            &recipe.effects,
            &recipe.motion,
        )?);
    }
    Ok(hash_text(&parts))
}

fn ordered_piece_ids(connection: &Connection, icon_id: &str) -> AppResult<Vec<String>> {
    let mut statement = connection
        .prepare("SELECT id FROM icon_pieces WHERE icon_id = ?1 ORDER BY piece_index ASC")?;
    let piece_ids = statement
        .query_map([icon_id], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(piece_ids)
}

pub(crate) fn candidate_stale_reason(
    connection: &Connection,
    collection_id: &str,
    icon_id: &str,
    candidate_id: &str,
    current: &crate::db::repositories::ai::EffectiveVisualSource,
) -> AppResult<Option<String>> {
    let request = connection.query_row(
        "SELECT
           request.request_recipe_signature,
           request.activation_revision,
           request.original_lineage_id,
           request.original_lineage_generation,
           request.original_source_sha256,
           request.effective_source_sha256,
           request.status,
           request.superseded_at
         FROM ai_candidates candidate
         JOIN ai_requests request ON request.id = candidate.request_id
         WHERE candidate.id = ?1
           AND request.origin_icon_id = ?2
           AND request.origin_collection_id = ?3",
        params![candidate_id, icon_id, collection_id],
        |row| {
            Ok((
                row.get::<_, String>("request_recipe_signature")?,
                row.get::<_, i64>("activation_revision")?,
                row.get::<_, String>("original_lineage_id")?,
                row.get::<_, i64>("original_lineage_generation")?,
                row.get::<_, String>("original_source_sha256")?,
                row.get::<_, String>("effective_source_sha256")?,
                row.get::<_, String>("status")?,
                row.get::<_, Option<String>>("superseded_at")?,
            ))
        },
    )?;
    let current_recipe = current_recipe_signature(
        connection,
        collection_id,
        icon_id,
        &current.render_source,
        current.activation_revision,
    )?;
    let valid = request.0 == current_recipe
        && request.1 == current.activation_revision
        && request.2 == current.original_lineage_id
        && request.3 == current.original_lineage_generation
        && request.4 == current.original_source.sha256
        && request.5 == current.render_source.sha256
        && request.6 == "completed"
        && request.7.is_none();
    if valid {
        Ok(None)
    } else {
        Ok(Some(
            "후보를 만든 뒤 원본 또는 편집 상태가 바뀌었습니다. 새 후보를 가져와 주세요."
                .to_string(),
        ))
    }
}

fn load_version_source(
    connection: &Connection,
    icon_id: &str,
    version_id: &str,
    lineage_id: &str,
    lineage_generation: i64,
) -> AppResult<VisualSourceRecord> {
    let source_id = connection
        .query_row(
            "SELECT effective_source_file_id
             FROM icon_ai_versions
             WHERE id = ?1
               AND icon_id = ?2
               AND base_original_lineage_id = ?3
               AND base_original_lineage_generation = ?4",
            params![version_id, icon_id, lineage_id, lineage_generation],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .ok_or_else(|| {
            AppError::new(
                "ai_version_lineage",
                "현재 원본 계보에서 복원할 수 없는 AI 버전입니다.",
            )
        })?;
    load_and_validate_source(connection, &source_id)
}

fn ensure_native_recipe_signature(
    connection: &Connection,
    collection_id: &str,
    icon_id: &str,
    current: &crate::db::repositories::ai::EffectiveVisualSource,
    expected: Option<&str>,
) -> AppResult<()> {
    let Some(expected) = expected else {
        return Ok(());
    };
    let actual = current_recipe_signature(
        connection,
        collection_id,
        icon_id,
        &current.render_source,
        current.activation_revision,
    )?;
    if actual == expected {
        Ok(())
    } else {
        Err(AppError::new(
            "ai_normalization_preview_stale",
            "편집값이 변경되었습니다. 현재 설정으로 규격화 미리보기를 다시 확인해 주세요.",
        ))
    }
}
fn ensure_expected_revision(actual: i64, expected: i64) -> AppResult<()> {
    if actual == expected {
        Ok(())
    } else {
        Err(AppError::new(
            "ai_revision_conflict",
            "다른 작업에서 AI 적용 상태가 변경되었습니다. 이력을 새로 불러와 주세요.",
        ))
    }
}

fn load_activation_state(
    connection: &Connection,
    collection_id: &str,
    icon_id: &str,
) -> AppResult<ActivationState> {
    let projection = connection
        .query_row(
            "SELECT
               i.source_file_id AS original_source_file_id,
               i.original_lineage_id,
               i.original_lineage_generation,
               st.active_version_id,
               st.revision AS activation_revision
             FROM icons i
             JOIN collections c ON c.id = i.collection_id
             JOIN icon_ai_state st ON st.icon_id = i.id
             WHERE i.id = ?1
               AND i.collection_id = ?2
               AND i.deleted_at IS NULL
               AND c.deleted_at IS NULL",
            params![icon_id, collection_id],
            |row| {
                Ok((
                    row.get::<_, String>("original_source_file_id")?,
                    row.get::<_, String>("original_lineage_id")?,
                    row.get::<_, i64>("original_lineage_generation")?,
                    row.get::<_, Option<String>>("active_version_id")?,
                    row.get::<_, i64>("activation_revision")?,
                ))
            },
        )
        .optional()?
        .ok_or_else(ai_repair_required)?;
    Ok(ActivationState {
        original_source: load_and_validate_source(connection, &projection.0)?,
        original_lineage_id: projection.1,
        original_lineage_generation: projection.2,
        active_version_id: projection.3,
        activation_revision: projection.4,
    })
}

fn rebase_artifact_path(path: &Path, from: &Path, to: &Path) -> AppResult<PathBuf> {
    let relative = path.strip_prefix(from).map_err(|_| {
        AppError::new(
            "ai_activation_path",
            "AI 적용 미리보기 경로가 staging 바깥을 가리킵니다.",
        )
    })?;
    Ok(to.join(relative))
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}
