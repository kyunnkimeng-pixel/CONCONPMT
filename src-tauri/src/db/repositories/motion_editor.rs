use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

use crate::db::repositories::editor as editor_repository;
use crate::db::repositories::effects as effect_repository;
use crate::db::repositories::motion as motion_repository;
use crate::error::{AppError, AppResult};
use crate::ids::create_id;
use crate::imaging::effects::EffectRecipe;
use crate::imaging::gif_pipeline::{output_repeat_for_settings, GifOutputRepeat};
use crate::imaging::motion::{validate_motion_recipe, MotionRecipe};
use crate::imaging::preview::{
    generate_icon_preview_in_directory, CropRect, GeneratePreviewRequest, GeneratedPreview,
};
use crate::imaging::text_overlay::{text_overlay_from_fields, TextOverlayRenderSpec};
use crate::imaging::transform::ImageTransform;
use crate::models::{
    IconEditorStateDto, MotionPreviewDto, PreviewIconMotionPayload, UpdateIconMotionPayload,
};
use crate::optimization::cache::hash_text;
use crate::paths::AppPaths;

const MOTION_PREVIEW_DIRECTORY: &str = "motion-previews";
const IN_PROGRESS_MARKER: &str = ".in-progress";
const COMPLETE_MARKER: &str = ".complete";
const MAX_COMPLETED_PREVIEWS_PER_ICON: usize = 8;

#[derive(Debug, Clone)]
struct MotionRenderRecord {
    original_path_in_library: String,
    original_extension: String,
    source_hash: String,
    original_loop_mode: String,
    original_loop_count: Option<i64>,
    current_preview_path: Option<String>,
    shape: String,
    cell_width: i64,
    cell_height: i64,
    transform_quarter_turns: i64,
    transform_flip_horizontal: bool,
    transform_flip_vertical: bool,
    gif_loop_mode: String,
    gif_loop_count: Option<i64>,
    crop_x: f64,
    crop_y: f64,
    crop_w: f64,
    crop_h: f64,
    max_bytes: i64,
    text_overlay: Option<TextOverlayRenderSpec>,
}

#[derive(Debug)]
pub struct PreparedMotionRender {
    collection_id: String,
    icon_id: String,
    record: MotionRenderRecord,
    effects: EffectRecipe,
    recipe: MotionRecipe,
    expected_revision: Option<i64>,
    render_signature: String,
}

#[derive(Debug)]
pub struct RenderedMotionSave {
    prepared: PreparedMotionRender,
    artifact: OwnedMotionArtifact,
    generated: GeneratedPreview,
}

pub fn prepare_motion_preview(
    connection: &Connection,
    collection_id: &str,
    payload: PreviewIconMotionPayload,
) -> AppResult<PreparedMotionRender> {
    validate_motion_recipe(&payload.recipe)?;
    prepare_motion_render(
        connection,
        collection_id,
        payload.icon_id,
        payload.recipe,
        None,
        None,
    )
}

pub fn prepare_motion_update(
    connection: &Connection,
    collection_id: &str,
    payload: UpdateIconMotionPayload,
) -> AppResult<PreparedMotionRender> {
    validate_motion_recipe(&payload.recipe)?;
    if payload.expected_revision < 0 {
        return Err(AppError::new(
            "validation",
            "모션 recipe revision은 0 이상이어야 합니다.",
        ));
    }
    if !is_sha256(&payload.expected_render_signature) {
        return Err(AppError::new(
            "validation",
            "먼저 현재 설정으로 GIF 미리보기·용량 측정을 실행해 주세요.",
        ));
    }
    prepare_motion_render(
        connection,
        collection_id,
        payload.icon_id,
        payload.recipe,
        Some(payload.expected_revision),
        Some(payload.expected_render_signature),
    )
}

fn prepare_motion_render(
    connection: &Connection,
    collection_id: &str,
    icon_id: String,
    recipe: MotionRecipe,
    expected_revision: Option<i64>,
    expected_render_signature: Option<String>,
) -> AppResult<PreparedMotionRender> {
    let record = motion_render_record(connection, collection_id, &icon_id)?;
    let effects =
        effect_repository::effect_recipe_for_icon(connection, collection_id, &icon_id)?.recipe;
    let stored = motion_repository::motion_recipe_for_icon(connection, collection_id, &icon_id)?;
    if expected_revision.is_some_and(|expected| expected != stored.revision) {
        return Err(AppError::new(
            "conflict",
            "다른 편집기에서 모션이 먼저 변경되었습니다. 최신 저장값을 다시 불러와 주세요.",
        ));
    }
    let render_signature = visual_render_signature(&record, &effects, &recipe)?;
    if expected_render_signature
        .as_deref()
        .is_some_and(|expected| expected != render_signature)
    {
        return Err(AppError::new(
            "stale_measurement",
            "현재 설정과 마지막 GIF 측정 결과가 다릅니다. 다시 측정해 주세요.",
        ));
    }

    Ok(PreparedMotionRender {
        collection_id: collection_id.to_string(),
        icon_id,
        record,
        effects,
        recipe,
        expected_revision,
        render_signature,
    })
}

pub fn render_motion_preview(
    paths: &AppPaths,
    prepared: PreparedMotionRender,
) -> AppResult<MotionPreviewDto> {
    let mut request =
        OwnedMotionPreview::create(paths, &prepared.icon_id, &prepared.render_signature)?;
    let started = Instant::now();
    let generated = render_prepared(request.directory(), &prepared)?;
    let preview = motion_preview_dto(&prepared, &generated, started.elapsed().as_millis())?;
    request.mark_completed()?;
    prune_completed_previews(request.icon_root(), request.directory());
    Ok(preview)
}

pub fn render_motion_update(
    paths: &AppPaths,
    prepared: PreparedMotionRender,
) -> AppResult<RenderedMotionSave> {
    if prepared.expected_revision.is_none() {
        return Err(AppError::new(
            "validation",
            "모션 저장 revision이 준비되지 않았습니다.",
        ));
    }
    let root = paths
        .collection_previews_dir
        .join(&prepared.collection_id)
        .join(&prepared.icon_id)
        .join("motion");
    let artifact = OwnedMotionArtifact::create(&root, &prepared.render_signature)?;
    let generated = render_prepared(artifact.staging_dir(), &prepared)?;
    validate_generated_paths(&generated)?;
    Ok(RenderedMotionSave {
        prepared,
        artifact,
        generated,
    })
}

pub fn commit_motion_update(
    connection: &mut Connection,
    mut rendered: RenderedMotionSave,
) -> AppResult<IconEditorStateDto> {
    let expected_revision = rendered
        .prepared
        .expected_revision
        .ok_or_else(|| AppError::new("validation", "모션 저장 revision이 준비되지 않았습니다."))?;
    let collection_id = rendered.prepared.collection_id.clone();
    let icon_id = rendered.prepared.icon_id.clone();
    let recipe = rendered.prepared.recipe.clone();
    let expected_signature = rendered.prepared.render_signature.clone();
    let artifact_root = rendered.artifact.root().to_path_buf();

    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let current_record = motion_render_record(&transaction, &collection_id, &icon_id)?;
    let previous_preview_path = current_record.current_preview_path.clone();
    let current_effects =
        effect_repository::effect_recipe_for_icon(&transaction, &collection_id, &icon_id)?.recipe;
    let current_motion =
        motion_repository::motion_recipe_for_icon(&transaction, &collection_id, &icon_id)?;
    if current_motion.revision != expected_revision {
        return Err(AppError::new(
            "conflict",
            "다른 편집기에서 모션이 먼저 변경되었습니다. 최신 저장값을 다시 불러와 주세요.",
        ));
    }
    let current_signature = visual_render_signature(&current_record, &current_effects, &recipe)?;
    if current_signature != expected_signature {
        return Err(AppError::new(
            "conflict",
            "측정 이후 원본·자르기·정적 효과 또는 반복 설정이 변경되었습니다. 다시 측정해 주세요.",
        ));
    }

    let next_revision = motion_repository::upsert_motion_recipe(
        &transaction,
        &collection_id,
        &icon_id,
        expected_revision,
        &recipe,
    )?;
    let final_dir = rendered.artifact.promote(next_revision)?;
    rebase_generated_preview(
        &mut rendered.generated,
        rendered.artifact.staging_dir(),
        &final_dir,
    )?;
    update_persisted_preview(&transaction, &collection_id, &icon_id, &rendered.generated)?;
    let editor_state =
        editor_repository::get_icon_editor_state(&transaction, &collection_id, &icon_id)?;
    transaction.commit()?;
    rendered.artifact.keep_final();
    cleanup_previous_motion_artifact(
        connection,
        &artifact_root,
        previous_preview_path.as_deref(),
        &final_dir,
    );

    Ok(editor_state)
}

fn render_prepared(
    output_dir: &Path,
    prepared: &PreparedMotionRender,
) -> AppResult<GeneratedPreview> {
    let transform = ImageTransform::new(
        prepared.record.transform_quarter_turns,
        prepared.record.transform_flip_horizontal,
        prepared.record.transform_flip_vertical,
    )?;
    generate_icon_preview_in_directory(
        output_dir,
        GeneratePreviewRequest {
            collection_id: &prepared.collection_id,
            icon_id: &prepared.icon_id,
            source_path: Path::new(&prepared.record.original_path_in_library),
            source_extension: &prepared.record.original_extension,
            shape: &prepared.record.shape,
            crop: CropRect {
                x: prepared.record.crop_x,
                y: prepared.record.crop_y,
                width: prepared.record.crop_w,
                height: prepared.record.crop_h,
            },
            cell_width: prepared.record.cell_width,
            cell_height: prepared.record.cell_height,
            transform,
            gif_loop_mode: &prepared.record.gif_loop_mode,
            gif_loop_count: prepared.record.gif_loop_count,
            source_gif_loop_mode: Some(&prepared.record.original_loop_mode),
            source_gif_loop_count: prepared.record.original_loop_count,
            text_overlay: prepared.record.text_overlay.clone(),
            effects: prepared.effects.clone(),
            motion: prepared.recipe.clone(),
        },
    )
}

fn motion_render_record(
    connection: &Connection,
    collection_id: &str,
    icon_id: &str,
) -> AppResult<MotionRenderRecord> {
    connection
        .query_row(
            "SELECT
               s.original_path_in_library,
               s.original_extension,
               s.sha256 AS source_hash,
               COALESCE(s.original_loop_mode, 'preserve') AS original_loop_mode,
               s.original_loop_count,
               i.current_preview_path,
               i.shape,
               COALESCE(i.cell_width_override, c.default_cell_width) AS cell_width,
               COALESCE(i.cell_height_override, c.default_cell_height) AS cell_height,
               i.transform_quarter_turns,
               i.transform_flip_horizontal,
               i.transform_flip_vertical,
               CASE WHEN i.gif_pingpong = 1 THEN 'pingpong' ELSE i.gif_loop_mode END AS gif_loop_mode,
               i.gif_loop_count,
               cs.crop_x,
               cs.crop_y,
               cs.crop_w,
               cs.crop_h,
               c.max_bytes,
               i.text_overlay_enabled,
               i.text_overlay_text,
               i.text_overlay_font_path,
               i.text_overlay_font_size,
               i.text_overlay_x,
               i.text_overlay_y,
               i.text_overlay_color,
               i.text_overlay_stroke_color,
               i.text_overlay_stroke_width
             FROM icons i
             JOIN source_files s ON s.id = i.source_file_id
             JOIN collections c ON c.id = i.collection_id
             JOIN crop_settings cs ON cs.icon_id = i.id
             WHERE i.id = ?1
               AND i.collection_id = ?2
               AND i.deleted_at IS NULL
               AND c.deleted_at IS NULL",
            params![icon_id, collection_id],
            |row| {
                let text_enabled = row.get::<_, i64>("text_overlay_enabled")? != 0;
                let text_overlay = text_overlay_from_fields(
                    text_enabled,
                    row.get("text_overlay_text")?,
                    row.get("text_overlay_font_path")?,
                    row.get("text_overlay_font_size")?,
                    row.get("text_overlay_x")?,
                    row.get("text_overlay_y")?,
                    row.get("text_overlay_color")?,
                    row.get("text_overlay_stroke_color")?,
                    row.get("text_overlay_stroke_width")?,
                )
                .map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?;
                Ok(MotionRenderRecord {
                    original_path_in_library: row.get("original_path_in_library")?,
                    original_extension: row.get("original_extension")?,
                    source_hash: row.get("source_hash")?,
                    original_loop_mode: row.get("original_loop_mode")?,
                    original_loop_count: row.get("original_loop_count")?,
                    current_preview_path: row.get("current_preview_path")?,
                    shape: row.get("shape")?,
                    cell_width: row.get("cell_width")?,
                    cell_height: row.get("cell_height")?,
                    transform_quarter_turns: row.get("transform_quarter_turns")?,
                    transform_flip_horizontal:
                        row.get::<_, i64>("transform_flip_horizontal")? != 0,
                    transform_flip_vertical:
                        row.get::<_, i64>("transform_flip_vertical")? != 0,
                    gif_loop_mode: row.get("gif_loop_mode")?,
                    gif_loop_count: row.get("gif_loop_count")?,
                    crop_x: row.get("crop_x")?,
                    crop_y: row.get("crop_y")?,
                    crop_w: row.get("crop_w")?,
                    crop_h: row.get("crop_h")?,
                    max_bytes: row.get("max_bytes")?,
                    text_overlay,
                })
            },
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("모션을 편집할 아이콘을 찾을 수 없습니다."))
}

fn visual_render_signature(
    record: &MotionRenderRecord,
    effects: &EffectRecipe,
    motion: &MotionRecipe,
) -> AppResult<String> {
    let mut parts = vec![
        "motion_render_v1".to_string(),
        record.source_hash.clone(),
        record.original_extension.clone(),
        record.original_loop_mode.clone(),
        record.original_loop_count.unwrap_or_default().to_string(),
        record.shape.clone(),
        record.cell_width.to_string(),
        record.cell_height.to_string(),
        record.transform_quarter_turns.to_string(),
        record.transform_flip_horizontal.to_string(),
        record.transform_flip_vertical.to_string(),
        record.gif_loop_mode.clone(),
        record.gif_loop_count.unwrap_or_default().to_string(),
        record.crop_x.to_bits().to_string(),
        record.crop_y.to_bits().to_string(),
        record.crop_w.to_bits().to_string(),
        record.crop_h.to_bits().to_string(),
        record.max_bytes.to_string(),
    ];
    if let Some(text_overlay) = &record.text_overlay {
        parts.push("text:enabled".to_string());
        parts.extend(text_overlay.normalized_hash_parts());
    } else {
        parts.push("text:none".to_string());
    }
    parts.extend(effects.normalized_hash_parts()?);
    parts.extend(motion.normalized_hash_parts()?);
    Ok(hash_text(&parts))
}

fn motion_preview_dto(
    prepared: &PreparedMotionRender,
    generated: &GeneratedPreview,
    processing_ms: u128,
) -> AppResult<MotionPreviewDto> {
    validate_generated_paths(generated)?;
    let piece_byte_sizes = generated
        .piece_paths
        .iter()
        .map(|path| metadata_size(path))
        .collect::<AppResult<Vec<_>>>()?;
    let max_piece_byte_size = piece_byte_sizes.iter().copied().max().unwrap_or(0);
    let max_bytes = prepared.record.max_bytes.max(1);
    let passes_byte_limit = max_piece_byte_size <= max_bytes;
    let clipped = generated.clipped_frame_count > 0 || generated.clipped_pixel_count > 0;
    let mut warnings = Vec::new();
    if !passes_byte_limit {
        warnings.push(format!(
            "가장 큰 출력 조각이 {}로 모음 제한 {}를 넘습니다.",
            format_bytes(max_piece_byte_size),
            format_bytes(max_bytes),
        ));
    }
    if clipped {
        warnings.push(format!(
            "{}개 프레임에서 캔버스 가장자리 잘림이 감지되었습니다.",
            generated.clipped_frame_count
        ));
    }
    if prepared.record.original_extension != "gif" && prepared.recipe.has_enabled_motion() {
        warnings.push(
            "모션이 활성화되어 정적 원본도 GIF 형식으로 미리보기·내보내기 됩니다.".to_string(),
        );
    }
    if generated.frame_count >= 200 {
        warnings.push(format!(
            "{}프레임을 인코딩하므로 편집과 내보내기에 시간이 걸릴 수 있습니다.",
            generated.frame_count
        ));
    }
    let generated_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().to_string())
        .unwrap_or_else(|_| prepared.render_signature.clone());
    let (loop_mode, loop_count) = effective_repeat_metadata(&prepared.record)?;

    Ok(MotionPreviewDto {
        preview_path: generated.current_preview_path.to_string_lossy().to_string(),
        poster_path: generated.poster_path.to_string_lossy().to_string(),
        byte_size: i64::try_from(generated.encoded_byte_size).unwrap_or(i64::MAX),
        piece_byte_sizes,
        max_piece_byte_size,
        max_bytes,
        passes_byte_limit,
        frame_count: i64::try_from(generated.frame_count).unwrap_or(i64::MAX),
        duration_ms: i64::try_from(generated.duration_ms).unwrap_or(i64::MAX),
        effective_fps: generated.effective_fps,
        timing_source: if prepared.record.original_extension == "gif" {
            "source_gif".to_string()
        } else {
            "generated".to_string()
        },
        loop_mode,
        loop_count,
        clipped,
        clipped_frame_count: i64::try_from(generated.clipped_frame_count).unwrap_or(i64::MAX),
        processing_ms: i64::try_from(processing_ms).unwrap_or(i64::MAX),
        warnings,
        render_signature: prepared.render_signature.clone(),
        generated_at,
    })
}

fn effective_repeat_metadata(record: &MotionRenderRecord) -> AppResult<(String, Option<i64>)> {
    if record.gif_loop_mode == "pingpong" {
        return Ok(("pingpong".to_string(), None));
    }

    Ok(
        match output_repeat_for_settings(
            &record.gif_loop_mode,
            record.gif_loop_count,
            &record.original_loop_mode,
            record.original_loop_count,
        )? {
            GifOutputRepeat::Once => ("once".to_string(), None),
            GifOutputRepeat::Infinite => ("infinite".to_string(), None),
            GifOutputRepeat::Finite(count) => ("count".to_string(), Some(i64::from(count))),
        },
    )
}

fn update_persisted_preview(
    transaction: &rusqlite::Transaction<'_>,
    collection_id: &str,
    icon_id: &str,
    generated: &GeneratedPreview,
) -> AppResult<()> {
    transaction.execute(
        "UPDATE icons
         SET current_preview_path = ?1,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?2
           AND collection_id = ?3
           AND deleted_at IS NULL",
        params![
            generated.current_preview_path.to_string_lossy().as_ref(),
            icon_id,
            collection_id,
        ],
    )?;
    let piece_ids = {
        let mut statement = transaction.prepare(
            "SELECT id
             FROM icon_pieces
             WHERE icon_id = ?1
             ORDER BY piece_index ASC",
        )?;
        let rows = statement
            .query_map([icon_id], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        rows
    };
    if piece_ids.len() != generated.piece_paths.len() {
        return Err(AppError::new(
            "conflict",
            "모션 렌더 중 아이콘 조각 구성이 변경되었습니다. 다시 시도해 주세요.",
        ));
    }
    for (piece_id, path) in piece_ids.iter().zip(&generated.piece_paths) {
        transaction.execute(
            "UPDATE icon_pieces
             SET generated_preview_path = ?1,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?2
               AND icon_id = ?3",
            params![path.to_string_lossy().as_ref(), piece_id, icon_id],
        )?;
    }
    transaction.execute(
        "UPDATE collections
         SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?1
           AND cover_icon_id = ?2
           AND deleted_at IS NULL",
        params![collection_id, icon_id],
    )?;
    Ok(())
}

fn cleanup_previous_motion_artifact(
    connection: &Connection,
    artifact_root: &Path,
    previous: Option<&str>,
    current: &Path,
) {
    let Some(previous) = previous else {
        return;
    };
    let previous = Path::new(previous);
    let Some(previous_dir) = previous.parent() else {
        return;
    };
    if previous_dir == current || !is_owned_motion_artifact_directory(artifact_root, previous_dir) {
        return;
    }

    let (Ok(canonical_root), Ok(canonical_previous)) =
        (artifact_root.canonicalize(), previous_dir.canonicalize())
    else {
        return;
    };
    let Some(directory_name) = previous_dir.file_name() else {
        return;
    };
    if canonical_previous != canonical_root.join(directory_name) {
        return;
    }
    if current
        .canonicalize()
        .ok()
        .is_some_and(|canonical_current| canonical_current == canonical_previous)
    {
        return;
    }
    if matches!(
        motion_artifact_directory_is_referenced(connection, previous_dir),
        Ok(false)
    ) {
        let _ = fs::remove_dir_all(canonical_previous);
    }
}

fn is_owned_motion_artifact_directory(artifact_root: &Path, candidate: &Path) -> bool {
    let Some(relative) = candidate.strip_prefix(artifact_root).ok() else {
        return false;
    };
    let mut components = relative.components();
    let Some(Component::Normal(name)) = components.next() else {
        return false;
    };
    if components.next().is_some() {
        return false;
    }
    let Some(name) = name.to_str() else {
        return false;
    };
    let mut parts = name.splitn(3, '-');
    let Some(revision) = parts.next() else {
        return false;
    };
    let Some(signature) = parts.next() else {
        return false;
    };
    let Some(request_token) = parts.next() else {
        return false;
    };

    revision
        .parse::<i64>()
        .ok()
        .is_some_and(|revision| revision >= 1)
        && signature.len() == 16
        && signature
            .chars()
            .all(|character| character.is_ascii_hexdigit())
        && request_token.starts_with("motionsave_")
        && request_token
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
}

fn motion_artifact_directory_is_referenced(
    connection: &Connection,
    directory: &Path,
) -> AppResult<bool> {
    let mut statement = connection.prepare(
        "SELECT current_preview_path AS referenced_path
         FROM icons
         WHERE current_preview_path IS NOT NULL
         UNION ALL
         SELECT generated_preview_path AS referenced_path
         FROM icon_pieces
         WHERE generated_preview_path IS NOT NULL",
    )?;
    let referenced_paths = statement.query_map([], |row| row.get::<_, String>(0))?;
    for referenced_path in referenced_paths {
        if Path::new(&referenced_path?).starts_with(directory) {
            return Ok(true);
        }
    }
    Ok(false)
}
fn validate_generated_paths(generated: &GeneratedPreview) -> AppResult<()> {
    for path in std::iter::once(&generated.current_preview_path)
        .chain(std::iter::once(&generated.poster_path))
        .chain(generated.piece_paths.iter())
    {
        if !path.is_file() {
            return Err(AppError::new(
                "motion_render",
                "모션 렌더 결과 파일을 찾을 수 없습니다.",
            ));
        }
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if !matches!(extension.as_str(), "png" | "gif") {
            return Err(AppError::new(
                "motion_render",
                "모션 렌더 결과는 PNG 또는 GIF여야 합니다.",
            ));
        }
    }
    Ok(())
}

fn rebase_generated_preview(
    generated: &mut GeneratedPreview,
    staging_dir: &Path,
    final_dir: &Path,
) -> AppResult<()> {
    generated.current_preview_path =
        rebase_path(&generated.current_preview_path, staging_dir, final_dir)?;
    generated.poster_path = rebase_path(&generated.poster_path, staging_dir, final_dir)?;
    for piece_path in &mut generated.piece_paths {
        *piece_path = rebase_path(piece_path, staging_dir, final_dir)?;
    }
    Ok(())
}

fn rebase_path(path: &Path, staging_dir: &Path, final_dir: &Path) -> AppResult<PathBuf> {
    let relative = path.strip_prefix(staging_dir).map_err(|_| {
        AppError::new(
            "motion_artifact_path",
            "모션 렌더 결과가 전용 임시 폴더 밖을 가리킵니다.",
        )
    })?;
    Ok(final_dir.join(relative))
}

fn metadata_size(path: &Path) -> AppResult<i64> {
    Ok(i64::try_from(fs::metadata(path)?.len()).unwrap_or(i64::MAX))
}

fn format_bytes(bytes: i64) -> String {
    let bytes = bytes.max(0) as f64;
    if bytes < 1024.0 {
        return format!("{bytes:.0}B");
    }
    if bytes < 1024.0 * 1024.0 {
        return format!("{:.1}KB", bytes / 1024.0);
    }
    format!("{:.1}MB", bytes / (1024.0 * 1024.0))
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.chars().all(|character| character.is_ascii_hexdigit())
}

fn safe_component(value: &str) -> bool {
    if value.is_empty() || value.len() > 160 {
        return false;
    }
    let mut components = Path::new(value).components();
    matches!(components.next(), Some(Component::Normal(_)))
        && components.next().is_none()
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
}

#[derive(Debug)]
struct OwnedMotionPreview {
    icon_root: PathBuf,
    directory: PathBuf,
    completed: bool,
}

impl OwnedMotionPreview {
    fn create(paths: &AppPaths, icon_id: &str, signature: &str) -> AppResult<Self> {
        if !safe_component(icon_id) || !is_sha256(signature) {
            return Err(AppError::new(
                "motion_preview_path",
                "모션 미리보기 경로 구성값이 올바르지 않습니다.",
            ));
        }
        let root = paths.temp_export_dir.join(MOTION_PREVIEW_DIRECTORY);
        fs::create_dir_all(&root)?;
        let icon_root = root.join(icon_id);
        fs::create_dir_all(&icon_root)?;
        for _ in 0..32 {
            let token = create_id("motionpreview");
            let directory = icon_root.join(format!("{}-{token}", &signature[..16]));
            match fs::create_dir(&directory) {
                Ok(()) => {
                    fs::write(directory.join(IN_PROGRESS_MARKER), b"")?;
                    return Ok(Self {
                        icon_root,
                        directory,
                        completed: false,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            }
        }
        Err(AppError::new(
            "motion_preview_path",
            "모션 미리보기 임시 폴더를 만들 수 없습니다.",
        ))
    }

    fn directory(&self) -> &Path {
        &self.directory
    }

    fn icon_root(&self) -> &Path {
        &self.icon_root
    }

    fn mark_completed(&mut self) -> AppResult<()> {
        fs::rename(
            self.directory.join(IN_PROGRESS_MARKER),
            self.directory.join(COMPLETE_MARKER),
        )?;
        self.completed = true;
        Ok(())
    }
}

impl Drop for OwnedMotionPreview {
    fn drop(&mut self) {
        if !self.completed
            && self.directory.parent() == Some(self.icon_root.as_path())
            && self.directory.starts_with(&self.icon_root)
        {
            let _ = fs::remove_dir_all(&self.directory);
        }
    }
}

fn prune_completed_previews(icon_root: &Path, current: &Path) {
    let Ok(entries) = fs::read_dir(icon_root) else {
        return;
    };
    let mut completed = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path != current
                && path.parent() == Some(icon_root)
                && path.join(COMPLETE_MARKER).is_file()
        })
        .filter_map(|path| {
            let modified = fs::metadata(&path).ok()?.modified().ok()?;
            Some((path, modified))
        })
        .collect::<Vec<_>>();
    completed.sort_by(|left, right| right.1.cmp(&left.1));
    for (path, _) in completed
        .into_iter()
        .skip(MAX_COMPLETED_PREVIEWS_PER_ICON.saturating_sub(1))
    {
        let _ = fs::remove_dir_all(path);
    }
}

#[derive(Debug)]
struct OwnedMotionArtifact {
    root: PathBuf,
    signature_prefix: String,
    token: String,
    staging_dir: PathBuf,
    final_dir: Option<PathBuf>,
    keep: bool,
}

impl OwnedMotionArtifact {
    fn create(root: &Path, signature: &str) -> AppResult<Self> {
        if !is_sha256(signature) {
            return Err(AppError::new(
                "motion_artifact_path",
                "모션 렌더 서명이 올바르지 않습니다.",
            ));
        }
        let staging_root = root.join(".staging");
        fs::create_dir_all(&staging_root)?;
        for _ in 0..32 {
            let token = create_id("motionsave");
            let staging_dir = staging_root.join(&token);
            match fs::create_dir(&staging_dir) {
                Ok(()) => {
                    return Ok(Self {
                        root: root.to_path_buf(),
                        signature_prefix: signature[..16].to_string(),
                        token,
                        staging_dir,
                        final_dir: None,
                        keep: false,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            }
        }
        Err(AppError::new(
            "motion_artifact_path",
            "모션 저장 임시 폴더를 만들 수 없습니다.",
        ))
    }

    fn root(&self) -> &Path {
        &self.root
    }

    fn staging_dir(&self) -> &Path {
        &self.staging_dir
    }

    fn promote(&mut self, revision: i64) -> AppResult<PathBuf> {
        let final_dir = self.root.join(format!(
            "{revision}-{}-{}",
            self.signature_prefix, self.token
        ));
        if final_dir.exists() {
            return Err(AppError::new(
                "motion_artifact_collision",
                "같은 모션 저장 결과 폴더가 이미 존재합니다.",
            ));
        }
        fs::rename(&self.staging_dir, &final_dir)?;
        self.final_dir = Some(final_dir.clone());
        Ok(final_dir)
    }

    fn keep_final(&mut self) {
        self.keep = true;
    }
}

impl Drop for OwnedMotionArtifact {
    fn drop(&mut self) {
        if self.keep {
            return;
        }
        if self.staging_dir == self.root.join(".staging").join(&self.token) {
            let _ = fs::remove_dir_all(&self.staging_dir);
        }
        if let Some(final_dir) = &self.final_dir {
            if final_dir.parent() == Some(self.root.as_path())
                && final_dir
                    .file_name()
                    .and_then(|value| value.to_str())
                    .is_some_and(|value| value.ends_with(&self.token))
            {
                let _ = fs::remove_dir_all(final_dir);
            }
        }
    }
}
