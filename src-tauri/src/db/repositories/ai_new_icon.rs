use std::fs;
use std::path::{Component, Path, PathBuf};

use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};

use crate::db::repositories::ai::{
    candidate_created_icon_usage, get_ai_review_state, load_and_validate_source,
    resolve_effective_visual_source, VisualSourceRecord,
};
use crate::db::repositories::ai_activation;
use crate::db::repositories::ai_candidate_normalization;
use crate::db::repositories::clone_artifacts::clone_current_ai_lineage;
use crate::db::repositories::icons;
use crate::error::{AppError, AppResult};
use crate::ids::create_id;
use crate::models::{CreateAiIconRootPayload, CreateAiIconRootResultDto};
use crate::paths::AppPaths;

pub(crate) fn create_ai_icon_root(
    connection: &mut Connection,
    paths: &AppPaths,
    collection_id: &str,
    payload: &CreateAiIconRootPayload,
) -> AppResult<CreateAiIconRootResultDto> {
    let target_icon_id = create_id("icon");
    let operation_id = create_id("ai_new_icon");
    validate_path_component("모음 ID", collection_id)?;
    validate_path_component("아이콘 ID", &target_icon_id)?;
    let staging_dir = paths.ai_activation_staging_dir.join(&operation_id);
    let final_dir = paths
        .ai_activation_previews_dir
        .join(collection_id)
        .join(&target_icon_id)
        .join(&operation_id);

    let mut source_artifact_snapshot = None;
    let create_result = (|| -> AppResult<CreateAiIconRootResultDto> {
        ensure_path_absent(&staging_dir)?;
        let prepared_staging_dir =
            prepare_owned_directory(&paths.root, &paths.ai_activation_staging_dir, &staging_dir)?;
        ensure_path_absent(&final_dir)?;

        let normalization = ai_candidate_normalization::prepare_candidate_normalization(
            connection,
            paths,
            collection_id,
            &payload.icon_id,
            &payload.candidate_id,
            payload.expected_revision,
            &payload.normalization,
            &prepared_staging_dir,
        )?;
        ai_candidate_normalization::ensure_preview_signature(
            payload.expected_preview_signature.as_deref(),
            &normalization.preview_signature,
        )?;
        ai_candidate_normalization::ensure_new_icon_compatible(
            &normalization.new_icon_compatibility,
        )?;

        let prepared_current =
            resolve_effective_visual_source(connection, collection_id, &payload.icon_id)?;
        ensure_expected_revision(
            prepared_current.activation_revision,
            payload.expected_revision,
        )?;
        let prepared_render = ai_activation::prepare_source_preview(
            connection,
            collection_id,
            &payload.icon_id,
            &target_icon_id,
            &normalization.effective_source,
            &prepared_current.render_source,
            prepared_current.activation_revision,
            &prepared_current.original_lineage_id,
            prepared_current.original_lineage_generation,
            &normalization.native_recipe_signature,
        )?;
        validate_clone_history_sources(connection, &payload.icon_id)?;
        let generated =
            ai_activation::render_prepared_source_preview(&prepared_staging_dir, &prepared_render)?;
        validate_piece_sizes(connection, collection_id, &generated.piece_paths)?;

        if normalization
            .normalized_preview_path
            .starts_with(&prepared_staging_dir)
        {
            let _ = fs::remove_file(&normalization.normalized_preview_path);
        }
        let final_preview = promote_preview_directory(
            &paths.root,
            &paths.ai_activation_staging_dir,
            &paths.ai_activation_previews_dir,
            &staging_dir,
            &prepared_staging_dir,
            &final_dir,
            &generated.current_preview_path,
        )?;
        let final_directory = final_preview.parent().ok_or_else(|| {
            AppError::new(
                "ai_new_icon_path",
                "새 AI 아이콘 미리보기 파일의 상위 경로를 확인할 수 없습니다.",
            )
        })?;
        let final_piece_paths = generated
            .piece_paths
            .iter()
            .map(|path| rebase_artifact_path(path, &prepared_staging_dir, final_directory))
            .collect::<AppResult<Vec<_>>>()?;
        source_artifact_snapshot = normalization.source_artifact_snapshot(connection, paths)?;

        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current =
            resolve_effective_visual_source(&transaction, collection_id, &payload.icon_id)?;
        ensure_expected_revision(current.activation_revision, payload.expected_revision)?;
        let transaction_native_recipe = ai_activation::current_recipe_signature(
            &transaction,
            collection_id,
            &payload.icon_id,
            &current.render_source,
            current.activation_revision,
        )?;
        if transaction_native_recipe != normalization.native_recipe_signature {
            return Err(AppError::new(
                "ai_new_icon_conflict",
                "새 아이콘을 준비하는 동안 편집값이 변경되었습니다. 규격화 미리보기를 다시 확인해 주세요.",
            ));
        }
        let effective_source = normalization.commit_effective_source(&transaction, paths)?;
        let piece_ids = insert_working_icon_clone(
            &transaction,
            collection_id,
            &payload.icon_id,
            &target_icon_id,
        )?;
        if generated.piece_paths.len() != piece_ids.len() {
            return Err(AppError::new(
                "ai_new_icon_preview",
                "새 AI 아이콘의 미리보기 조각 수가 원본 아이콘과 일치하지 않습니다.",
            ));
        }
        clone_current_ai_lineage(&transaction, &payload.icon_id, &target_icon_id)?;
        insert_candidate_child_version(
            &transaction,
            &target_icon_id,
            &normalization,
            &effective_source,
        )?;
        insert_ai_icon_root_creation(
            &transaction,
            &payload.icon_id,
            &target_icon_id,
            &normalization,
        )?;
        let icon_rows = transaction.execute(
            "UPDATE icons
             SET current_preview_path = ?1,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?2
               AND collection_id = ?3
               AND deleted_at IS NULL",
            params![path_string(&final_preview), target_icon_id, collection_id],
        )?;
        if icon_rows != 1 {
            return Err(AppError::new(
                "ai_new_icon_state",
                "새 AI 아이콘의 미리보기 경로를 저장할 수 없습니다.",
            ));
        }
        for (piece_id, piece_path) in piece_ids.iter().zip(final_piece_paths.iter()) {
            let piece_rows = transaction.execute(
                "UPDATE icon_pieces
                 SET generated_preview_path = ?1,
                     last_export_path = NULL,
                     export_status = 'ready',
                     updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE id = ?2
                   AND icon_id = ?3",
                params![path_string(piece_path), piece_id, target_icon_id],
            )?;
            if piece_rows != 1 {
                return Err(AppError::new(
                    "ai_new_icon_state",
                    "새 AI 아이콘의 조각 미리보기 경로를 저장할 수 없습니다.",
                ));
            }
        }

        let created_icon = icons::get_icon(&transaction, collection_id, &target_icon_id)?;
        let source_review_state =
            get_ai_review_state(&transaction, collection_id, &payload.icon_id)?;
        let created_icon_usage =
            candidate_created_icon_usage(&transaction, collection_id, &payload.candidate_id)?;
        let result = CreateAiIconRootResultDto {
            created_icon,
            source_review_state,
            created_icon_usage,
        };
        transaction.commit()?;
        Ok(result)
    })();

    if create_result.is_err() {
        let _ = remove_owned_directory(&paths.root, &paths.ai_activation_staging_dir, &staging_dir);
        let _ = remove_owned_directory(&paths.root, &paths.ai_activation_previews_dir, &final_dir);
        if let Some(parent) = final_dir.parent() {
            let _ = remove_empty_owned_ancestors(
                &paths.root,
                &paths.ai_activation_previews_dir,
                parent,
            );
        }
        if let Some(snapshot) = source_artifact_snapshot.as_ref() {
            let _ = snapshot.cleanup_if_unreferenced(connection);
        }
    }
    create_result
}

fn insert_ai_icon_root_creation(
    transaction: &Transaction<'_>,
    source_icon_id: &str,
    target_icon_id: &str,
    normalization: &ai_candidate_normalization::PreparedCandidateNormalization,
) -> AppResult<()> {
    let inserted = transaction.execute(
        "INSERT INTO ai_icon_root_creations (
           icon_id, source_icon_id, candidate_id, normalization_recipe_hash, created_at
         ) VALUES (?1, ?2, ?3, ?4, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
        params![
            target_icon_id,
            source_icon_id,
            normalization.candidate_id.as_str(),
            normalization.normalization_recipe_hash.as_str(),
        ],
    )?;
    if inserted != 1 {
        return Err(AppError::new(
            "ai_new_icon_provenance",
            "새 AI 아이콘의 생성 이력을 기록할 수 없습니다.",
        ));
    }
    Ok(())
}
fn validate_clone_history_sources(connection: &Connection, source_icon_id: &str) -> AppResult<()> {
    let source_ids = {
        let mut statement = connection.prepare(
            "SELECT original_source_file_id
             FROM icon_ai_lineages
             WHERE icon_id = ?1
             UNION
             SELECT effective_source_file_id
             FROM icon_ai_versions
             WHERE icon_id = ?1",
        )?;
        let rows = statement
            .query_map([source_icon_id], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        rows
    };
    for source_id in source_ids {
        load_and_validate_source(connection, &source_id)?;
    }
    Ok(())
}

fn insert_working_icon_clone(
    transaction: &Transaction<'_>,
    collection_id: &str,
    source_icon_id: &str,
    target_icon_id: &str,
) -> AppResult<Vec<String>> {
    let (source_name, source_order_index) = transaction
        .query_row(
            "SELECT display_name, order_index
             FROM icons
             WHERE id = ?1
               AND collection_id = ?2
               AND deleted_at IS NULL",
            params![source_icon_id, collection_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("복제할 원본 아이콘을 찾을 수 없습니다."))?;
    let order_index = source_order_index.checked_add(1).ok_or_else(|| {
        AppError::new(
            "ai_new_icon_order",
            "새 AI 아이콘의 정렬 위치를 계산할 수 없습니다.",
        )
    })?;
    transaction.execute(
        "UPDATE icons
         SET order_index = order_index + 1
         WHERE collection_id = ?1
           AND deleted_at IS NULL
           AND order_index >= ?2",
        params![collection_id, order_index],
    )?;
    let display_name = if source_name.trim().is_empty() {
        "AI 아이콘".to_string()
    } else {
        format!("{} AI", source_name.trim())
    };
    let inserted = transaction.execute(
        "INSERT INTO icons (
           id, collection_id, source_file_id, display_name,
           icon_kind, readiness, placeholder_text, shape, order_index,
           cell_width_override, cell_height_override,
           thumbnail_path, thumbnail_override_source_file_id,
           thumbnail_override_path, current_preview_path,
           text_overlay_enabled, text_overlay_text, text_overlay_font_path,
           text_overlay_font_size, text_overlay_x, text_overlay_y,
           text_overlay_color, text_overlay_stroke_color,
           text_overlay_stroke_width, transform_quarter_turns,
           transform_flip_horizontal, transform_flip_vertical,
           gif_loop_mode, gif_loop_count, gif_pingpong, created_at, updated_at
         )
         SELECT
           ?1, collection_id, source_file_id, ?2,
           'image', 'working', NULL, shape, ?3,
           cell_width_override, cell_height_override,
           thumbnail_path, NULL, NULL, NULL,
           text_overlay_enabled, text_overlay_text, text_overlay_font_path,
           text_overlay_font_size, text_overlay_x, text_overlay_y,
           text_overlay_color, text_overlay_stroke_color,
           text_overlay_stroke_width, transform_quarter_turns,
           transform_flip_horizontal, transform_flip_vertical,
           gif_loop_mode, gif_loop_count, gif_pingpong,
           strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
           strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         FROM icons
         WHERE id = ?4
           AND collection_id = ?5
           AND deleted_at IS NULL",
        params![
            target_icon_id,
            display_name,
            order_index,
            source_icon_id,
            collection_id
        ],
    )?;
    if inserted != 1 {
        return Err(AppError::not_found(
            "새 AI 아이콘으로 복제할 원본 아이콘을 찾을 수 없습니다.",
        ));
    }

    let crop_rows = transaction.execute(
        "INSERT INTO crop_settings (
           id, icon_id, crop_mode, crop_x, crop_y, crop_w, crop_h,
           preset_position, source_width_at_apply, source_height_at_apply,
           viewport_width_at_apply, viewport_height_at_apply, updated_at
         )
         SELECT
           ?1, ?2, crop_mode, crop_x, crop_y, crop_w, crop_h,
           preset_position, source_width_at_apply, source_height_at_apply,
           viewport_width_at_apply, viewport_height_at_apply,
           strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         FROM crop_settings
         WHERE icon_id = ?3",
        params![create_id("crop"), target_icon_id, source_icon_id],
    )?;
    if crop_rows != 1 {
        return Err(AppError::new(
            "ai_new_icon_crop",
            "원본 아이콘의 크롭 설정을 복제할 수 없습니다.",
        ));
    }

    let source_pieces = {
        let mut statement = transaction.prepare(
            "SELECT piece_index, piece_role
             FROM icon_pieces
             WHERE icon_id = ?1
             ORDER BY piece_index ASC",
        )?;
        let rows = statement
            .query_map([source_icon_id], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows
    };
    if source_pieces.is_empty() {
        return Err(AppError::new(
            "ai_new_icon_piece",
            "원본 아이콘의 조각 설정을 찾을 수 없습니다.",
        ));
    }
    let mut piece_ids = Vec::with_capacity(source_pieces.len());
    for (piece_index, piece_role) in source_pieces {
        let piece_id = create_id("piece");
        transaction.execute(
            "INSERT INTO icon_pieces (
               id, icon_id, piece_index, piece_role, alt_text,
               generated_preview_path, last_export_path, export_status,
               created_at, updated_at
             )
             VALUES (
               ?1, ?2, ?3, ?4, '', NULL, NULL, 'not_exported',
               strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
               strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             )",
            params![piece_id, target_icon_id, piece_index, piece_role],
        )?;
        piece_ids.push(piece_id);
    }

    transaction.execute(
        "INSERT INTO icon_effect_recipes (
           icon_id, recipe_schema, revision, effects_json, created_at, updated_at
         )
         SELECT ?2, recipe_schema, revision, effects_json,
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         FROM icon_effect_recipes
         WHERE icon_id = ?1",
        params![source_icon_id, target_icon_id],
    )?;
    transaction.execute(
        "INSERT INTO icon_motion_recipes (
           icon_id, recipe_schema, revision, motion_json, created_at, updated_at
         )
         SELECT ?2, recipe_schema, revision, motion_json,
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         FROM icon_motion_recipes
         WHERE icon_id = ?1",
        params![source_icon_id, target_icon_id],
    )?;
    Ok(piece_ids)
}

fn insert_candidate_child_version(
    transaction: &Transaction<'_>,
    target_icon_id: &str,
    normalization: &ai_candidate_normalization::PreparedCandidateNormalization,
    effective_source: &VisualSourceRecord,
) -> AppResult<()> {
    let (
        base_source_file_id,
        base_lineage_id,
        base_lineage_generation,
        parent_version_id,
        current_revision,
    ) = transaction
        .query_row(
            "SELECT icon.source_file_id, icon.original_lineage_id,
                        icon.original_lineage_generation, state.active_version_id, state.revision
                 FROM icons icon
                 JOIN icon_ai_state state ON state.icon_id = icon.id
                 WHERE icon.id = ?1",
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
        .ok_or_else(|| {
            AppError::new(
                "ai_new_icon_state",
                "새 AI 아이콘의 원본 계보 상태를 찾을 수 없습니다.",
            )
        })?;
    let next_revision = current_revision.checked_add(1).ok_or_else(|| {
        AppError::new(
            "ai_revision_overflow",
            "새 AI 아이콘의 적용 이력 번호를 증가시킬 수 없습니다.",
        )
    })?;
    let version_id = create_id("ai_version");
    transaction.execute(
        "INSERT INTO icon_ai_versions (
           id, icon_id, candidate_id, base_original_source_file_id,
           base_original_lineage_id, base_original_lineage_generation,
           parent_version_id, effective_source_file_id, input_stage, apply_kind,
           provider_native_width, provider_native_height,
           target_canvas_width, target_canvas_height,
           normalization_recipe_json, normalization_recipe_hash,
           canvas_kind, animation_kind, payload_input_signature, created_at
         )
         VALUES (
           ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'base_source', 'new_icon_root',
           ?9, ?10, ?11, ?12, ?13, ?14, 'source', ?15, ?16,
           strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         )",
        params![
            version_id,
            target_icon_id,
            normalization.candidate_id,
            base_source_file_id,
            base_lineage_id,
            base_lineage_generation,
            parent_version_id,
            effective_source.id,
            normalization.raw_source.width,
            normalization.raw_source.height,
            effective_source.width,
            effective_source.height,
            normalization.normalization_recipe_json,
            normalization.normalization_recipe_hash,
            if effective_source.is_animated {
                "animated"
            } else {
                "static"
            },
            normalization.payload_input_signature,
        ],
    )?;
    let updated = transaction.execute(
        "UPDATE icon_ai_state
         SET active_version_id = ?1,
             revision = ?2,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE icon_id = ?3
           AND active_version_id IS ?4
           AND revision = ?5",
        params![
            version_id,
            next_revision,
            target_icon_id,
            parent_version_id,
            current_revision
        ],
    )?;
    if updated != 1 {
        return Err(AppError::new(
            "ai_revision_conflict",
            "새 AI 아이콘의 적용 상태가 동시에 변경되었습니다.",
        ));
    }
    Ok(())
}

fn validate_piece_sizes(
    connection: &Connection,
    collection_id: &str,
    piece_paths: &[PathBuf],
) -> AppResult<()> {
    let max_bytes = connection
        .query_row(
            "SELECT max_bytes FROM collections WHERE id = ?1 AND deleted_at IS NULL",
            [collection_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("새 아이콘을 추가할 모음을 찾을 수 없습니다."))?;
    let max_bytes = u64::try_from(max_bytes.max(1)).unwrap_or(u64::MAX);
    for path in piece_paths {
        if fs::metadata(path)?.len() > max_bytes {
            return Err(AppError::new(
                "validation",
                "새 AI 아이콘의 미리보기 조각이 모음의 파일 용량 제한을 초과했습니다.",
            ));
        }
    }
    Ok(())
}

fn promote_preview_directory(
    app_root: &Path,
    staging_root: &Path,
    previews_root: &Path,
    owned_staging_dir: &Path,
    rendered_staging_dir: &Path,
    final_dir: &Path,
    staging_preview: &Path,
) -> AppResult<PathBuf> {
    let canonical_staging =
        canonical_owned_directory(app_root, staging_root, owned_staging_dir, false)?.ok_or_else(
            || {
                AppError::new(
                    "ai_new_icon_path",
                    "새 AI 아이콘의 staging 경로를 찾을 수 없습니다.",
                )
            },
        )?;
    let final_parent = final_dir.parent().ok_or_else(|| {
        AppError::new(
            "ai_new_icon_path",
            "새 AI 아이콘 미리보기의 상위 경로를 확인할 수 없습니다.",
        )
    })?;
    let canonical_final_parent = prepare_owned_directory(app_root, previews_root, final_parent)?;
    let final_name = final_dir.file_name().ok_or_else(|| {
        AppError::new(
            "ai_new_icon_path",
            "새 AI 아이콘 미리보기 경로의 이름을 확인할 수 없습니다.",
        )
    })?;
    let canonical_final_dir = canonical_final_parent.join(final_name);
    ensure_path_absent(&canonical_final_dir)?;
    fs::rename(&canonical_staging, &canonical_final_dir)?;
    let promoted = canonical_owned_directory(app_root, previews_root, final_dir, false)?
        .ok_or_else(|| {
            AppError::new(
                "ai_new_icon_path",
                "승격된 새 AI 아이콘 미리보기 경로를 확인할 수 없습니다.",
            )
        })?;
    rebase_artifact_path(staging_preview, rendered_staging_dir, &promoted)
}

fn rebase_artifact_path(path: &Path, from: &Path, to: &Path) -> AppResult<PathBuf> {
    let relative = path.strip_prefix(from).map_err(|_| {
        AppError::new(
            "ai_new_icon_path",
            "새 AI 아이콘의 미리보기 경로가 staging 바깥을 가리킵니다.",
        )
    })?;
    Ok(to.join(relative))
}

fn ensure_path_absent(path: &Path) -> AppResult<()> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
        Ok(_) => Err(AppError::new(
            "ai_new_icon_path",
            "새 AI 아이콘의 미리보기 경로가 이미 존재합니다.",
        )),
    }
}

fn prepare_owned_directory(
    app_root: &Path,
    allowed_root: &Path,
    target: &Path,
) -> AppResult<PathBuf> {
    canonical_owned_directory(app_root, allowed_root, target, true)?.ok_or_else(|| {
        AppError::new(
            "ai_new_icon_path",
            "새 AI 아이콘의 관리 경로를 준비할 수 없습니다.",
        )
    })
}

fn canonical_owned_directory(
    app_root: &Path,
    allowed_root: &Path,
    target: &Path,
    create_missing: bool,
) -> AppResult<Option<PathBuf>> {
    let allowed_relative = allowed_root.strip_prefix(app_root).map_err(|_| {
        AppError::new(
            "ai_new_icon_path",
            "AI 미리보기 관리 루트가 앱 데이터 루트 밖에 있습니다.",
        )
    })?;
    validate_relative_components(allowed_relative)?;
    let target_relative_to_allowed = target.strip_prefix(allowed_root).map_err(|_| {
        AppError::new(
            "ai_new_icon_path",
            "새 AI 아이콘 경로가 허용된 관리 루트 밖에 있습니다.",
        )
    })?;
    validate_relative_components(target_relative_to_allowed)?;
    let target_relative = target.strip_prefix(app_root).map_err(|_| {
        AppError::new(
            "ai_new_icon_path",
            "새 AI 아이콘 경로가 앱 데이터 루트 밖에 있습니다.",
        )
    })?;
    validate_relative_components(target_relative)?;

    if !fs::metadata(app_root)?.is_dir() {
        return Err(AppError::new(
            "ai_new_icon_path",
            "앱 데이터 루트가 디렉터리가 아닙니다.",
        ));
    }
    let canonical_app_root = app_root.canonicalize()?;
    let canonical_allowed_root = canonical_app_root.join(allowed_relative);
    let mut current = app_root.to_path_buf();
    let mut expected = canonical_app_root;

    for component in target_relative.components() {
        let Component::Normal(component) = component else {
            return Err(AppError::new(
                "ai_new_icon_path",
                "새 AI 아이콘 경로에 안전하지 않은 구성 요소가 있습니다.",
            ));
        };
        current.push(component);
        expected.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) => validate_owned_component(&current, &expected, &metadata)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && create_missing => {
                fs::create_dir(&current)?;
                let metadata = fs::symlink_metadata(&current)?;
                validate_owned_component(&current, &expected, &metadata)?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        }
    }

    if !expected.starts_with(&canonical_allowed_root) {
        return Err(AppError::new(
            "ai_new_icon_path",
            "새 AI 아이콘의 정규 경로가 관리 루트 밖에 있습니다.",
        ));
    }
    Ok(Some(expected))
}

fn validate_relative_components(path: &Path) -> AppResult<()> {
    if path
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
    {
        Ok(())
    } else {
        Err(AppError::new(
            "ai_new_icon_path",
            "새 AI 아이콘 경로에 안전하지 않은 구성 요소가 있습니다.",
        ))
    }
}

fn validate_owned_component(
    path: &Path,
    expected: &Path,
    metadata: &fs::Metadata,
) -> AppResult<()> {
    if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
        return Err(AppError::new(
            "ai_new_icon_path",
            "새 AI 아이콘 경로에 안전하지 않은 링크 또는 파일이 포함되어 있습니다.",
        ));
    }
    let canonical = path.canonicalize()?;
    if canonical != expected {
        return Err(AppError::new(
            "ai_new_icon_path",
            "새 AI 아이콘 경로의 정규 위치가 예상된 관리 경로와 다릅니다.",
        ));
    }
    Ok(())
}

fn remove_owned_directory(app_root: &Path, root: &Path, target: &Path) -> AppResult<()> {
    let Some(target) = canonical_owned_directory(app_root, root, target, false)? else {
        return Ok(());
    };
    let root = canonical_owned_directory(app_root, root, root, false)?.ok_or_else(|| {
        AppError::new(
            "ai_new_icon_path",
            "AI 미리보기 관리 루트를 찾을 수 없습니다.",
        )
    })?;
    if target == root {
        return Err(AppError::new(
            "ai_new_icon_path",
            "AI 미리보기 관리 루트 자체는 정리할 수 없습니다.",
        ));
    }
    fs::remove_dir_all(target)?;
    Ok(())
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

fn remove_empty_owned_ancestors(app_root: &Path, root: &Path, start: &Path) -> AppResult<()> {
    let Some(mut current) = canonical_owned_directory(app_root, root, start, false)? else {
        return Ok(());
    };
    let root = canonical_owned_directory(app_root, root, root, false)?.ok_or_else(|| {
        AppError::new(
            "ai_new_icon_path",
            "AI 미리보기 관리 루트를 찾을 수 없습니다.",
        )
    })?;
    while current != root && current.starts_with(&root) {
        match fs::remove_dir(&current) {
            Ok(()) => {}
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::DirectoryNotEmpty
                ) =>
            {
                if error.kind() == std::io::ErrorKind::DirectoryNotEmpty {
                    break;
                }
            }
            Err(error) => return Err(error.into()),
        }
        let Some(parent) = current.parent() else {
            break;
        };
        current = parent.to_path_buf();
    }
    Ok(())
}
fn validate_path_component(label: &str, value: &str) -> AppResult<()> {
    let mut components = Path::new(value).components();
    let valid = matches!(components.next(), Some(Component::Normal(_)))
        && components.next().is_none()
        && value != "."
        && value != "..";
    if valid {
        Ok(())
    } else {
        Err(AppError::new(
            "ai_new_icon_path",
            format!("{label}가 안전한 단일 경로 구성요소가 아닙니다."),
        ))
    }
}
fn path_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}
