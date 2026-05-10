use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::db::repositories::export_profiles as export_profile_repository;
use crate::error::{AppError, AppResult};
use crate::imaging::export_render::{
    render_icon_export, ExportCropRect, ExportRenderPiece, ExportRenderRequest,
};
use crate::imaging::geometry::piece_roles;
use crate::models::{
    ExportCollectionResultDto, ExportPlanItemDto, ExportProfileDto, ExportRequestPayload,
    ExportValidationIssueDto, ExportValidationResultDto,
};
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
    let mut plan = load_export_plan(connection, collection_id, profile)?;
    let mut issues = validate_plan_before_render(&plan);

    if !hard_errors(&issues).is_empty() {
        return Ok(validation_result(&plan, issues));
    }

    let temp_dir = unique_child_dir(&paths.temp_export_dir, "validation")?;
    fs::create_dir_all(&temp_dir)?;
    let render_result = render_plan(&plan, &temp_dir);

    match render_result {
        Ok(rendered_files) => {
            apply_rendered_metadata(&mut plan, rendered_files)?;
            issues.extend(validate_plan_after_render(&plan));
        }
        Err(error) => issues.push(error_issue("render_failed", error.message, None, None)),
    }

    let _ = fs::remove_dir_all(&temp_dir);

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
    let mut plan = load_export_plan(connection, collection_id, profile)?;
    let mut issues = validate_plan_before_render(&plan);

    if !hard_errors(&issues).is_empty() {
        let validation = validation_result(&plan, issues);
        return Ok(ExportCollectionResultDto {
            validation,
            export_directory: None,
            alt_txt_path: None,
            manifest_path: None,
        });
    }

    let output_root = output_root(paths, payload)?;
    fs::create_dir_all(&output_root)?;
    let temp_dir = unique_child_dir(&output_root, ".pmtconcon-export-temp")?;
    fs::create_dir_all(&temp_dir)?;

    let render_result = render_plan(&plan, &temp_dir);
    match render_result {
        Ok(rendered_files) => {
            apply_rendered_metadata(&mut plan, rendered_files)?;
            issues.extend(validate_plan_after_render(&plan));
        }
        Err(error) => issues.push(error_issue("render_failed", error.message, None, None)),
    }

    let validation = validation_result(&plan, issues);
    if !validation.can_export {
        let _ = fs::remove_dir_all(&temp_dir);
        return Ok(ExportCollectionResultDto {
            validation,
            export_directory: None,
            alt_txt_path: None,
            manifest_path: None,
        });
    }

    let alt_txt_path = if plan.profile.include_alt_txt {
        Some(write_alts_txt(&temp_dir, &plan)?)
    } else {
        None
    };
    let manifest_path = write_manifest(&temp_dir, &plan)?;
    let final_dir = unique_export_dir(&output_root, &plan.collection_name)?;
    fs::rename(&temp_dir, &final_dir)?;

    let final_alt_txt_path = alt_txt_path
        .as_ref()
        .and_then(|path| path.file_name())
        .map(|file_name| final_dir.join(file_name));
    let final_manifest_path = manifest_path
        .file_name()
        .map(|file_name| final_dir.join(file_name))
        .ok_or_else(|| AppError::new("export", "manifest 경로를 만들 수 없습니다."))?;

    update_export_status(connection, &plan, &final_dir)?;

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
        manifest_path: Some(path_string(&final_manifest_path)),
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
    icons: Vec<PlannedIcon>,
}

#[derive(Debug, Clone)]
struct PlannedIcon {
    icon_id: String,
    display_name: String,
    shape: String,
    source_path: PathBuf,
    source_extension: String,
    source_width: i64,
    source_height: i64,
    source_gif_loop_mode: String,
    source_gif_loop_count: Option<i64>,
    crop: ExportCropRect,
    cell_width: i64,
    cell_height: i64,
    output_format: String,
    gif_loop_mode: String,
    gif_loop_count: Option<i64>,
    pieces: Vec<PlannedPiece>,
}

#[derive(Debug, Clone)]
struct PlannedPiece {
    piece_id: String,
    piece_index: usize,
    piece_role: String,
    alt_text: String,
    export_index: i64,
    file_name: String,
    byte_size: Option<i64>,
    output_path: Option<PathBuf>,
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
    cell_width_override: Option<i64>,
    cell_height_override: Option<i64>,
    gif_loop_mode: String,
    gif_loop_count: Option<i64>,
    source_path: String,
    source_extension: String,
    source_width: i64,
    source_height: i64,
    source_gif_loop_mode: String,
    source_gif_loop_count: Option<i64>,
    crop_x: f64,
    crop_y: f64,
    crop_w: f64,
    crop_h: f64,
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
    collection_id: &str,
    profile: ExportProfileDto,
) -> AppResult<ExportPlan> {
    let collection = load_collection(connection, collection_id)?;
    let icon_records = load_icons(connection, collection_id)?;
    let mut icons = Vec::with_capacity(icon_records.len());

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
        let output_format = output_format_for_icon(&profile.target_format, &icon.source_extension);

        icons.push(PlannedIcon {
            icon_id: icon.id,
            display_name: icon.display_name,
            shape: icon.shape,
            source_path: PathBuf::from(icon.source_path),
            source_extension: normalize_format(&icon.source_extension),
            source_width: icon.source_width,
            source_height: icon.source_height,
            source_gif_loop_mode: icon.source_gif_loop_mode,
            source_gif_loop_count: icon.source_gif_loop_count,
            crop: ExportCropRect {
                x: icon.crop_x,
                y: icon.crop_y,
                width: icon.crop_w,
                height: icon.crop_h,
            },
            cell_width,
            cell_height,
            output_format,
            gif_loop_mode: icon.gif_loop_mode,
            gif_loop_count: icon.gif_loop_count,
            pieces: pieces
                .into_iter()
                .map(|piece| PlannedPiece {
                    piece_id: piece.id,
                    piece_index: usize::try_from(piece.piece_index.max(0)).unwrap_or(0),
                    piece_role: piece.piece_role,
                    alt_text: piece.alt_text.trim().to_string(),
                    export_index: 0,
                    file_name: String::new(),
                    byte_size: None,
                    output_path: None,
                })
                .collect(),
        });
    }

    let mut plan = ExportPlan {
        collection_id: collection.id,
        collection_name: collection.name,
        profile,
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
    let mut statement = connection.prepare(
        "SELECT
           i.id,
           i.display_name,
           i.shape,
           i.cell_width_override,
           i.cell_height_override,
           i.gif_loop_mode,
           i.gif_loop_count,
           s.original_path_in_library,
           s.original_extension,
           s.width,
           s.height,
           COALESCE(s.original_loop_mode, 'preserve') AS source_loop_mode,
           s.original_loop_count,
           cs.crop_x,
           cs.crop_y,
           cs.crop_w,
           cs.crop_h
         FROM icons i
         JOIN source_files s ON s.id = i.source_file_id
         JOIN crop_settings cs ON cs.icon_id = i.id
         WHERE i.collection_id = ?1
           AND i.deleted_at IS NULL
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
                gif_loop_mode: row.get("gif_loop_mode")?,
                gif_loop_count: row.get("gif_loop_count")?,
                source_path: row.get("original_path_in_library")?,
                source_extension: row.get("original_extension")?,
                source_width: row.get("width")?,
                source_height: row.get("height")?,
                source_gif_loop_mode: row.get("source_loop_mode")?,
                source_gif_loop_count: row.get("original_loop_count")?,
                crop_x: row.get("crop_x")?,
                crop_y: row.get("crop_y")?,
                crop_w: row.get("crop_w")?,
                crop_h: row.get("crop_h")?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

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
            issues.push(error_issue(
                "dcinside_profile_size",
                "DCInside 프로필 기준 크기는 200×200이어야 합니다.",
                None,
                None,
            ));
        }
    }

    let allowed_formats = normalized_allowed_formats(&plan.profile.allowed_formats);
    let mut alt_to_piece_ids: HashMap<String, Vec<String>> = HashMap::new();
    let mut file_names = HashSet::new();

    for icon in &plan.icons {
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
            issues.push(error_issue(
                "unsupported_format",
                format!(
                    "{} 형식은 현재 프로필에서 허용되지 않습니다.",
                    icon.output_format
                ),
                Some(icon.icon_id.clone()),
                None,
            ));
        }

        if plan.profile.profile_type == "dcinside"
            && (icon.cell_width != 200 || icon.cell_height != 200)
        {
            issues.push(error_issue(
                "dcinside_output_size",
                format!(
                    "{} 출력 조각 크기가 {}×{}입니다. DCInside는 200×200이 필요합니다.",
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
                        format!("alt 값 '{}'이 중복되었습니다. 내보내기는 계속할 수 있습니다.", alt_text),
                        None,
                        Some(piece_id),
                    ));
                }
            }
        }
    }

    issues
}

fn validate_plan_after_render(plan: &ExportPlan) -> Vec<ExportValidationIssueDto> {
    let mut issues = Vec::new();
    let max_bytes = plan.profile.max_bytes.max(1);

    for icon in &plan.icons {
        for piece in &icon.pieces {
            if let Some(byte_size) = piece.byte_size {
                if byte_size > max_bytes {
                    issues.push(error_issue(
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
            } else {
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

fn render_plan(plan: &ExportPlan, output_dir: &Path) -> AppResult<Vec<(String, PathBuf, i64)>> {
    let mut rendered_files = Vec::new();

    for icon in &plan.icons {
        let render_pieces: Vec<ExportRenderPiece> = icon
            .pieces
            .iter()
            .map(|piece| ExportRenderPiece {
                piece_index: piece.piece_index,
                file_name: piece.file_name.clone(),
            })
            .collect();
        let output_paths = render_icon_export(ExportRenderRequest {
            source_path: &icon.source_path,
            source_extension: &icon.source_extension,
            shape: &icon.shape,
            crop: icon.crop,
            cell_width: icon.cell_width,
            cell_height: icon.cell_height,
            output_format: &icon.output_format,
            gif_loop_mode: &icon.gif_loop_mode,
            gif_loop_count: icon.gif_loop_count,
            source_gif_loop_mode: &icon.source_gif_loop_mode,
            source_gif_loop_count: icon.source_gif_loop_count,
            output_dir,
            pieces: &render_pieces,
        })?;

        for (piece, output_path) in icon.pieces.iter().zip(output_paths) {
            let byte_size = i64::try_from(fs::metadata(&output_path)?.len()).unwrap_or(i64::MAX);
            rendered_files.push((piece.piece_id.clone(), output_path, byte_size));
        }
    }

    Ok(rendered_files)
}

fn apply_rendered_metadata(
    plan: &mut ExportPlan,
    rendered_files: Vec<(String, PathBuf, i64)>,
) -> AppResult<()> {
    let mut by_piece_id: HashMap<String, (PathBuf, i64)> = rendered_files
        .into_iter()
        .map(|(piece_id, output_path, byte_size)| (piece_id, (output_path, byte_size)))
        .collect();

    for icon in &mut plan.icons {
        for piece in &mut icon.pieces {
            let (output_path, byte_size) =
                by_piece_id.remove(&piece.piece_id).ok_or_else(|| {
                    AppError::new(
                        "export",
                        "렌더링된 내보내기 파일을 조각에 매핑할 수 없습니다.",
                    )
                })?;
            piece.output_path = Some(output_path);
            piece.byte_size = Some(byte_size);
        }
    }

    Ok(())
}

fn validation_result(
    plan: &ExportPlan,
    issues: Vec<ExportValidationIssueDto>,
) -> ExportValidationResultDto {
    let errors = hard_errors(&issues);
    let warnings = soft_warnings(&issues);
    let can_export = errors.is_empty() && !(plan.profile.strict_warnings && !warnings.is_empty());

    ExportValidationResultDto {
        can_export,
        profile: plan.profile.clone(),
        output_count: plan.output_count(),
        errors,
        warnings,
        items: plan.items(),
    }
}

fn hard_errors(issues: &[ExportValidationIssueDto]) -> Vec<ExportValidationIssueDto> {
    issues
        .iter()
        .filter(|issue| issue.severity == "error")
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

    for item in plan.items() {
        lines.push(format!(
            "{}\t{:03}\t{}\t{}\t{}",
            item.file_name, item.export_index, item.piece_id, item.display_name, item.alt_text,
        ));
    }

    fs::write(&path, format!("{}\n", lines.join("\n")))?;
    Ok(path)
}

fn write_manifest(output_dir: &Path, plan: &ExportPlan) -> AppResult<PathBuf> {
    let path = output_dir.join("export-manifest.json");
    let manifest = ExportManifest {
        product: "PMTCONCON Studio".to_string(),
        collection_id: plan.collection_id.clone(),
        collection_name: plan.collection_name.clone(),
        profile_id: plan.profile.id.clone(),
        profile_name: plan.profile.name.clone(),
        created_at_unix: now_unix_seconds(),
        items: plan.items(),
    };
    let bytes = serde_json::to_vec_pretty(&manifest)
        .map_err(|error| AppError::new("json", error.to_string()))?;
    fs::write(&path, bytes)?;

    Ok(path)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportManifest {
    product: String,
    collection_id: String,
    collection_name: String,
    profile_id: String,
    profile_name: String,
    created_at_unix: u64,
    items: Vec<ExportPlanItemDto>,
}

fn update_export_status(
    connection: &mut Connection,
    plan: &ExportPlan,
    final_dir: &Path,
) -> AppResult<()> {
    let transaction = connection.transaction()?;

    for icon in &plan.icons {
        for piece in &icon.pieces {
            let final_path = final_dir.join(&piece.file_name);
            transaction.execute(
                "UPDATE icon_pieces
                 SET last_export_path = ?1,
                     export_status = 'ready',
                     updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE id = ?2",
                params![path_string(&final_path), piece.piece_id],
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

fn output_format_for_icon(profile_format: &str, source_extension: &str) -> String {
    let source_format = normalize_format(source_extension);
    if source_format == "gif" {
        return "gif".to_string();
    }

    match normalize_format(profile_format).as_str() {
        "source" => source_format,
        "jpg" => "jpg".to_string(),
        "gif" => "gif".to_string(),
        _ => "png".to_string(),
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
        self.icons.iter().map(|icon| icon.pieces.len() as i64).sum()
    }

    fn items(&self) -> Vec<ExportPlanItemDto> {
        self.icons
            .iter()
            .flat_map(|icon| {
                icon.pieces.iter().map(|piece| ExportPlanItemDto {
                    export_index: piece.export_index,
                    file_name: piece.file_name.clone(),
                    icon_id: icon.icon_id.clone(),
                    piece_id: piece.piece_id.clone(),
                    display_name: icon.display_name.clone(),
                    alt_text: piece.alt_text.clone(),
                    output_format: icon.output_format.clone(),
                    width: icon.cell_width,
                    height: icon.cell_height,
                    byte_size: piece.byte_size,
                })
            })
            .collect()
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
    use rusqlite::Connection;

    use super::{
        assign_filenames, export_collection, sanitized_alt_filename_stem,
        validate_export_collection, ExportPlan, PlannedIcon, PlannedPiece,
    };

    use crate::db::migrations;
    use crate::db::repositories::collections::create_collection;
    use crate::db::repositories::editor::apply_icon_crop;
    use crate::db::repositories::export_profiles::list_export_profiles;
    use crate::db::repositories::imports::import_image_files;
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
            icons: vec![PlannedIcon {
                icon_id: "icon".to_string(),
                display_name: "아이콘".to_string(),
                shape: "single".to_string(),
                source_path: "source.png".into(),
                source_extension: "png".to_string(),
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
                output_format: "png".to_string(),
                gif_loop_mode: "preserve".to_string(),
                gif_loop_count: None,
                pieces: (0..count)
                    .map(|index| PlannedPiece {
                        piece_id: format!("piece-{index}"),
                        piece_index: index,
                        piece_role: "single".to_string(),
                        alt_text: format!("가{index}"),
                        export_index: 0,
                        file_name: String::new(),
                        byte_size: None,
                        output_path: None,
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
            },
        )
        .unwrap();

        assert!(result.validation.can_export);
        let export_dir = Path::new(result.export_directory.as_ref().unwrap());
        let first = export_dir.join("001.png");
        let second = export_dir.join("002.png");
        assert_eq!(image::image_dimensions(&first).unwrap(), (20, 20));
        assert_eq!(image::image_dimensions(&second).unwrap(), (20, 20));
        let alts = std::fs::read_to_string(result.alt_txt_path.as_ref().unwrap()).unwrap();
        assert!(alts.contains("# PMTCONCON Studio export"));
        assert!(alts.contains("001.png"));
        assert_eq!(std::fs::read(original_path).unwrap(), source_bytes);

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
            },
        )
        .unwrap();

        assert!(result.validation.can_export);
        let export_dir = Path::new(result.export_directory.as_ref().unwrap());
        let summary = gif_summary(&export_dir.join("001.gif"));
        assert_eq!(summary.frame_sizes, vec![(200, 200), (200, 200)]);
        assert_eq!(summary.delays, vec![5, 7]);

        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn validation_warns_when_jpg_output_may_drop_transparency() {
        let mut connection = connection();
        let paths = temp_paths("pmtconcon-export-jpg-warning");
        let collection =
            create_collection(&mut connection, Some("JPG 경고".to_string())).unwrap();
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
        let image = ImageBuffer::from_pixel(width, height, Rgba([0, 255, 0, 255]));
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
