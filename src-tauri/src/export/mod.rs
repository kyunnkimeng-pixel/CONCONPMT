use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::db::repositories::ai as ai_repository;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::db::repositories::export_profiles as export_profile_repository;
use crate::db::repositories::optimization as optimization_repository;
use crate::error::{AppError, AppResult};
use crate::imaging::effects::{parse_effect_recipe_json, EffectRecipe};
use crate::imaging::export_render::{
    render_icon_export, ExportCropRect, ExportRenderPiece, ExportRenderRequest,
};
use crate::imaging::geometry::piece_roles;
use crate::imaging::motion::{parse_motion_recipe_json, MotionRecipe};
use crate::imaging::text_overlay::{text_overlay_from_fields, TextOverlayRenderSpec};
use crate::imaging::transform::ImageTransform;
use crate::models::{
    ExportCollectionResultDto, ExportPlanItemDto, ExportProfileDto, ExportRequestPayload,
    ExportValidationIssueDto, ExportValidationResultDto,
};
use crate::optimization::cache::{hash_text, render_recipe_crop_hash};
use crate::paths::AppPaths;

pub fn validate_export_collection(
    connection: &Connection,
    paths: &AppPaths,
    collection_id: &str,
    payload: &ExportRequestPayload,
) -> AppResult<ExportValidationResultDto> {
    let profile = export_profile_repository::update_export_profile_settings(
        connection,
        collection_id,
        payload,
    )?;
    let plan = load_export_plan(
        connection,
        paths,
        collection_id,
        profile,
        &payload.excluded_piece_ids,
        &payload.resize_filter,
    )?;
    let mut issues = validate_plan_before_render(&plan);
    issues.extend(validate_active_variant_sizes(&plan));
    Ok(validation_result(&plan, issues))
}

pub fn export_collection(
    connection: &mut Connection,
    paths: &AppPaths,
    collection_id: &str,
    payload: &ExportRequestPayload,
) -> AppResult<ExportCollectionResultDto> {
    let profile = export_profile_repository::update_export_profile_settings(
        connection,
        collection_id,
        payload,
    )?;
    let mut plan = load_export_plan(
        connection,
        paths,
        collection_id,
        profile,
        &payload.excluded_piece_ids,
        &payload.resize_filter,
    )?;
    let mut issues = validate_plan_before_render(&plan);

    if plan.output_count() == 0 || !session_blocking_errors(&issues).is_empty() {
        let validation = validation_result(&plan, issues);
        return Ok(ExportCollectionResultDto {
            validation,
            export_directory: None,
            alt_txt_path: None,
            manifest_path: None,
            report_txt_path: None,
            report_json_path: None,
            issues_csv_path: None,
        });
    }

    let output_root = output_root(paths, payload)?;
    fs::create_dir_all(&output_root)?;
    let temp_dir = unique_child_dir(&output_root, ".pmtconcon-export-temp")?;
    fs::create_dir_all(&temp_dir)?;
    let files_dir = temp_dir.join("files");
    fs::create_dir_all(&files_dir)?;

    let render_result = render_plan(&plan, &files_dir);
    apply_rendered_metadata(&mut plan, render_result.rendered_files);
    issues.extend(render_result.issues);
    issues.extend(validate_plan_after_render(&plan, &issues));

    let final_dir = unique_export_dir(&output_root, &plan.collection_name)?;
    apply_final_paths(&mut plan, &final_dir);
    let validation = validation_result(&plan, issues);

    let alt_txt_path = if plan.profile.include_alt_txt {
        Some(write_alts_txt(&temp_dir, &plan)?)
    } else {
        None
    };
    let report_paths = write_export_reports(&temp_dir, &plan, &validation)?;
    finalize_export_directory(&temp_dir, &final_dir)?;

    let final_alt_txt_path = alt_txt_path
        .as_ref()
        .and_then(|path| path.file_name())
        .map(|file_name| final_dir.join(file_name));
    let final_report_txt_path = report_paths
        .txt_path
        .file_name()
        .map(|file_name| final_dir.join(file_name))
        .ok_or_else(|| AppError::new("export", "report path could not be created."))?;
    let final_report_json_path = report_paths
        .json_path
        .file_name()
        .map(|file_name| final_dir.join(file_name))
        .ok_or_else(|| AppError::new("export", "report path could not be created."))?;
    let final_issues_csv_path = report_paths
        .issues_csv_path
        .file_name()
        .map(|file_name| final_dir.join(file_name))
        .ok_or_else(|| AppError::new("export", "manifest 경로를 만들 수 없습니다."))?;

    update_export_status(connection, &plan, &final_dir, &validation)?;

    if payload.open_folder_after_export {
        open_path(&final_dir, OpenMode::Folder)?;
    }
    if payload.open_alt_txt_after_export {
        if let Some(path) = &final_alt_txt_path {
            open_path(path, OpenMode::TextFile)?;
        }
    }

    Ok(ExportCollectionResultDto {
        validation,
        export_directory: Some(path_string(&final_dir)),
        alt_txt_path: final_alt_txt_path.map(|path| path_string(&path)),
        manifest_path: Some(path_string(&final_report_json_path)),
        report_txt_path: Some(path_string(&final_report_txt_path)),
        report_json_path: Some(path_string(&final_report_json_path)),
        issues_csv_path: Some(path_string(&final_issues_csv_path)),
    })
}

pub fn export_selected_collection_items(
    connection: &mut Connection,
    paths: &AppPaths,
    collection_id: &str,
    payload: &ExportRequestPayload,
    selected_piece_ids: &[String],
    export_directory: &str,
) -> AppResult<ExportCollectionResultDto> {
    let selected_piece_ids: HashSet<String> = selected_piece_ids.iter().cloned().collect();
    if selected_piece_ids.is_empty() {
        return Err(AppError::new(
            "validation",
            "다시 내보낼 선택 항목이 없습니다.",
        ));
    }

    let final_dir = PathBuf::from(export_directory.trim());
    if !final_dir.is_absolute() {
        return Err(AppError::new(
            "validation",
            "기존 내보내기 폴더는 절대 경로여야 합니다.",
        ));
    }
    if !final_dir.is_dir() {
        return Err(AppError::not_found(
            "기존 내보내기 폴더를 찾을 수 없습니다. 먼저 전체 내보내기를 실행해 주세요.",
        ));
    }

    let profile = export_profile_repository::update_export_profile_settings(
        connection,
        collection_id,
        payload,
    )?;
    let mut plan = load_export_plan(
        connection,
        paths,
        collection_id,
        profile,
        &payload.excluded_piece_ids,
        &payload.resize_filter,
    )?;
    let mut issues = validate_plan_before_render(&plan);

    let selected_included_piece_ids = plan
        .icons
        .iter()
        .flat_map(|icon| icon.pieces.iter())
        .filter(|piece| piece.included && selected_piece_ids.contains(&piece.piece_id))
        .map(|piece| piece.piece_id.clone())
        .collect::<HashSet<_>>();

    if selected_included_piece_ids.is_empty() {
        return Err(AppError::new(
            "validation",
            "선택 항목 중 내보내기에 포함된 항목이 없습니다.",
        ));
    }

    if plan.output_count() == 0 || !session_blocking_errors(&issues).is_empty() {
        let validation = validation_result(&plan, issues);
        return Ok(ExportCollectionResultDto {
            validation,
            export_directory: Some(path_string(&final_dir)),
            alt_txt_path: None,
            manifest_path: None,
            report_txt_path: None,
            report_json_path: None,
            issues_csv_path: None,
        });
    }

    let final_files_dir = final_dir.join("files");
    fs::create_dir_all(&final_files_dir)?;

    let temp_dir = unique_child_dir(&final_dir, ".pmtconcon-selected-export-temp")?;
    let temp_files_dir = temp_dir.join("files");
    fs::create_dir_all(&temp_files_dir)?;

    let render_result = render_plan_selected(&plan, &temp_files_dir, &selected_included_piece_ids);
    let replace_result = replace_rendered_files(render_result.rendered_files, &final_files_dir);
    issues.extend(render_result.issues);
    issues.extend(replace_result.issues);
    apply_rendered_metadata(&mut plan, replace_result.rendered_files);
    hydrate_existing_export_files(&mut plan, &final_files_dir);
    issues.extend(validate_plan_after_render(&plan, &issues));
    apply_final_paths(&mut plan, &final_dir);

    let validation = validation_result(&plan, issues);

    let final_alt_txt_path = if plan.profile.include_alt_txt {
        Some(write_alts_txt(&final_dir, &plan)?)
    } else {
        None
    };
    let report_paths = write_export_reports(&final_dir, &plan, &validation)?;
    let _ = fs::remove_dir_all(&temp_dir);

    update_export_status(connection, &plan, &final_dir, &validation)?;

    if payload.open_folder_after_export {
        open_path(&final_dir, OpenMode::Folder)?;
    }
    if payload.open_alt_txt_after_export {
        if let Some(path) = &final_alt_txt_path {
            open_path(path, OpenMode::TextFile)?;
        }
    }

    Ok(ExportCollectionResultDto {
        validation,
        export_directory: Some(path_string(&final_dir)),
        alt_txt_path: final_alt_txt_path.map(|path| path_string(&path)),
        manifest_path: Some(path_string(&report_paths.json_path)),
        report_txt_path: Some(path_string(&report_paths.txt_path)),
        report_json_path: Some(path_string(&report_paths.json_path)),
        issues_csv_path: Some(path_string(&report_paths.issues_csv_path)),
    })
}

pub fn open_export_path(path: &str) -> AppResult<()> {
    let path = PathBuf::from(path);
    if !path.exists() {
        return Err(AppError::not_found("열 경로를 찾을 수 없습니다."));
    }

    let mode = if path.is_dir() {
        OpenMode::Folder
    } else if path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("txt"))
    {
        OpenMode::TextFile
    } else {
        OpenMode::Reveal
    };

    open_path(&path, mode)
}

#[derive(Debug, Clone)]
struct ExportPlan {
    collection_id: String,
    collection_name: String,
    profile: ExportProfileDto,
    resize_filter: String,
    icons: Vec<PlannedIcon>,
}

#[derive(Debug, Clone)]
struct PlannedIcon {
    icon_id: String,
    display_name: String,
    shape: String,
    source_path: PathBuf,
    source_extension: String,
    source_preview_url: Option<String>,
    source_is_animated: bool,
    source_width: i64,
    source_height: i64,
    source_gif_loop_mode: String,
    source_gif_loop_count: Option<i64>,
    crop: ExportCropRect,
    cell_width: i64,
    cell_height: i64,
    transform: ImageTransform,
    output_format: String,
    gif_loop_mode: String,
    gif_loop_count: Option<i64>,
    text_overlay: Option<TextOverlayRenderSpec>,
    effects: EffectRecipe,
    motion: MotionRecipe,
    pieces: Vec<PlannedPiece>,
}

#[derive(Debug, Clone)]
struct PlannedPiece {
    piece_id: String,
    piece_index: usize,
    piece_role: String,
    alt_text: String,
    included: bool,
    export_index: i64,
    file_name: String,
    byte_size: Option<i64>,
    output_path: Option<PathBuf>,
    active_variant: Option<ActiveExportVariant>,
    used_optimized_variant: bool,
}

#[derive(Debug, Clone)]
struct ActiveExportVariant {
    id: String,
    path: PathBuf,
    byte_size: i64,
}

#[derive(Debug)]
struct CollectionExportRecord {
    id: String,
    name: String,
    default_cell_width: i64,
    default_cell_height: i64,
}

#[derive(Debug)]
struct IconExportRecord {
    id: String,
    display_name: String,
    shape: String,
    source_file_id: String,
    cell_width_override: Option<i64>,
    cell_height_override: Option<i64>,
    transform_quarter_turns: i64,
    transform_flip_horizontal: bool,
    transform_flip_vertical: bool,
    gif_loop_mode: String,
    gif_loop_count: Option<i64>,
    source_path: String,
    source_extension: String,
    thumbnail_override_path: Option<String>,
    thumbnail_path: Option<String>,
    current_preview_path: Option<String>,
    source_is_animated: bool,
    source_width: i64,
    source_height: i64,
    source_gif_loop_mode: String,
    source_gif_loop_count: Option<i64>,
    source_hash: String,
    crop_x: f64,
    crop_y: f64,
    crop_w: f64,
    crop_h: f64,
    text_overlay_enabled: bool,
    text_overlay_text: String,
    text_overlay_font_path: Option<String>,
    text_overlay_font_size: f64,
    text_overlay_x: f64,
    text_overlay_y: f64,
    text_overlay_color: String,
    text_overlay_stroke_color: String,
    text_overlay_stroke_width: f64,
    effect_recipe_json: Option<String>,
    motion_recipe_json: Option<String>,
}

#[derive(Debug)]
struct PieceExportRecord {
    id: String,
    piece_index: i64,
    piece_role: String,
    alt_text: String,
}

fn load_export_plan(
    connection: &Connection,
    paths: &AppPaths,
    collection_id: &str,
    profile: ExportProfileDto,
    excluded_piece_ids: &[String],
    resize_filter: &str,
) -> AppResult<ExportPlan> {
    let collection = load_collection(connection, collection_id)?;
    let icon_records = load_icons(connection, collection_id)?;
    let mut icons = Vec::with_capacity(icon_records.len());
    let excluded_piece_ids: HashSet<&str> = excluded_piece_ids.iter().map(String::as_str).collect();

    for icon in icon_records {
        let pieces = load_pieces(connection, &icon.id)?;
        let cell_width = icon
            .cell_width_override
            .unwrap_or(collection.default_cell_width)
            .max(1);
        let cell_height = icon
            .cell_height_override
            .unwrap_or(collection.default_cell_height)
            .max(1);
        let crop = ExportCropRect {
            x: icon.crop_x,
            y: icon.crop_y,
            width: icon.crop_w,
            height: icon.crop_h,
        };
        let text_overlay = text_overlay_from_fields(
            icon.text_overlay_enabled,
            Some(icon.text_overlay_text.clone()),
            icon.text_overlay_font_path.clone(),
            Some(icon.text_overlay_font_size),
            Some(icon.text_overlay_x),
            Some(icon.text_overlay_y),
            Some(icon.text_overlay_color.clone()),
            Some(icon.text_overlay_stroke_color.clone()),
            Some(icon.text_overlay_stroke_width),
        )?;
        let transform = ImageTransform::new(
            icon.transform_quarter_turns,
            icon.transform_flip_horizontal,
            icon.transform_flip_vertical,
        )?;
        let effects =
            parse_effect_recipe_json(icon.effect_recipe_json.as_deref().unwrap_or_default())?;
        let motion =
            parse_motion_recipe_json(icon.motion_recipe_json.as_deref().unwrap_or_default())?;
        let output_format = output_format_for_icon(
            &profile.target_format,
            &icon.source_extension,
            motion.has_enabled_motion(),
        );
        let profile_hash = active_variant_profile_hash(
            &profile,
            &output_format,
            cell_width,
            cell_height,
            resize_filter,
        );
        let mut planned_pieces = Vec::with_capacity(pieces.len());

        for piece in pieces {
            let piece_index = usize::try_from(piece.piece_index.max(0)).unwrap_or(0);
            let crop_hash = render_recipe_crop_hash(
                &icon.shape,
                &crop,
                cell_width,
                cell_height,
                piece_index,
                transform,
                &icon.gif_loop_mode,
                icon.gif_loop_count,
                text_overlay.as_ref(),
                &effects,
                &motion,
            )?;
            let active_variant = optimization_repository::find_active_variant(
                connection,
                paths,
                &icon.id,
                &profile.id,
                &piece.id,
                &icon.source_hash,
                &crop_hash,
                &profile_hash,
                &output_format,
            )?
            .filter(|variant| {
                variant.source_file_id.as_deref() == Some(icon.source_file_id.as_str())
            })
            .map(|variant| ActiveExportVariant {
                id: variant.id,
                path: PathBuf::from(variant.path),
                byte_size: variant.byte_size,
            });

            planned_pieces.push(PlannedPiece {
                included: !excluded_piece_ids.contains(piece.id.as_str()),
                piece_id: piece.id,
                piece_index,
                piece_role: piece.piece_role,
                alt_text: piece.alt_text.trim().to_string(),
                export_index: 0,
                file_name: String::new(),
                byte_size: active_variant.as_ref().map(|variant| variant.byte_size),
                output_path: active_variant.as_ref().map(|variant| variant.path.clone()),
                active_variant,
                used_optimized_variant: false,
            });
        }

        icons.push(PlannedIcon {
            icon_id: icon.id,
            display_name: icon.display_name,
            shape: icon.shape,
            source_path: PathBuf::from(icon.source_path),
            source_extension: normalize_format(&icon.source_extension),
            source_preview_url: icon
                .thumbnail_override_path
                .or(icon.current_preview_path)
                .or(icon.thumbnail_path),
            source_is_animated: icon.source_is_animated,
            source_width: icon.source_width,
            source_height: icon.source_height,
            source_gif_loop_mode: icon.source_gif_loop_mode,
            source_gif_loop_count: icon.source_gif_loop_count,
            crop,
            cell_width,
            cell_height,
            transform,
            output_format,
            gif_loop_mode: icon.gif_loop_mode,
            gif_loop_count: icon.gif_loop_count,
            text_overlay,
            effects,
            motion,
            pieces: planned_pieces,
        });
    }

    let mut plan = ExportPlan {
        collection_id: collection.id,
        collection_name: collection.name,
        profile,
        resize_filter: normalize_resize_filter(resize_filter),
        icons,
    };
    assign_filenames(&mut plan)?;

    Ok(plan)
}

fn load_collection(
    connection: &Connection,
    collection_id: &str,
) -> AppResult<CollectionExportRecord> {
    connection
        .query_row(
            "SELECT
               id,
               name,
               default_cell_width,
               default_cell_height
             FROM collections
             WHERE id = ?1
               AND deleted_at IS NULL",
            params![collection_id],
            |row| {
                Ok(CollectionExportRecord {
                    id: row.get("id")?,
                    name: row.get("name")?,
                    default_cell_width: row.get("default_cell_width")?,
                    default_cell_height: row.get("default_cell_height")?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("내보낼 모음을 찾을 수 없습니다."))
}

fn load_icons(connection: &Connection, collection_id: &str) -> AppResult<Vec<IconExportRecord>> {
    let expected_icon_ids = {
        let mut statement = connection.prepare(
            "SELECT id
             FROM icons
             WHERE collection_id = ?1
               AND deleted_at IS NULL
               AND icon_kind = 'image'
               AND readiness = 'complete'
             ORDER BY order_index ASC, created_at ASC",
        )?;
        let ids = statement
            .query_map(params![collection_id], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        ids
    };

    // Resolve every exportable icon before the inner-join query below. This is
    // intentionally fail-closed: a broken AI pointer or managed artifact must
    // abort the batch instead of making the affected icon disappear silently.
    for icon_id in &expected_icon_ids {
        ai_repository::resolve_effective_visual_source(connection, collection_id, icon_id)?;
    }

    let mut statement = connection.prepare(
        "SELECT
           i.id,
           i.display_name,
           i.shape,
           i.cell_width_override,
           i.cell_height_override,
           i.transform_quarter_turns,
           evs.effective_source_file_id AS source_file_id,
           i.transform_flip_horizontal,
           i.transform_flip_vertical,
           CASE WHEN i.gif_pingpong = 1 THEN 'pingpong' ELSE i.gif_loop_mode END AS gif_loop_mode,
           i.gif_loop_count,
           i.thumbnail_override_path,
           i.thumbnail_path,
           i.current_preview_path,
           s.original_path_in_library,
           s.original_extension,
           s.is_animated,
           s.width,
           s.height,
           s.sha256,
           COALESCE(s.original_loop_mode, 'preserve') AS source_loop_mode,
           s.original_loop_count,
           cs.crop_x,
           cs.crop_y,
           cs.crop_w,
           cs.crop_h,
           i.text_overlay_enabled,
           i.text_overlay_text,
           i.text_overlay_font_path,
           i.text_overlay_font_size,
           i.text_overlay_x,
           i.text_overlay_y,
           i.text_overlay_color,
           i.text_overlay_stroke_color,
           i.text_overlay_stroke_width,
           er.effects_json AS effect_recipe_json,
           mr.motion_json AS motion_recipe_json
         FROM icons i
         JOIN effective_visual_sources evs ON evs.icon_id = i.id
         JOIN source_files s ON s.id = evs.effective_source_file_id
         JOIN crop_settings cs ON cs.icon_id = i.id
         LEFT JOIN icon_effect_recipes er ON er.icon_id = i.id
         LEFT JOIN icon_motion_recipes mr ON mr.icon_id = i.id
         WHERE i.collection_id = ?1
           AND i.deleted_at IS NULL
           AND i.icon_kind = 'image'
           AND i.readiness = 'complete'
         ORDER BY i.order_index ASC, i.created_at ASC",
    )?;

    let icons = statement
        .query_map(params![collection_id], |row| {
            Ok(IconExportRecord {
                id: row.get("id")?,
                display_name: row.get("display_name")?,
                shape: row.get("shape")?,
                cell_width_override: row.get("cell_width_override")?,
                cell_height_override: row.get("cell_height_override")?,
                transform_quarter_turns: row.get("transform_quarter_turns")?,
                transform_flip_horizontal: row.get::<_, i64>("transform_flip_horizontal")? != 0,
                source_file_id: row.get("source_file_id")?,
                transform_flip_vertical: row.get::<_, i64>("transform_flip_vertical")? != 0,
                gif_loop_mode: row.get("gif_loop_mode")?,
                gif_loop_count: row.get("gif_loop_count")?,
                thumbnail_override_path: row.get("thumbnail_override_path")?,
                thumbnail_path: row.get("thumbnail_path")?,
                current_preview_path: row.get("current_preview_path")?,
                source_path: row.get("original_path_in_library")?,
                source_extension: row.get("original_extension")?,
                source_is_animated: row.get::<_, i64>("is_animated")? != 0,
                source_width: row.get("width")?,
                source_height: row.get("height")?,
                source_hash: row.get("sha256")?,
                source_gif_loop_mode: row.get("source_loop_mode")?,
                source_gif_loop_count: row.get("original_loop_count")?,
                crop_x: row.get("crop_x")?,
                crop_y: row.get("crop_y")?,
                crop_w: row.get("crop_w")?,
                crop_h: row.get("crop_h")?,
                text_overlay_enabled: row.get::<_, i64>("text_overlay_enabled")? != 0,
                text_overlay_text: row.get("text_overlay_text")?,
                text_overlay_font_path: row.get("text_overlay_font_path")?,
                text_overlay_font_size: row.get("text_overlay_font_size")?,
                text_overlay_x: row.get("text_overlay_x")?,
                text_overlay_y: row.get("text_overlay_y")?,
                text_overlay_color: row.get("text_overlay_color")?,
                text_overlay_stroke_color: row.get("text_overlay_stroke_color")?,
                text_overlay_stroke_width: row.get("text_overlay_stroke_width")?,
                effect_recipe_json: row.get("effect_recipe_json")?,
                motion_recipe_json: row.get("motion_recipe_json")?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    if icons.len() != expected_icon_ids.len() {
        return Err(AppError::new(
            "export_visual_state",
            "내보낼 아이콘의 렌더 상태가 불완전합니다. 편집 상태를 복구한 뒤 다시 시도해 주세요.",
        ));
    }

    Ok(icons)
}

fn load_pieces(connection: &Connection, icon_id: &str) -> AppResult<Vec<PieceExportRecord>> {
    let mut statement = connection.prepare(
        "SELECT
           id,
           piece_index,
           piece_role,
           alt_text
         FROM icon_pieces
         WHERE icon_id = ?1
         ORDER BY piece_index ASC",
    )?;

    let pieces = statement
        .query_map(params![icon_id], |row| {
            Ok(PieceExportRecord {
                id: row.get("id")?,
                piece_index: row.get("piece_index")?,
                piece_role: row.get("piece_role")?,
                alt_text: row.get("alt_text")?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(pieces)
}

fn assign_filenames(plan: &mut ExportPlan) -> AppResult<()> {
    let total_count = plan.output_count();
    let padding = total_count.to_string().len().max(3);
    let mut export_index = 1_i64;
    let mut seen_alt_stems = HashSet::new();

    for icon in &mut plan.icons {
        for piece in &mut icon.pieces {
            if !piece.included {
                piece.export_index = 0;
                piece.file_name.clear();
                continue;
            }

            piece.export_index = export_index;
            piece.file_name = match plan.profile.filename_mode.as_str() {
                "alt" => {
                    let stem = sanitized_alt_filename_stem(&piece.alt_text)?;
                    if !seen_alt_stems.insert(stem.to_ascii_lowercase()) {
                        return Err(AppError::new(
                            "validation",
                            "alt 파일명 방식에서 같은 파일명이 만들어집니다.",
                        ));
                    }
                    format!("{stem}.{}", icon.output_format)
                }
                "sequence" => {
                    format!("{export_index:0padding$}.{}", icon.output_format)
                }
                _ => {
                    return Err(AppError::new(
                        "validation",
                        "지원하지 않는 파일명 방식입니다.",
                    ));
                }
            };
            export_index += 1;
        }
    }

    Ok(())
}

fn validate_plan_before_render(plan: &ExportPlan) -> Vec<ExportValidationIssueDto> {
    let mut issues = Vec::new();
    let output_count = plan.output_count();

    if output_count == 0 {
        issues.push(error_issue(
            "empty_collection",
            "내보낼 아이콘 조각이 없습니다.",
            None,
            None,
        ));
    }

    if plan.profile.profile_type == "dcinside" {
        if !(10..=200).contains(&output_count) {
            issues.push(warning_issue(
                "dcinside_count",
                "DCInside 권장 이미지 수는 10개 이상 200개 이하입니다. 내보내기는 계속할 수 있습니다.",
                None,
                None,
            ));
        }

        if plan.profile.target_cell_width != 200 || plan.profile.target_cell_height != 200 {
            issues.push(warning_issue(
                "dcinside_profile_size",
                "DCInside 프로필 기준 크기는 200×200 권장입니다. 현재 설정으로도 내보내기는 진행됩니다.",
                None,
                None,
            ));
        }
    }

    let allowed_formats = normalized_allowed_formats(&plan.profile.allowed_formats);
    let mut alt_to_piece_ids: HashMap<String, Vec<String>> = HashMap::new();
    let mut file_names = HashSet::new();

    for icon in &plan.icons {
        if !icon.pieces.iter().any(|piece| piece.included) {
            continue;
        }

        let expected_roles = match piece_roles(&icon.shape) {
            Ok(roles) => roles,
            Err(error) => {
                issues.push(error_issue(
                    "invalid_shape",
                    error.message,
                    Some(icon.icon_id.clone()),
                    None,
                ));
                continue;
            }
        };

        if icon.pieces.len() != expected_roles.len() {
            issues.push(error_issue(
                "piece_count_mismatch",
                format!(
                    "{} 아이콘의 조각 수가 모양 설정과 일치하지 않습니다.",
                    icon.display_name
                ),
                Some(icon.icon_id.clone()),
                None,
            ));
        }

        if !icon.source_path.exists() {
            issues.push(error_issue(
                "missing_source",
                format!("{} 원본 파일을 찾을 수 없습니다.", icon.display_name),
                Some(icon.icon_id.clone()),
                None,
            ));
        }

        if !allowed_formats.contains(&icon.output_format) {
            let issue = if icon.motion.has_enabled_motion() {
                error_issue(
                    "motion_gif_not_allowed",
                    "모션 효과가 켜진 아이콘은 GIF가 허용된 프로필에서만 내보낼 수 있습니다.",
                    Some(icon.icon_id.clone()),
                    None,
                )
            } else {
                non_blocking_error_issue(
                    "unsupported_format",
                    format!(
                        "{} 형식은 현재 프로필에서 허용되지 않습니다.",
                        icon.output_format
                    ),
                    Some(icon.icon_id.clone()),
                    None,
                )
            };
            issues.push(issue);
        }

        if plan.profile.profile_type == "dcinside"
            && (icon.cell_width != 200 || icon.cell_height != 200)
        {
            issues.push(warning_issue(
                "dcinside_output_size",
                format!(
                    "{} 출력 조각 크기가 {}×{}입니다. DCInside는 200×200을 권장합니다.",
                    icon.display_name, icon.cell_width, icon.cell_height
                ),
                Some(icon.icon_id.clone()),
                None,
            ));
        }

        if icon.crop.width < icon.viewport_width() as f64
            || icon.crop.height < icon.viewport_height() as f64
            || icon.crop.x < 0.0
            || icon.crop.y < 0.0
            || icon.crop.x + icon.crop.width > icon.source_width as f64
            || icon.crop.y + icon.crop.height > icon.source_height as f64
        {
            issues.push(warning_issue(
                "quality_upscale_or_padding",
                format!(
                    "{} 크롭 영역이 출력보다 작거나 원본 밖을 포함해 품질이 낮아질 수 있습니다.",
                    icon.display_name
                ),
                Some(icon.icon_id.clone()),
                None,
            ));
        }

        if icon.output_format == "jpg" {
            issues.push(warning_issue(
                "transparent_background_recommended",
                format!(
                    "{}은 JPG로 내보냅니다. 투명 배경이 필요하면 PNG/GIF가 더 적합합니다.",
                    icon.display_name
                ),
                Some(icon.icon_id.clone()),
                None,
            ));
        }

        for (piece_position, piece) in icon.pieces.iter().enumerate() {
            if !piece.included {
                continue;
            }

            if let Some(expected_role) = expected_roles.get(piece_position) {
                if piece.piece_role != *expected_role {
                    issues.push(error_issue(
                        "piece_role_mismatch",
                        format!(
                            "{} 조각 역할이 현재 모양 설정과 일치하지 않습니다.",
                            icon.display_name
                        ),
                        Some(icon.icon_id.clone()),
                        Some(piece.piece_id.clone()),
                    ));
                }
            }

            if plan.profile.profile_type == "dcinside" {
                if let Err(message) = validate_dcinside_alt(&piece.alt_text) {
                    issues.push(warning_issue(
                        "invalid_alt",
                        message,
                        Some(icon.icon_id.clone()),
                        Some(piece.piece_id.clone()),
                    ));
                }
            }

            if plan.profile.filename_mode == "alt" && piece.alt_text.trim().is_empty() {
                issues.push(error_issue(
                    "empty_alt_filename",
                    "alt 파일명 방식에서는 빈 alt 값을 사용할 수 없습니다.",
                    Some(icon.icon_id.clone()),
                    Some(piece.piece_id.clone()),
                ));
            }

            alt_to_piece_ids
                .entry(piece.alt_text.trim().to_string())
                .or_default()
                .push(piece.piece_id.clone());

            if !file_names.insert(piece.file_name.to_ascii_lowercase()) {
                issues.push(error_issue(
                    "duplicate_filename",
                    format!("{} 파일명이 중복됩니다.", piece.file_name),
                    Some(icon.icon_id.clone()),
                    Some(piece.piece_id.clone()),
                ));
            }
        }
    }

    if plan.profile.profile_type == "dcinside" {
        for (alt_text, piece_ids) in alt_to_piece_ids {
            if !alt_text.is_empty() && piece_ids.len() > 1 {
                for piece_id in piece_ids {
                    issues.push(warning_issue(
                        "duplicate_alt",
                        format!(
                            "alt 값 '{}'이 중복되었습니다. 내보내기는 계속할 수 있습니다.",
                            alt_text
                        ),
                        None,
                        Some(piece_id),
                    ));
                }
            }
        }
    }

    issues
}

fn validate_plan_after_render(
    plan: &ExportPlan,
    existing_issues: &[ExportValidationIssueDto],
) -> Vec<ExportValidationIssueDto> {
    let mut issues = Vec::new();
    let max_bytes = plan.profile.max_bytes.max(1);
    let failed_piece_ids: HashSet<&str> = existing_issues
        .iter()
        .filter(|issue| issue.severity == "error")
        .filter_map(|issue| issue.piece_id.as_deref())
        .collect();

    for icon in &plan.icons {
        for piece in &icon.pieces {
            if !piece.included {
                continue;
            }

            if let Some(byte_size) = piece.byte_size {
                if byte_size > max_bytes {
                    issues.push(non_blocking_error_issue(
                        "max_bytes",
                        format!(
                            "{}이(가) {} 제한을 초과했습니다. 현재 크기: {}",
                            piece.file_name,
                            format_bytes(max_bytes),
                            format_bytes(byte_size),
                        ),
                        Some(icon.icon_id.clone()),
                        Some(piece.piece_id.clone()),
                    ));
                }
            } else if !failed_piece_ids.contains(piece.piece_id.as_str()) {
                issues.push(error_issue(
                    "missing_output",
                    format!("{} 출력 파일을 확인할 수 없습니다.", piece.file_name),
                    Some(icon.icon_id.clone()),
                    Some(piece.piece_id.clone()),
                ));
            }
        }
    }

    issues
}

fn validate_active_variant_sizes(plan: &ExportPlan) -> Vec<ExportValidationIssueDto> {
    let mut issues = Vec::new();
    let max_bytes = plan.profile.max_bytes.max(1);

    for icon in &plan.icons {
        for piece in &icon.pieces {
            if !piece.included || piece.active_variant.is_none() {
                continue;
            }
            if let Some(byte_size) = piece.byte_size {
                if byte_size > max_bytes {
                    issues.push(non_blocking_error_issue(
                        "max_bytes",
                        format!(
                            "{} 최적화 후보가 {} 제한을 초과했습니다. 현재 크기: {}",
                            piece.file_name,
                            format_bytes(max_bytes),
                            format_bytes(byte_size),
                        ),
                        Some(icon.icon_id.clone()),
                        Some(piece.piece_id.clone()),
                    ));
                }
            }
        }
    }

    issues
}

#[derive(Debug, Default)]
struct RenderPlanResult {
    rendered_files: Vec<(String, PathBuf, i64, Option<String>)>,
    issues: Vec<ExportValidationIssueDto>,
}

fn render_plan(plan: &ExportPlan, output_dir: &Path) -> RenderPlanResult {
    let mut result = RenderPlanResult::default();

    for icon in &plan.icons {
        for piece in icon
            .pieces
            .iter()
            .filter(|piece| piece.included && piece.active_variant.is_some())
        {
            if let Some(active_variant) = &piece.active_variant {
                let output_path = output_dir.join(&piece.file_name);
                match copy_active_variant(active_variant, &output_path) {
                    Ok(byte_size) => {
                        result.rendered_files.push((
                            piece.piece_id.clone(),
                            output_path,
                            byte_size,
                            Some(active_variant.id.clone()),
                        ));
                    }
                    Err(error) => {
                        result.issues.push(error_issue(
                            "optimization_variant_failed",
                            format!(
                                "{} optimized variant copy failed: {}",
                                piece.file_name, error.message
                            ),
                            Some(icon.icon_id.clone()),
                            Some(piece.piece_id.clone()),
                        ));
                    }
                }
            }
        }

        let render_pieces: Vec<ExportRenderPiece> = icon
            .pieces
            .iter()
            .filter(|piece| piece.included && piece.active_variant.is_none())
            .map(|piece| ExportRenderPiece {
                piece_index: piece.piece_index,
                file_name: piece.file_name.clone(),
            })
            .collect();

        if render_pieces.is_empty() {
            continue;
        }

        let output_paths = match render_icon_export(ExportRenderRequest {
            source_path: &icon.source_path,
            source_extension: &icon.source_extension,
            shape: &icon.shape,
            crop: icon.crop,
            cell_width: icon.cell_width,
            cell_height: icon.cell_height,
            transform: icon.transform,
            output_format: &icon.output_format,
            resize_filter: &plan.resize_filter,
            gif_loop_mode: &icon.gif_loop_mode,
            gif_loop_count: icon.gif_loop_count,
            source_gif_loop_mode: &icon.source_gif_loop_mode,
            source_gif_loop_count: icon.source_gif_loop_count,
            text_overlay: icon.text_overlay.clone(),
            effects: icon.effects.clone(),
            motion: icon.motion.clone(),
            output_dir,
            pieces: &render_pieces,
        }) {
            Ok(paths) => paths,
            Err(error) => {
                for render_piece in &render_pieces {
                    let _ = fs::remove_file(output_dir.join(&render_piece.file_name));
                }
                for piece in icon
                    .pieces
                    .iter()
                    .filter(|piece| piece.included && piece.active_variant.is_none())
                {
                    result.issues.push(error_issue(
                        "render_failed",
                        format!("{} render failed: {}", piece.file_name, error.message),
                        Some(icon.icon_id.clone()),
                        Some(piece.piece_id.clone()),
                    ));
                }
                continue;
            }
        };

        for (piece, output_path) in icon
            .pieces
            .iter()
            .filter(|piece| piece.included && piece.active_variant.is_none())
            .zip(output_paths)
        {
            match fs::metadata(&output_path) {
                Ok(metadata) => {
                    let byte_size = i64::try_from(metadata.len()).unwrap_or(i64::MAX);
                    result.rendered_files.push((
                        piece.piece_id.clone(),
                        output_path,
                        byte_size,
                        None,
                    ));
                }
                Err(error) => {
                    result.issues.push(error_issue(
                        "write_metadata_failed",
                        format!("{} metadata check failed: {error}", piece.file_name),
                        Some(icon.icon_id.clone()),
                        Some(piece.piece_id.clone()),
                    ));
                }
            }
        }
    }

    result
}

fn render_plan_selected(
    plan: &ExportPlan,
    output_dir: &Path,
    selected_piece_ids: &HashSet<String>,
) -> RenderPlanResult {
    let mut result = RenderPlanResult::default();

    for icon in &plan.icons {
        for piece in icon.pieces.iter().filter(|piece| {
            piece.included
                && selected_piece_ids.contains(&piece.piece_id)
                && piece.active_variant.is_some()
        }) {
            if let Some(active_variant) = &piece.active_variant {
                let output_path = output_dir.join(&piece.file_name);
                match copy_active_variant(active_variant, &output_path) {
                    Ok(byte_size) => {
                        result.rendered_files.push((
                            piece.piece_id.clone(),
                            output_path,
                            byte_size,
                            Some(active_variant.id.clone()),
                        ));
                    }
                    Err(error) => {
                        result.issues.push(error_issue(
                            "optimization_variant_failed",
                            format!(
                                "{} optimized variant copy failed: {}",
                                piece.file_name, error.message
                            ),
                            Some(icon.icon_id.clone()),
                            Some(piece.piece_id.clone()),
                        ));
                    }
                }
            }
        }

        let render_pieces: Vec<ExportRenderPiece> = icon
            .pieces
            .iter()
            .filter(|piece| {
                piece.included
                    && selected_piece_ids.contains(&piece.piece_id)
                    && piece.active_variant.is_none()
            })
            .map(|piece| ExportRenderPiece {
                piece_index: piece.piece_index,
                file_name: piece.file_name.clone(),
            })
            .collect();

        if render_pieces.is_empty() {
            continue;
        }

        let output_paths = match render_icon_export(ExportRenderRequest {
            source_path: &icon.source_path,
            source_extension: &icon.source_extension,
            shape: &icon.shape,
            crop: icon.crop,
            cell_width: icon.cell_width,
            cell_height: icon.cell_height,
            transform: icon.transform,
            output_format: &icon.output_format,
            resize_filter: &plan.resize_filter,
            gif_loop_mode: &icon.gif_loop_mode,
            gif_loop_count: icon.gif_loop_count,
            source_gif_loop_mode: &icon.source_gif_loop_mode,
            source_gif_loop_count: icon.source_gif_loop_count,
            text_overlay: icon.text_overlay.clone(),
            effects: icon.effects.clone(),
            motion: icon.motion.clone(),
            output_dir,
            pieces: &render_pieces,
        }) {
            Ok(paths) => paths,
            Err(error) => {
                for render_piece in &render_pieces {
                    let _ = fs::remove_file(output_dir.join(&render_piece.file_name));
                }
                for piece in icon.pieces.iter().filter(|piece| {
                    piece.included
                        && selected_piece_ids.contains(&piece.piece_id)
                        && piece.active_variant.is_none()
                }) {
                    result.issues.push(error_issue(
                        "render_failed",
                        format!("{} render failed: {}", piece.file_name, error.message),
                        Some(icon.icon_id.clone()),
                        Some(piece.piece_id.clone()),
                    ));
                }
                continue;
            }
        };

        for (piece, output_path) in icon
            .pieces
            .iter()
            .filter(|piece| {
                piece.included
                    && selected_piece_ids.contains(&piece.piece_id)
                    && piece.active_variant.is_none()
            })
            .zip(output_paths)
        {
            match fs::metadata(&output_path) {
                Ok(metadata) => {
                    let byte_size = i64::try_from(metadata.len()).unwrap_or(i64::MAX);
                    result.rendered_files.push((
                        piece.piece_id.clone(),
                        output_path,
                        byte_size,
                        None,
                    ));
                }
                Err(error) => {
                    result.issues.push(error_issue(
                        "write_metadata_failed",
                        format!("{} metadata check failed: {error}", piece.file_name),
                        Some(icon.icon_id.clone()),
                        Some(piece.piece_id.clone()),
                    ));
                }
            }
        }
    }

    result
}

fn replace_rendered_files(
    rendered_files: Vec<(String, PathBuf, i64, Option<String>)>,
    final_files_dir: &Path,
) -> RenderPlanResult {
    let mut result = RenderPlanResult::default();

    for (piece_id, temp_path, _byte_size, variant_id) in rendered_files {
        let Some(file_name) = temp_path.file_name().map(|value| value.to_os_string()) else {
            result.issues.push(error_issue(
                "write_failed",
                "rendered export file path has no filename".to_string(),
                None,
                Some(piece_id),
            ));
            continue;
        };
        let final_path = final_files_dir.join(&file_name);
        let display_name = file_name.to_string_lossy().to_string();

        match replace_file_from_temp(&temp_path, &final_path) {
            Ok(byte_size) => {
                result
                    .rendered_files
                    .push((piece_id, final_path, byte_size, variant_id));
            }
            Err(error) => {
                result.issues.push(error_issue(
                    "write_failed",
                    format!("{display_name} replace failed: {}", error.message),
                    None,
                    Some(piece_id),
                ));
            }
        }
    }

    result
}

fn replace_file_from_temp(temp_path: &Path, final_path: &Path) -> AppResult<i64> {
    if let Some(parent) = final_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let backup_path = if final_path.exists() {
        let file_name = final_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("export-file");
        let backup_path =
            unique_path(final_path.with_file_name(format!(".{file_name}.pmtconcon-backup")))?;
        fs::rename(final_path, &backup_path)?;
        Some(backup_path)
    } else {
        None
    };

    let move_result = fs::rename(temp_path, final_path).or_else(|rename_error| {
        fs::copy(temp_path, final_path).map_err(|copy_error| {
            AppError::new(
                "export_replace_failed",
                format!("rename failed: {rename_error}; copy fallback failed: {copy_error}"),
            )
        })?;
        fs::remove_file(temp_path)?;
        Ok(())
    });

    if let Err(error) = move_result {
        if let Some(backup_path) = &backup_path {
            let _ = fs::remove_file(final_path);
            let _ = fs::rename(backup_path, final_path);
        }
        return Err(error);
    }

    if let Some(backup_path) = backup_path {
        let _ = fs::remove_file(backup_path);
    }

    Ok(i64::try_from(fs::metadata(final_path)?.len()).unwrap_or(i64::MAX))
}

fn hydrate_existing_export_files(plan: &mut ExportPlan, final_files_dir: &Path) {
    for icon in &mut plan.icons {
        for piece in &mut icon.pieces {
            if !piece.included || piece.byte_size.is_some() || piece.file_name.is_empty() {
                continue;
            }

            let output_path = final_files_dir.join(&piece.file_name);
            if let Ok(metadata) = fs::metadata(&output_path) {
                piece.output_path = Some(output_path);
                piece.byte_size = Some(i64::try_from(metadata.len()).unwrap_or(i64::MAX));
            }
        }
    }
}

fn copy_active_variant(variant: &ActiveExportVariant, output_path: &Path) -> AppResult<i64> {
    if !variant.path.is_file() {
        return Err(AppError::not_found(
            "활성 최적화 후보 파일을 찾을 수 없습니다.",
        ));
    }
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    if output_path.exists() {
        fs::remove_file(output_path)?;
    }
    fs::copy(&variant.path, output_path)?;
    Ok(i64::try_from(fs::metadata(output_path)?.len()).unwrap_or(i64::MAX))
}

fn apply_rendered_metadata(
    plan: &mut ExportPlan,
    rendered_files: Vec<(String, PathBuf, i64, Option<String>)>,
) {
    let mut by_piece_id: HashMap<String, (PathBuf, i64, Option<String>)> = rendered_files
        .into_iter()
        .map(|(piece_id, output_path, byte_size, variant_id)| {
            (piece_id, (output_path, byte_size, variant_id))
        })
        .collect();

    for icon in &mut plan.icons {
        for piece in &mut icon.pieces {
            if let Some((output_path, byte_size, variant_id)) = by_piece_id.remove(&piece.piece_id)
            {
                let _unused = AppError::new(
                    "export",
                    "렌더링된 내보내기 파일을 조각에 매핑할 수 없습니다.",
                );
                piece.output_path = Some(output_path);
                piece.byte_size = Some(byte_size);
                if variant_id.is_some() {
                    piece.used_optimized_variant = true;
                }
            }
        }
    }
}

fn validation_result(
    plan: &ExportPlan,
    issues: Vec<ExportValidationIssueDto>,
) -> ExportValidationResultDto {
    let errors = hard_errors(&issues);
    let warnings = soft_warnings(&issues);
    let can_export = plan.output_count() > 0
        && session_blocking_errors(&issues).is_empty()
        && !(plan.profile.strict_warnings && !warnings.is_empty());

    ExportValidationResultDto {
        can_export,
        profile: plan.profile.clone(),
        output_count: plan.output_count(),
        errors,
        warnings,
        items: plan.items(&issues),
    }
}

fn hard_errors(issues: &[ExportValidationIssueDto]) -> Vec<ExportValidationIssueDto> {
    issues
        .iter()
        .filter(|issue| issue.severity == "error")
        .cloned()
        .collect()
}

fn session_blocking_errors(issues: &[ExportValidationIssueDto]) -> Vec<ExportValidationIssueDto> {
    issues
        .iter()
        .filter(|issue| {
            issue.severity == "error"
                && issue.blocking
                && issue.icon_id.is_none()
                && issue.piece_id.is_none()
        })
        .cloned()
        .collect()
}

fn soft_warnings(issues: &[ExportValidationIssueDto]) -> Vec<ExportValidationIssueDto> {
    issues
        .iter()
        .filter(|issue| issue.severity == "warning")
        .cloned()
        .collect()
}

fn write_alts_txt(output_dir: &Path, plan: &ExportPlan) -> AppResult<PathBuf> {
    let path = output_dir.join("alts.txt");
    let mut lines = vec![
        "# PMTCONCON Studio export".to_string(),
        format!("# Collection: {}", plan.collection_name),
        format!("# Profile: {}", plan.profile.name),
    ];

    for item in plan.items(&[]) {
        if !item.included || item.byte_size.is_none() {
            continue;
        }

        lines.push(format!(
            "{}\t{:03}\t{}\t{}\t{}",
            item.file_name, item.export_index, item.piece_id, item.display_name, item.alt_text,
        ));
    }

    fs::write(&path, format!("{}\n", lines.join("\n")))?;
    Ok(path)
}

#[derive(Debug)]
struct ExportReportPaths {
    txt_path: PathBuf,
    json_path: PathBuf,
    issues_csv_path: PathBuf,
}

fn write_export_reports(
    output_dir: &Path,
    plan: &ExportPlan,
    validation: &ExportValidationResultDto,
) -> AppResult<ExportReportPaths> {
    let txt_path = output_dir.join("export_report.txt");
    let json_path = output_dir.join("export_report.json");
    let issues_csv_path = output_dir.join("export_issues.csv");
    let report = build_export_report(plan, validation);

    fs::write(&txt_path, report_text(&report))?;
    let json_bytes = serde_json::to_vec_pretty(&report)
        .map_err(|error| AppError::new("json", error.to_string()))?;
    fs::write(&json_path, json_bytes)?;
    fs::write(&issues_csv_path, report_issues_csv(&report))?;

    Ok(ExportReportPaths {
        txt_path,
        json_path,
        issues_csv_path,
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportReport {
    product: String,
    collection_id: String,
    collection_name: String,
    profile_id: String,
    profile_name: String,
    created_at_unix: u64,
    summary: ExportReportSummary,
    items: Vec<ExportReportItem>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportReportSummary {
    total_items: i64,
    included_items: i64,
    written_items: i64,
    upload_ready_items: i64,
    warning_items: i64,
    not_upload_ready_items: i64,
    failed_items: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportReportItem {
    export_index: i64,
    icon_id: String,
    piece_id: String,
    display_name: String,
    alt: String,
    filename: String,
    status: String,
    byte_size: Option<i64>,
    limit: i64,
    warnings: Vec<String>,
    errors: Vec<String>,
    suggested_fix: String,
    output_path: Option<String>,
    optimized_variant_used: bool,
    optimized_variant_id: Option<String>,
}

fn build_export_report(plan: &ExportPlan, validation: &ExportValidationResultDto) -> ExportReport {
    let issues_by_piece_id = issues_by_piece_id(validation);
    let mut items = Vec::with_capacity(validation.items.len());

    for item in &validation.items {
        let issues = issues_by_piece_id
            .get(item.piece_id.as_str())
            .cloned()
            .unwrap_or_default();
        let warnings = issues
            .iter()
            .filter(|issue| issue.severity == "warning")
            .map(|issue| issue.message.clone())
            .collect::<Vec<_>>();
        let errors = issues
            .iter()
            .filter(|issue| issue.severity == "error")
            .map(|issue| issue.message.clone())
            .collect::<Vec<_>>();

        items.push(ExportReportItem {
            export_index: item.export_index,
            icon_id: item.icon_id.clone(),
            piece_id: item.piece_id.clone(),
            display_name: item.display_name.clone(),
            alt: item.alt_text.clone(),
            filename: item.file_name.clone(),
            status: item.status.clone(),
            byte_size: item.byte_size,
            limit: item.limit_bytes,
            warnings,
            errors,
            suggested_fix: suggested_fix_for_item(item, &issues),
            output_path: item.export_path.clone(),
            optimized_variant_used: plan
                .piece(item.piece_id.as_str())
                .is_some_and(|piece| piece.used_optimized_variant),
            optimized_variant_id: plan
                .piece(item.piece_id.as_str())
                .and_then(|piece| piece.active_variant.as_ref())
                .map(|variant| variant.id.clone()),
        });
    }

    ExportReport {
        product: "PMTCONCON Studio".to_string(),
        collection_id: plan.collection_id.clone(),
        collection_name: plan.collection_name.clone(),
        profile_id: plan.profile.id.clone(),
        profile_name: plan.profile.name.clone(),
        created_at_unix: now_unix_seconds(),
        summary: report_summary(validation),
        items,
    }
}

fn report_summary(validation: &ExportValidationResultDto) -> ExportReportSummary {
    let included_items = validation.items.iter().filter(|item| item.included).count() as i64;
    let written_items = validation
        .items
        .iter()
        .filter(|item| {
            matches!(
                item.status.as_str(),
                "written_ok" | "written_with_warning" | "written_not_upload_ready"
            )
        })
        .count() as i64;
    let upload_ready_items = validation
        .items
        .iter()
        .filter(|item| matches!(item.status.as_str(), "preflight_ok" | "written_ok"))
        .count() as i64;
    let warning_items = validation
        .items
        .iter()
        .filter(|item| {
            matches!(
                item.status.as_str(),
                "preflight_warning" | "written_with_warning"
            )
        })
        .count() as i64;
    let not_upload_ready_items = validation
        .items
        .iter()
        .filter(|item| {
            matches!(
                item.status.as_str(),
                "preflight_not_upload_ready" | "written_not_upload_ready"
            )
        })
        .count() as i64;
    let failed_items = validation
        .items
        .iter()
        .filter(|item| item.status == "failed_to_render")
        .count() as i64;

    ExportReportSummary {
        total_items: validation.items.len() as i64,
        included_items,
        written_items,
        upload_ready_items,
        warning_items,
        not_upload_ready_items,
        failed_items,
    }
}

fn issues_by_piece_id(
    validation: &ExportValidationResultDto,
) -> HashMap<&str, Vec<ExportValidationIssueDto>> {
    let mut map: HashMap<&str, Vec<ExportValidationIssueDto>> = HashMap::new();
    for issue in validation.errors.iter().chain(validation.warnings.iter()) {
        if let Some(piece_id) = issue.piece_id.as_deref() {
            map.entry(piece_id).or_default().push(issue.clone());
        }
    }
    map
}

fn report_text(report: &ExportReport) -> String {
    let mut lines = vec![
        "PMTCONCON Studio export report".to_string(),
        format!("Collection: {}", report.collection_name),
        format!("Profile: {}", report.profile_name),
        format!("Created: {}", report.created_at_unix),
        String::new(),
        format!("Generated: {}", report.summary.written_items),
        format!("Upload ready: {}", report.summary.upload_ready_items),
        format!("Warnings: {}", report.summary.warning_items),
        format!(
            "Not upload-ready: {}",
            report.summary.not_upload_ready_items
        ),
        format!("Failed: {}", report.summary.failed_items),
        String::new(),
    ];

    for item in &report.items {
        if item.status == "excluded" {
            continue;
        }
        lines.push(format!(
            "{:03} {} {} {} bytes={}",
            item.export_index,
            item.filename,
            item.status,
            item.alt,
            item.byte_size
                .map(|size| size.to_string())
                .unwrap_or_else(|| "-".to_string())
        ));
        for warning in &item.warnings {
            lines.push(format!("  warning: {warning}"));
        }
        for error in &item.errors {
            lines.push(format!("  error: {error}"));
        }
        if item.optimized_variant_used {
            lines.push(format!(
                "  optimized_variant: {}",
                item.optimized_variant_id.as_deref().unwrap_or("active")
            ));
        }
        if !item.suggested_fix.is_empty() {
            lines.push(format!("  suggested_fix: {}", item.suggested_fix));
        }
    }

    format!("{}\n", lines.join("\n"))
}

fn report_issues_csv(report: &ExportReport) -> String {
    let mut lines = vec![
        "export_index,filename,piece_id,status,byte_size,limit,warnings,errors,suggested_fix"
            .to_string(),
    ];

    for item in &report.items {
        if item.warnings.is_empty() && item.errors.is_empty() && item.status != "failed_to_render" {
            continue;
        }

        lines.push(
            [
                item.export_index.to_string(),
                item.filename.clone(),
                item.piece_id.clone(),
                item.status.clone(),
                item.byte_size
                    .map(|size| size.to_string())
                    .unwrap_or_default(),
                item.limit.to_string(),
                item.warnings.join(" / "),
                item.errors.join(" / "),
                item.suggested_fix.clone(),
            ]
            .into_iter()
            .map(csv_escape)
            .collect::<Vec<_>>()
            .join(","),
        );
    }

    format!("{}\n", lines.join("\n"))
}

fn csv_escape(value: String) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value
    }
}

fn suggested_fix_for_item(item: &ExportPlanItemDto, issues: &[ExportValidationIssueDto]) -> String {
    if !item.included {
        return "excluded from this export".to_string();
    }
    if issues.iter().any(|issue| issue.code == "max_bytes") && item.output_format == "gif" {
        return "GIF optimization is the next implementation stage.".to_string();
    }
    if issues
        .iter()
        .any(|issue| issue.code == "invalid_alt" || issue.code == "duplicate_alt")
    {
        return "edit alt text".to_string();
    }
    if issues.iter().any(|issue| issue.code == "render_failed") {
        return "open the icon editor and check the source/crop settings".to_string();
    }
    String::new()
}

fn update_export_status(
    connection: &mut Connection,
    plan: &ExportPlan,
    final_dir: &Path,
    validation: &ExportValidationResultDto,
) -> AppResult<()> {
    let transaction = connection.transaction()?;
    let status_by_piece_id = export_status_by_piece_id(validation);

    for icon in &plan.icons {
        for piece in &icon.pieces {
            if !piece.included {
                continue;
            }

            let final_path = final_dir.join("files").join(&piece.file_name);
            let export_status = status_by_piece_id
                .get(piece.piece_id.as_str())
                .map(String::as_str)
                .unwrap_or("ready");
            transaction.execute(
                "UPDATE icon_pieces
                 SET last_export_path = ?1,
                     export_status = ?2,
                     updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE id = ?3",
                params![path_string(&final_path), export_status, piece.piece_id],
            )?;
        }
    }

    transaction.execute(
        "UPDATE app_settings
         SET last_export_directory = ?1,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = 1",
        params![path_string(final_dir)],
    )?;

    transaction.commit()?;
    Ok(())
}

fn apply_final_paths(plan: &mut ExportPlan, final_dir: &Path) {
    let final_files_dir = final_dir.join("files");
    for icon in &mut plan.icons {
        for piece in &mut icon.pieces {
            if piece.included && piece.byte_size.is_some() && !piece.file_name.is_empty() {
                piece.output_path = Some(final_files_dir.join(&piece.file_name));
            }
        }
    }
}

fn export_status_by_piece_id(validation: &ExportValidationResultDto) -> HashMap<&str, String> {
    let mut status_by_piece_id = HashMap::new();

    for warning in &validation.warnings {
        if let Some(piece_id) = warning.piece_id.as_deref() {
            status_by_piece_id
                .entry(piece_id)
                .or_insert_with(|| "warning".to_string());
        }
    }

    for error in &validation.errors {
        if let Some(piece_id) = error.piece_id.as_deref() {
            status_by_piece_id.insert(piece_id, "error".to_string());
        }
    }

    status_by_piece_id
}

fn output_root(paths: &AppPaths, payload: &ExportRequestPayload) -> AppResult<PathBuf> {
    match payload
        .output_directory
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(value) => {
            let path = PathBuf::from(value);
            if !path.is_absolute() {
                return Err(AppError::new(
                    "validation",
                    "출력 폴더는 절대 경로로 입력해야 합니다.",
                ));
            }
            Ok(path)
        }
        None => Ok(paths.exports_dir.clone()),
    }
}

fn unique_export_dir(output_root: &Path, collection_name: &str) -> AppResult<PathBuf> {
    let slug = slugify(collection_name);
    let base = output_root.join(format!("{slug}-{}", now_unix_seconds()));
    unique_path(base)
}

fn unique_child_dir(root: &Path, prefix: &str) -> AppResult<PathBuf> {
    fs::create_dir_all(root)?;
    unique_path(root.join(format!("{prefix}-{}", now_unix_seconds())))
}

fn unique_path(base: PathBuf) -> AppResult<PathBuf> {
    if !base.exists() {
        return Ok(base);
    }

    for suffix in 1..=999 {
        let candidate = base.with_file_name(format!(
            "{}-{suffix}",
            base.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("export")
        ));
        if !candidate.exists() {
            return Ok(candidate);
        }
    }

    Err(AppError::new(
        "export",
        "고유한 내보내기 폴더를 만들 수 없습니다.",
    ))
}

fn finalize_export_directory(temp_dir: &Path, final_dir: &Path) -> AppResult<()> {
    match fs::rename(temp_dir, final_dir) {
        Ok(()) => Ok(()),
        Err(rename_error) => {
            if final_dir.exists() {
                return Err(rename_error.into());
            }

            if let Err(copy_error) = copy_dir_recursive(temp_dir, final_dir) {
                let _ = fs::remove_dir_all(final_dir);
                return Err(AppError::new(
                    "export_finalize_failed",
                    format!(
                        "export files were written but the final folder could not be created. rename failed: {rename_error}; copy fallback failed: {copy_error}"
                    ),
                ));
            }

            let _ = fs::remove_dir_all(temp_dir);
            Ok(())
        }
    }
}

fn copy_dir_recursive(source: &Path, target: &Path) -> std::io::Result<()> {
    fs::create_dir_all(target)?;

    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());

        if source_path.is_dir() {
            copy_dir_recursive(&source_path, &target_path)?;
        } else {
            fs::copy(&source_path, &target_path)?;
        }
    }

    Ok(())
}

fn now_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn slugify(value: &str) -> String {
    let mut output = String::new();

    for character in value.chars().take(48) {
        if character.is_alphanumeric() || matches!(character, '-' | '_') {
            output.push(character);
        } else if !output.ends_with('-') {
            output.push('-');
        }
    }

    let trimmed = output.trim_matches('-');
    if trimmed.is_empty() {
        "collection".to_string()
    } else {
        trimmed.to_string()
    }
}

fn output_format_for_icon(
    profile_format: &str,
    source_extension: &str,
    motion_enabled: bool,
) -> String {
    let source_format = normalize_format(source_extension);
    if source_format == "gif" || motion_enabled {
        return "gif".to_string();
    }

    match normalize_format(profile_format).as_str() {
        "source" => source_format,
        "jpg" => "jpg".to_string(),
        "gif" => "gif".to_string(),
        _ => "png".to_string(),
    }
}

fn active_variant_profile_hash(
    profile: &ExportProfileDto,
    output_format: &str,
    cell_width: i64,
    cell_height: i64,
    resize_filter: &str,
) -> String {
    hash_text(&[
        profile.id.clone(),
        output_format.to_string(),
        profile.max_bytes.to_string(),
        cell_width.to_string(),
        cell_height.to_string(),
        normalize_resize_filter(resize_filter),
    ])
}

fn normalize_resize_filter(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "nearest" => "nearest".to_string(),
        "triangle" | "bilinear" => "triangle".to_string(),
        "catmull_rom" | "bicubic" => "catmull_rom".to_string(),
        "gaussian" => "gaussian".to_string(),
        "lanczos" | "lanczos3" => "lanczos3".to_string(),
        _ => "lanczos3".to_string(),
    }
}

fn normalize_format(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "jpeg" | "jpg" => "jpg".to_string(),
        "gif" => "gif".to_string(),
        "source" => "source".to_string(),
        _ => "png".to_string(),
    }
}

fn normalized_allowed_formats(values: &[String]) -> HashSet<String> {
    values.iter().map(|value| normalize_format(value)).collect()
}

fn validate_dcinside_alt(alt_text: &str) -> Result<(), String> {
    let normalized = alt_text.trim();
    let length = normalized.chars().count();

    if !(1..=3).contains(&length) {
        return Err("alt 값은 한글 기준 1~3글자여야 합니다.".to_string());
    }

    if !normalized.chars().all(is_allowed_alt_character) {
        return Err("한글, 영문, 숫자, * ^ ! ~ + 만 사용할 수 있습니다.".to_string());
    }

    Ok(())
}

fn is_allowed_alt_character(character: char) -> bool {
    character.is_ascii_alphanumeric()
        || matches!(character, '*' | '^' | '!' | '~' | '+')
        || matches!(
            character as u32,
            0x1100..=0x11FF | 0x3130..=0x318F | 0xAC00..=0xD7A3
        )
}

fn sanitized_alt_filename_stem(alt_text: &str) -> AppResult<String> {
    let mut output = String::new();

    for character in alt_text.trim().chars() {
        if is_filename_safe_character(character) {
            output.push(character);
        } else if !output.ends_with('_') {
            output.push('_');
        }
    }

    let trimmed = output.trim_matches([' ', '.', '_']).to_string();
    if trimmed.is_empty() {
        return Err(AppError::new(
            "validation",
            "alt 값으로 안전한 파일명을 만들 수 없습니다.",
        ));
    }

    if is_windows_reserved_name(&trimmed) {
        return Err(AppError::new(
            "validation",
            "Windows 예약어는 파일명으로 사용할 수 없습니다.",
        ));
    }

    Ok(trimmed)
}

fn is_filename_safe_character(character: char) -> bool {
    !character.is_control()
        && !character.is_whitespace()
        && !matches!(
            character,
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
        )
}

fn is_windows_reserved_name(value: &str) -> bool {
    let name = value
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();

    matches!(name.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (name.len() == 4
            && (name.starts_with("COM") || name.starts_with("LPT"))
            && name
                .chars()
                .nth(3)
                .is_some_and(|character| ('1'..='9').contains(&character)))
}

fn error_issue(
    code: impl Into<String>,
    message: impl Into<String>,
    icon_id: Option<String>,
    piece_id: Option<String>,
) -> ExportValidationIssueDto {
    ExportValidationIssueDto {
        severity: "error".to_string(),
        blocking: true,
        code: code.into(),
        message: message.into(),
        icon_id,
        piece_id,
    }
}

fn non_blocking_error_issue(
    code: impl Into<String>,
    message: impl Into<String>,
    icon_id: Option<String>,
    piece_id: Option<String>,
) -> ExportValidationIssueDto {
    ExportValidationIssueDto {
        severity: "error".to_string(),
        blocking: false,
        code: code.into(),
        message: message.into(),
        icon_id,
        piece_id,
    }
}

fn warning_issue(
    code: impl Into<String>,
    message: impl Into<String>,
    icon_id: Option<String>,
    piece_id: Option<String>,
) -> ExportValidationIssueDto {
    ExportValidationIssueDto {
        severity: "warning".to_string(),
        blocking: false,
        code: code.into(),
        message: message.into(),
        icon_id,
        piece_id,
    }
}

fn format_bytes(bytes: i64) -> String {
    if bytes < 1024 {
        return format!("{bytes} B");
    }

    if bytes < 1024 * 1024 {
        return format!("{:.1} KB", bytes as f64 / 1024.0);
    }

    format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

enum OpenMode {
    Folder,
    TextFile,
    Reveal,
}

fn open_path(path: &Path, mode: OpenMode) -> AppResult<()> {
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;

        match mode {
            OpenMode::Folder => {
                Command::new("explorer").arg(path).spawn()?;
            }
            OpenMode::TextFile => {
                Command::new("notepad.exe").arg(path).spawn()?;
            }
            OpenMode::Reveal => {
                Command::new("explorer").arg("/select,").arg(path).spawn()?;
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;

        match mode {
            OpenMode::Reveal => {
                Command::new("open").arg("-R").arg(path).spawn()?;
            }
            OpenMode::Folder | OpenMode::TextFile => {
                Command::new("open").arg(path).spawn()?;
            }
        }
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        use std::process::Command;

        let target = match mode {
            OpenMode::Folder | OpenMode::TextFile => path.to_path_buf(),
            OpenMode::Reveal => path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| path.to_path_buf()),
        };
        Command::new("xdg-open").arg(target).spawn()?;
    }

    Ok(())
}

impl ExportPlan {
    fn output_count(&self) -> i64 {
        self.icons
            .iter()
            .flat_map(|icon| icon.pieces.iter())
            .filter(|piece| piece.included)
            .count() as i64
    }

    fn piece(&self, piece_id: &str) -> Option<&PlannedPiece> {
        self.icons
            .iter()
            .flat_map(|icon| icon.pieces.iter())
            .find(|piece| piece.piece_id == piece_id)
    }

    fn items(&self, issues: &[ExportValidationIssueDto]) -> Vec<ExportPlanItemDto> {
        let max_bytes = self.profile.max_bytes;
        let issues_by_piece_id = issues.iter().fold(
            HashMap::<&str, Vec<&ExportValidationIssueDto>>::new(),
            |mut map, issue| {
                if let Some(piece_id) = issue.piece_id.as_deref() {
                    map.entry(piece_id).or_default().push(issue);
                }
                map
            },
        );

        self.icons
            .iter()
            .flat_map(|icon| {
                let issues_by_piece_id = &issues_by_piece_id;
                icon.pieces.iter().map(move |piece| {
                    let piece_issues = issues_by_piece_id
                        .get(piece.piece_id.as_str())
                        .map(Vec::as_slice)
                        .unwrap_or(&[]);

                    ExportPlanItemDto {
                        export_index: piece.export_index,
                        file_name: piece.file_name.clone(),
                        icon_id: icon.icon_id.clone(),
                        piece_id: piece.piece_id.clone(),
                        piece_role: piece.piece_role.clone(),
                        display_name: icon.display_name.clone(),
                        alt_text: piece.alt_text.clone(),
                        output_format: icon.output_format.clone(),
                        width: icon.cell_width,
                        height: icon.cell_height,
                        byte_size: piece.byte_size,
                        limit_bytes: max_bytes,
                        included: piece.included,
                        is_animated: icon.source_is_animated || icon.output_format == "gif",
                        source_preview_url: icon.source_preview_url.clone(),
                        export_path: piece.output_path.as_ref().map(|path| path_string(path)),
                        status: export_item_status(piece, piece_issues),
                    }
                })
            })
            .collect()
    }
}

fn export_item_status(piece: &PlannedPiece, issues: &[&ExportValidationIssueDto]) -> String {
    if !piece.included {
        return "excluded".to_string();
    }

    let has_error = issues.iter().any(|issue| issue.severity == "error");
    let has_warning = issues.iter().any(|issue| issue.severity == "warning");
    let has_render_failure = issues.iter().any(|issue| {
        issue.severity == "error"
            && matches!(
                issue.code.as_str(),
                "render_failed" | "missing_output" | "write_metadata_failed" | "missing_source"
            )
    });

    if has_render_failure {
        "failed_to_render".to_string()
    } else if piece.byte_size.is_some()
        && piece.active_variant.is_some()
        && !piece.used_optimized_variant
    {
        if has_error {
            "preflight_not_upload_ready".to_string()
        } else if has_warning {
            "preflight_warning".to_string()
        } else {
            "optimized".to_string()
        }
    } else if piece.byte_size.is_some() {
        if has_error {
            "written_not_upload_ready".to_string()
        } else {
            "written_ok".to_string()
        }
    } else if has_error {
        "preflight_not_upload_ready".to_string()
    } else if has_warning {
        "preflight_warning".to_string()
    } else {
        "preflight_ok".to_string()
    }
}

impl PlannedIcon {
    fn viewport_width(&self) -> i64 {
        match self.shape.as_str() {
            "horizontal_double" => self.cell_width * 2,
            _ => self.cell_width,
        }
    }

    fn viewport_height(&self) -> i64 {
        match self.shape.as_str() {
            "vertical_double" => self.cell_height * 2,
            _ => self.cell_height,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};
    use rusqlite::{params, Connection};

    use super::{
        assign_filenames, copy_dir_recursive, export_collection, export_selected_collection_items,
        sanitized_alt_filename_stem, validate_export_collection, ExportPlan, PlannedIcon,
        PlannedPiece,
    };

    use crate::db::migrations;
    use crate::db::repositories::collections::create_collection;
    use crate::db::repositories::editor::apply_icon_crop;
    use crate::db::repositories::export_profiles::list_export_profiles;
    use crate::db::repositories::icons::replace_icon_source;
    use crate::db::repositories::imports::import_image_files;
    use crate::imaging::transform::ImageTransform;
    use crate::models::{ApplyIconCropPayload, ExportRequestPayload, ImportImageFilePayload};
    use crate::paths::AppPaths;

    #[derive(Debug)]
    struct GifSummary {
        frame_sizes: Vec<(u16, u16)>,
        delays: Vec<u16>,
    }

    fn plan(filename_mode: &str, count: usize) -> ExportPlan {
        ExportPlan {
            collection_id: "collection".to_string(),
            collection_name: "테스트".to_string(),
            profile: crate::models::ExportProfileDto {
                id: "profile".to_string(),
                collection_id: "collection".to_string(),
                name: "DCInside".to_string(),
                profile_type: "dcinside".to_string(),
                target_format: "png".to_string(),
                target_cell_width: 200,
                target_cell_height: 200,
                preview_width: 100,
                preview_height: 100,
                max_bytes: 2_097_152,
                allowed_formats: vec!["jpg".to_string(), "png".to_string(), "gif".to_string()],
                filename_mode: filename_mode.to_string(),
                include_alt_txt: true,
                strict_warnings: false,
                created_at: String::new(),
                updated_at: String::new(),
            },
            resize_filter: "lanczos3".to_string(),
            icons: vec![PlannedIcon {
                icon_id: "icon".to_string(),
                display_name: "아이콘".to_string(),
                shape: "single".to_string(),
                source_path: "source.png".into(),
                source_extension: "png".to_string(),
                source_preview_url: None,
                source_is_animated: false,
                source_width: 200,
                source_height: 200,
                source_gif_loop_mode: "once".to_string(),
                source_gif_loop_count: None,
                crop: crate::imaging::export_render::ExportCropRect {
                    x: 0.0,
                    y: 0.0,
                    width: 200.0,
                    height: 200.0,
                },
                cell_width: 200,
                cell_height: 200,
                transform: ImageTransform::new(0, false, false).unwrap(),
                output_format: "png".to_string(),
                gif_loop_mode: "preserve".to_string(),
                gif_loop_count: None,
                text_overlay: None,
                effects: crate::imaging::effects::EffectRecipe::default(),
                motion: crate::imaging::motion::MotionRecipe::default(),
                pieces: (0..count)
                    .map(|index| PlannedPiece {
                        piece_id: format!("piece-{index}"),
                        piece_index: index,
                        piece_role: "single".to_string(),
                        alt_text: format!("가{index}"),
                        included: true,
                        export_index: 0,
                        file_name: String::new(),
                        byte_size: None,
                        output_path: None,
                        active_variant: None,
                        used_optimized_variant: false,
                    })
                    .collect(),
            }],
        }
    }

    #[test]
    fn sequence_filenames_keep_three_digit_padding_for_small_exports() {
        let mut plan = plan("sequence", 9);

        assign_filenames(&mut plan).unwrap();

        assert_eq!(plan.icons[0].pieces[0].file_name, "001.png");
        assert_eq!(plan.icons[0].pieces[8].file_name, "009.png");
    }

    #[test]
    fn alt_filename_mode_sanitizes_unsafe_characters() {
        assert_eq!(sanitized_alt_filename_stem("가*").unwrap(), "가");
        assert!(sanitized_alt_filename_stem("*").is_err());
        assert!(sanitized_alt_filename_stem("CON").is_err());
    }

    #[test]
    fn export_writes_multi_piece_files_alts_and_preserves_original() {
        let mut connection = connection();
        let paths = temp_paths("pmtconcon-export-static");
        let collection =
            create_collection(&mut connection, Some("내보내기 테스트".to_string())).unwrap();
        let source_bytes = png_bytes(40, 20);
        let imported = import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![ImportImageFilePayload {
                original_filename: "source.png".to_string(),
                bytes: source_bytes.clone(),
            }],
        )
        .unwrap();
        let icon_id = imported.imported_icons[0].id.clone();
        let original_path: String = connection
            .query_row(
                "SELECT s.original_path_in_library
                 FROM source_files s
                 JOIN icons i ON i.source_file_id = s.id
                 WHERE i.id = ?1",
                [&icon_id],
                |row| row.get(0),
            )
            .unwrap();

        apply_icon_crop(
            &mut connection,
            &paths,
            &collection.id,
            ApplyIconCropPayload {
                icon_id,
                shape: "horizontal_double".to_string(),
                crop_mode: "fixed".to_string(),
                crop_x: 0.0,
                crop_y: 0.0,
                crop_w: 40.0,
                crop_h: 20.0,
                preset_position: "center".to_string(),
                cell_width: 20,
                cell_height: 20,
                transform_quarter_turns: 0,
                transform_flip_horizontal: false,
                transform_flip_vertical: false,
                piece_ids: Vec::new(),
                gif_loop_mode: "preserve".to_string(),
                gif_loop_count: None,
            },
        )
        .unwrap();

        let custom_profile = custom_profile_id(&connection, &collection.id);
        let result = export_collection(
            &mut connection,
            &paths,
            &collection.id,
            &ExportRequestPayload {
                profile_id: custom_profile,
                target_format: "png".to_string(),
                target_cell_width: 20,
                target_cell_height: 20,
                max_bytes: 10_000_000,
                filename_mode: "sequence".to_string(),
                include_alt_txt: true,
                strict_warnings: false,
                output_directory: Some(
                    paths.root.join("exports-out").to_string_lossy().to_string(),
                ),
                open_folder_after_export: false,
                open_alt_txt_after_export: false,
                excluded_piece_ids: Vec::new(),
                resize_filter: "lanczos3".to_string(),
            },
        )
        .unwrap();

        assert!(result.validation.can_export);
        let export_dir = Path::new(result.export_directory.as_ref().unwrap());
        let first = export_dir.join("files").join("001.png");
        let second = export_dir.join("files").join("002.png");
        assert_eq!(image::image_dimensions(&first).unwrap(), (20, 20));
        assert_eq!(image::image_dimensions(&second).unwrap(), (20, 20));
        let alts = std::fs::read_to_string(result.alt_txt_path.as_ref().unwrap()).unwrap();
        assert!(alts.contains("# PMTCONCON Studio export"));
        assert!(alts.contains("001.png"));
        assert_eq!(std::fs::read(original_path).unwrap(), source_bytes);

        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn selected_reexport_replaces_only_selected_file_in_existing_export_folder() {
        let mut connection = connection();
        let paths = temp_paths("pmtconcon-export-selected-rerun");
        let collection =
            create_collection(&mut connection, Some("selected rerun".to_string())).unwrap();
        let imported = import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![
                ImportImageFilePayload {
                    original_filename: "one.png".to_string(),
                    bytes: png_bytes_with_color(20, 20, Rgba([255, 0, 0, 255])),
                },
                ImportImageFilePayload {
                    original_filename: "two.png".to_string(),
                    bytes: png_bytes_with_color(20, 20, Rgba([0, 255, 0, 255])),
                },
            ],
        )
        .unwrap();
        let second_icon_id = imported.imported_icons[1].id.clone();
        let second_piece_id = imported.imported_icons[1].pieces[0].id.clone();
        let custom_profile = custom_profile_id(&connection, &collection.id);
        let payload = ExportRequestPayload {
            profile_id: custom_profile,
            target_format: "png".to_string(),
            target_cell_width: 20,
            target_cell_height: 20,
            max_bytes: 10_000_000,
            filename_mode: "sequence".to_string(),
            include_alt_txt: true,
            strict_warnings: false,
            output_directory: Some(paths.root.join("exports-out").to_string_lossy().to_string()),
            open_folder_after_export: false,
            open_alt_txt_after_export: false,
            excluded_piece_ids: Vec::new(),
            resize_filter: "lanczos3".to_string(),
        };

        let first_result =
            export_collection(&mut connection, &paths, &collection.id, &payload).unwrap();
        let export_dir = Path::new(first_result.export_directory.as_ref().unwrap()).to_path_buf();
        let first_path = export_dir.join("files").join("001.png");
        let second_path = export_dir.join("files").join("002.png");
        let first_before = std::fs::read(&first_path).unwrap();
        let second_before = std::fs::read(&second_path).unwrap();

        let replacement_bytes = png_bytes_with_color(20, 20, Rgba([0, 0, 255, 255]));
        replace_icon_source(
            &mut connection,
            &paths,
            &collection.id,
            &second_icon_id,
            ImportImageFilePayload {
                original_filename: "replacement-two.png".to_string(),
                bytes: replacement_bytes,
            },
        )
        .unwrap();

        let second_result = export_selected_collection_items(
            &mut connection,
            &paths,
            &collection.id,
            &payload,
            std::slice::from_ref(&second_piece_id),
            export_dir.to_string_lossy().as_ref(),
        )
        .unwrap();

        assert_eq!(
            Path::new(second_result.export_directory.as_ref().unwrap()),
            export_dir.as_path()
        );
        assert_eq!(std::fs::read(&first_path).unwrap(), first_before);
        assert_ne!(std::fs::read(&second_path).unwrap(), second_before);
        assert!(second_result.validation.items.iter().any(|item| {
            item.piece_id == second_piece_id
                && item.file_name == "002.png"
                && item.status == "written_ok"
        }));
        let alts = std::fs::read_to_string(second_result.alt_txt_path.as_ref().unwrap()).unwrap();
        assert!(alts.contains("001.png"));
        assert!(alts.contains("002.png"));

        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn export_preserves_gif_animation_frames_and_delays() {
        let mut connection = connection();
        let paths = temp_paths("pmtconcon-export-gif");
        let collection =
            create_collection(&mut connection, Some("GIF 내보내기".to_string())).unwrap();
        let source_bytes = animated_gif_bytes();
        import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![ImportImageFilePayload {
                original_filename: "source.gif".to_string(),
                bytes: source_bytes,
            }],
        )
        .unwrap();

        let custom_profile = custom_profile_id(&connection, &collection.id);
        let result = export_collection(
            &mut connection,
            &paths,
            &collection.id,
            &ExportRequestPayload {
                profile_id: custom_profile,
                target_format: "png".to_string(),
                target_cell_width: 200,
                target_cell_height: 200,
                max_bytes: 10_000_000,
                filename_mode: "sequence".to_string(),
                include_alt_txt: true,
                strict_warnings: false,
                output_directory: Some(
                    paths.root.join("exports-out").to_string_lossy().to_string(),
                ),
                open_folder_after_export: false,
                open_alt_txt_after_export: false,
                excluded_piece_ids: Vec::new(),
                resize_filter: "lanczos3".to_string(),
            },
        )
        .unwrap();

        assert!(result.validation.can_export);
        let export_dir = Path::new(result.export_directory.as_ref().unwrap());
        let summary = gif_summary(&export_dir.join("files").join("001.gif"));
        assert_eq!(summary.frame_sizes, vec![(200, 200), (200, 200)]);
        assert_eq!(summary.delays, vec![5, 7]);

        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn unchanged_single_gif_export_copies_original_without_reencoding() {
        let mut connection = connection();
        let paths = temp_paths("pmtconcon-export-gif-pass-through");
        let collection =
            create_collection(&mut connection, Some("GIF 원본 유지".to_string())).unwrap();
        let source_bytes = animated_gif_bytes();
        let imported = import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![ImportImageFilePayload {
                original_filename: "same-size.gif".to_string(),
                bytes: source_bytes.clone(),
            }],
        )
        .unwrap();
        let icon_id = imported.imported_icons[0].id.clone();

        apply_icon_crop(
            &mut connection,
            &paths,
            &collection.id,
            ApplyIconCropPayload {
                icon_id,
                shape: "single".to_string(),
                crop_mode: "fixed".to_string(),
                crop_x: 0.0,
                crop_y: 0.0,
                crop_w: 8.0,
                crop_h: 8.0,
                preset_position: "center".to_string(),
                cell_width: 8,
                cell_height: 8,
                transform_quarter_turns: 0,
                transform_flip_horizontal: false,
                transform_flip_vertical: false,
                piece_ids: Vec::new(),
                gif_loop_mode: "preserve".to_string(),
                gif_loop_count: None,
            },
        )
        .unwrap();

        let custom_profile = custom_profile_id(&connection, &collection.id);
        let result = export_collection(
            &mut connection,
            &paths,
            &collection.id,
            &ExportRequestPayload {
                profile_id: custom_profile,
                target_format: "gif".to_string(),
                target_cell_width: 8,
                target_cell_height: 8,
                max_bytes: 10_000_000,
                filename_mode: "sequence".to_string(),
                include_alt_txt: true,
                strict_warnings: false,
                output_directory: Some(
                    paths
                        .root
                        .join("exports-pass-through")
                        .to_string_lossy()
                        .to_string(),
                ),
                open_folder_after_export: false,
                open_alt_txt_after_export: false,
                excluded_piece_ids: Vec::new(),
                resize_filter: "lanczos3".to_string(),
            },
        )
        .unwrap();

        assert!(result.validation.can_export);
        let export_dir = Path::new(result.export_directory.as_ref().unwrap());
        let exported_bytes = std::fs::read(export_dir.join("files").join("001.gif")).unwrap();
        assert_eq!(exported_bytes, source_bytes);

        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn validation_warns_when_jpg_output_may_drop_transparency() {
        let mut connection = connection();
        let paths = temp_paths("pmtconcon-export-jpg-warning");
        let collection = create_collection(&mut connection, Some("JPG 경고".to_string())).unwrap();
        import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![ImportImageFilePayload {
                original_filename: "source.png".to_string(),
                bytes: png_bytes(200, 200),
            }],
        )
        .unwrap();

        let custom_profile = custom_profile_id(&connection, &collection.id);
        let result = validate_export_collection(
            &connection,
            &paths,
            &collection.id,
            &ExportRequestPayload {
                profile_id: custom_profile,
                target_format: "jpg".to_string(),
                target_cell_width: 200,
                target_cell_height: 200,
                max_bytes: 10_000_000,
                filename_mode: "sequence".to_string(),
                include_alt_txt: true,
                strict_warnings: false,
                output_directory: None,
                open_folder_after_export: false,
                open_alt_txt_after_export: false,
                excluded_piece_ids: Vec::new(),
                resize_filter: "lanczos3".to_string(),
            },
        )
        .unwrap();

        assert!(result
            .warnings
            .iter()
            .any(|warning| warning.code == "transparent_background_recommended"));

        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn dcinside_count_and_alt_warnings_do_not_block_sequence_export() {
        let mut connection = connection();
        let paths = temp_paths("pmtconcon-export-warning-only");
        let collection =
            create_collection(&mut connection, Some("DCInside warning export".to_string()))
                .unwrap();
        let imported = import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![ImportImageFilePayload {
                original_filename: "source.png".to_string(),
                bytes: png_bytes(200, 200),
            }],
        )
        .unwrap();
        let piece_id = imported.imported_icons[0].pieces[0].id.clone();
        connection
            .execute(
                "UPDATE icon_pieces SET alt_text = 'abcd' WHERE id = ?1",
                [&piece_id],
            )
            .unwrap();

        let dcinside_profile = list_export_profiles(&connection, &collection.id)
            .unwrap()
            .into_iter()
            .find(|profile| profile.profile_type == "dcinside")
            .unwrap()
            .id;
        let result = export_collection(
            &mut connection,
            &paths,
            &collection.id,
            &ExportRequestPayload {
                profile_id: dcinside_profile,
                target_format: "png".to_string(),
                target_cell_width: 200,
                target_cell_height: 200,
                max_bytes: 10_000_000,
                filename_mode: "sequence".to_string(),
                include_alt_txt: true,
                strict_warnings: false,
                output_directory: Some(
                    paths.root.join("exports-out").to_string_lossy().to_string(),
                ),
                open_folder_after_export: false,
                open_alt_txt_after_export: false,
                excluded_piece_ids: Vec::new(),
                resize_filter: "lanczos3".to_string(),
            },
        )
        .unwrap();

        assert!(result.validation.can_export);
        assert!(result.export_directory.is_some());
        assert!(result
            .validation
            .warnings
            .iter()
            .any(|warning| warning.code == "dcinside_count"));
        assert!(result
            .validation
            .warnings
            .iter()
            .any(|warning| warning.code == "invalid_alt"));

        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn export_skips_working_icons_and_user_excluded_pieces() {
        let mut connection = connection();
        let paths = temp_paths("pmtconcon-export-exclusions");
        let collection =
            create_collection(&mut connection, Some("제외 테스트".to_string())).unwrap();
        let imported = import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![
                ImportImageFilePayload {
                    original_filename: "one.png".to_string(),
                    bytes: png_bytes(200, 200),
                },
                ImportImageFilePayload {
                    original_filename: "two.png".to_string(),
                    bytes: png_bytes(200, 200),
                },
                ImportImageFilePayload {
                    original_filename: "three.png".to_string(),
                    bytes: png_bytes(200, 200),
                },
            ],
        )
        .unwrap();
        let working_icon_id = imported.imported_icons[1].id.clone();
        let excluded_piece_id = imported.imported_icons[2].pieces[0].id.clone();
        connection
            .execute(
                "UPDATE icons SET readiness = 'working' WHERE id = ?1",
                [&working_icon_id],
            )
            .unwrap();

        let custom_profile = custom_profile_id(&connection, &collection.id);
        let result = export_collection(
            &mut connection,
            &paths,
            &collection.id,
            &ExportRequestPayload {
                profile_id: custom_profile,
                target_format: "png".to_string(),
                target_cell_width: 200,
                target_cell_height: 200,
                max_bytes: 10_000_000,
                filename_mode: "sequence".to_string(),
                include_alt_txt: true,
                strict_warnings: false,
                output_directory: Some(
                    paths.root.join("exports-out").to_string_lossy().to_string(),
                ),
                open_folder_after_export: false,
                open_alt_txt_after_export: false,
                excluded_piece_ids: vec![excluded_piece_id],
                resize_filter: "lanczos3".to_string(),
            },
        )
        .unwrap();

        assert!(result.validation.can_export);
        assert_eq!(result.validation.output_count, 1);
        assert_eq!(result.validation.items.len(), 2);
        assert_eq!(
            result
                .validation
                .items
                .iter()
                .filter(|item| item.included)
                .map(|item| item.file_name.as_str())
                .collect::<Vec<_>>(),
            vec!["001.png"]
        );
        assert!(result
            .validation
            .items
            .iter()
            .any(|item| !item.included && item.status == "excluded"));
        let alts = std::fs::read_to_string(result.alt_txt_path.as_ref().unwrap()).unwrap();
        assert!(alts.contains("001.png"));
        assert!(!alts.contains("002.png"));

        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn export_writes_not_upload_ready_file_and_reports_oversized_output() {
        let mut connection = connection();
        let paths = temp_paths("pmtconcon-export-oversized-report");
        let collection =
            create_collection(&mut connection, Some("oversized report".to_string())).unwrap();
        import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![ImportImageFilePayload {
                original_filename: "source.png".to_string(),
                bytes: png_bytes(200, 200),
            }],
        )
        .unwrap();

        let custom_profile = custom_profile_id(&connection, &collection.id);
        let result = export_collection(
            &mut connection,
            &paths,
            &collection.id,
            &ExportRequestPayload {
                profile_id: custom_profile,
                target_format: "png".to_string(),
                target_cell_width: 200,
                target_cell_height: 200,
                max_bytes: 1,
                filename_mode: "sequence".to_string(),
                include_alt_txt: true,
                strict_warnings: false,
                output_directory: Some(
                    paths.root.join("exports-out").to_string_lossy().to_string(),
                ),
                open_folder_after_export: false,
                open_alt_txt_after_export: false,
                excluded_piece_ids: Vec::new(),
                resize_filter: "lanczos3".to_string(),
            },
        )
        .unwrap();

        let export_dir = Path::new(result.export_directory.as_ref().unwrap());
        assert!(export_dir.join("files").join("001.png").is_file());
        assert_eq!(
            result.validation.items[0].status,
            "written_not_upload_ready"
        );
        assert!(result
            .validation
            .errors
            .iter()
            .any(|issue| issue.code == "max_bytes" && !issue.blocking));
        assert!(Path::new(result.report_txt_path.as_ref().unwrap()).is_file());
        assert!(Path::new(result.report_json_path.as_ref().unwrap()).is_file());
        assert!(Path::new(result.issues_csv_path.as_ref().unwrap()).is_file());
        let report = std::fs::read_to_string(result.report_txt_path.as_ref().unwrap()).unwrap();
        assert!(report.contains("001.png"));
        assert!(report.contains("written_not_upload_ready"));

        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn broken_effective_visual_source_aborts_batch_before_render() {
        let mut connection = connection();
        let paths = temp_paths("pmtconcon-export-broken-effective-source");
        let collection =
            create_collection(&mut connection, Some("partial render".to_string())).unwrap();
        let imported = import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![
                ImportImageFilePayload {
                    original_filename: "good.png".to_string(),
                    bytes: png_bytes(200, 200),
                },
                ImportImageFilePayload {
                    original_filename: "missing.png".to_string(),
                    bytes: png_bytes(180, 180),
                },
            ],
        )
        .unwrap();
        let bad_icon_id = imported.imported_icons[1].id.clone();
        let missing_path = paths
            .root
            .join("missing-original.png")
            .to_string_lossy()
            .to_string();
        connection
            .execute(
                "UPDATE source_files
                 SET original_path_in_library = ?1
                 WHERE id = (
                   SELECT source_file_id FROM icons WHERE id = ?2
                 )",
                params![missing_path, bad_icon_id],
            )
            .unwrap();

        let custom_profile = custom_profile_id(&connection, &collection.id);
        let output_directory = paths.root.join("exports-out");
        let error = export_collection(
            &mut connection,
            &paths,
            &collection.id,
            &ExportRequestPayload {
                profile_id: custom_profile,
                target_format: "png".to_string(),
                target_cell_width: 200,
                target_cell_height: 200,
                max_bytes: 10_000_000,
                filename_mode: "sequence".to_string(),
                include_alt_txt: true,
                strict_warnings: false,
                output_directory: Some(output_directory.to_string_lossy().to_string()),
                open_folder_after_export: false,
                open_alt_txt_after_export: false,
                excluded_piece_ids: Vec::new(),
                resize_filter: "lanczos3".to_string(),
            },
        )
        .unwrap_err();

        assert_eq!(error.code, "ai_source_repair_required");
        assert!(!output_directory.exists());

        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn final_export_uses_persisted_non_destructive_transform_recipe() {
        let mut connection = connection();
        let paths = temp_paths("pmtconcon-export-transform");
        let collection =
            create_collection(&mut connection, Some("변형 내보내기".to_string())).unwrap();
        let source_bytes = asymmetric_png_bytes();
        let imported = import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![ImportImageFilePayload {
                original_filename: "asymmetric.png".to_string(),
                bytes: source_bytes.clone(),
            }],
        )
        .unwrap();
        let icon = &imported.imported_icons[0];

        apply_icon_crop(
            &mut connection,
            &paths,
            &collection.id,
            ApplyIconCropPayload {
                icon_id: icon.id.clone(),
                shape: "single".to_string(),
                crop_mode: "fixed".to_string(),
                crop_x: 0.0,
                crop_y: 0.0,
                crop_w: 3.0,
                crop_h: 2.0,
                preset_position: "center".to_string(),
                cell_width: 2,
                cell_height: 3,
                transform_quarter_turns: 1,
                transform_flip_horizontal: false,
                transform_flip_vertical: false,
                piece_ids: vec![icon.pieces[0].id.clone()],
                gif_loop_mode: "preserve".to_string(),
                gif_loop_count: None,
            },
        )
        .unwrap();

        let profile_id = custom_profile_id(&connection, &collection.id);
        let result = export_collection(
            &mut connection,
            &paths,
            &collection.id,
            &ExportRequestPayload {
                profile_id,
                target_format: "png".to_string(),
                target_cell_width: 2,
                target_cell_height: 3,
                max_bytes: 10_000_000,
                filename_mode: "sequence".to_string(),
                include_alt_txt: false,
                strict_warnings: false,
                output_directory: Some(
                    paths
                        .root
                        .join("transformed-export")
                        .to_string_lossy()
                        .to_string(),
                ),
                open_folder_after_export: false,
                open_alt_txt_after_export: false,
                excluded_piece_ids: Vec::new(),
                resize_filter: "nearest".to_string(),
            },
        )
        .unwrap();

        assert!(result.validation.can_export);
        let output_path = Path::new(result.export_directory.as_ref().unwrap())
            .join("files")
            .join("001.png");
        let output = image::open(output_path).unwrap().to_rgba8();
        assert_eq!((output.width(), output.height()), (2, 3));
        let values = output.pixels().map(|pixel| pixel.0[0]).collect::<Vec<_>>();
        assert_eq!(values, vec![3, 0, 4, 1, 5, 2]);

        let original_path: String = connection
            .query_row(
                "SELECT s.original_path_in_library
                 FROM source_files s
                 JOIN icons i ON i.source_file_id = s.id
                 WHERE i.id = ?1",
                [&icon.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(std::fs::read(original_path).unwrap(), source_bytes);

        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn copy_dir_recursive_copies_nested_export_tree() {
        let paths = temp_paths("pmtconcon-export-copy-dir");
        let source = paths.root.join("source");
        let target = paths.root.join("target");
        std::fs::create_dir_all(source.join("files")).unwrap();
        std::fs::write(source.join("alts.txt"), "001.png\talt").unwrap();
        std::fs::write(source.join("files").join("001.png"), [1_u8, 2, 3, 4]).unwrap();

        copy_dir_recursive(&source, &target).unwrap();

        assert_eq!(
            std::fs::read_to_string(target.join("alts.txt")).unwrap(),
            "001.png\talt"
        );
        assert_eq!(
            std::fs::read(target.join("files").join("001.png")).unwrap(),
            vec![1_u8, 2, 3, 4]
        );

        std::fs::remove_dir_all(paths.root).unwrap();
    }

    fn connection() -> Connection {
        let mut connection = Connection::open_in_memory().unwrap();
        migrations::run(&mut connection).unwrap();
        connection
    }

    fn temp_paths(prefix: &str) -> AppPaths {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        AppPaths::prepare(std::env::temp_dir().join(format!("{prefix}-{suffix}"))).unwrap()
    }

    fn custom_profile_id(connection: &Connection, collection_id: &str) -> String {
        list_export_profiles(connection, collection_id)
            .unwrap()
            .into_iter()
            .find(|profile| profile.profile_type == "custom")
            .unwrap()
            .id
    }

    fn png_bytes(width: u32, height: u32) -> Vec<u8> {
        png_bytes_with_color(width, height, Rgba([0, 255, 0, 255]))
    }

    fn asymmetric_png_bytes() -> Vec<u8> {
        let image = ImageBuffer::from_fn(3, 2, |x, y| Rgba([(y * 3 + x) as u8, 0, 0, 255]));
        let mut cursor = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image)
            .write_to(&mut cursor, ImageFormat::Png)
            .unwrap();
        cursor.into_inner()
    }

    fn png_bytes_with_color(width: u32, height: u32, color: Rgba<u8>) -> Vec<u8> {
        let image = ImageBuffer::from_pixel(width, height, color);
        let mut cursor = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image)
            .write_to(&mut cursor, ImageFormat::Png)
            .unwrap();
        cursor.into_inner()
    }

    fn animated_gif_bytes() -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut encoder = gif::Encoder::new(&mut bytes, 8, 8, &[]).unwrap();
            encoder.set_repeat(gif::Repeat::Infinite).unwrap();

            for (color, delay) in [([255, 0, 0, 255], 5_u16), ([0, 0, 255, 255], 7_u16)] {
                let mut pixels = Vec::with_capacity(8 * 8 * 4);
                for _ in 0..(8 * 8) {
                    pixels.extend_from_slice(&color);
                }
                let mut frame = gif::Frame::from_rgba_speed(8, 8, &mut pixels, 10);
                frame.delay = delay;
                encoder.write_frame(&frame).unwrap();
            }
        }

        bytes
    }

    fn gif_summary(path: &Path) -> GifSummary {
        let mut options = gif::DecodeOptions::new();
        options.set_color_output(gif::ColorOutput::RGBA);
        let file = std::fs::File::open(path).unwrap();
        let mut reader = options.read_info(file).unwrap();
        let mut frame_sizes = Vec::new();
        let mut delays = Vec::new();

        while let Some(frame) = reader.read_next_frame().unwrap() {
            frame_sizes.push((frame.width, frame.height));
            delays.push(frame.delay);
        }

        GifSummary {
            frame_sizes,
            delays,
        }
    }
}
