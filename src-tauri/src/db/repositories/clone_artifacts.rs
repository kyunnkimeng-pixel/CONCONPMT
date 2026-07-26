use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use rusqlite::Transaction;

use crate::db::repositories::optimization as optimization_repository;
use crate::error::{AppError, AppResult};
use crate::ids::create_id;
use crate::imaging::import_limits::MAX_IMPORT_FILE_BYTES;
use crate::optimization::analyzer;
use crate::paths::AppPaths;

const CLONED_PREVIEW_DIRECTORY: &str = "cloned";
const CLONED_VARIANT_DIRECTORY: &str = "cloned";

pub(crate) fn clone_current_preview(
    paths: &AppPaths,
    collection_id: &str,
    icon_id: &str,
    source_path: Option<&str>,
) -> AppResult<Option<String>> {
    clone_preview_file(paths, collection_id, icon_id, source_path, "preview")
}

pub(crate) fn clone_piece_preview(
    paths: &AppPaths,
    collection_id: &str,
    icon_id: &str,
    piece_index: i64,
    source_path: Option<&str>,
) -> AppResult<Option<String>> {
    clone_preview_file(
        paths,
        collection_id,
        icon_id,
        source_path,
        &format!("piece-{piece_index:02}"),
    )
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
        if source_target.source_hash != variant.source_hash
            || source_target.crop_hash != variant.crop_hash
            || source_target.profile_hash != variant.profile_hash
            || source_target.output_format != variant_format
            || !Path::new(&variant.path).is_file()
        {
            continue;
        }
        if !cloned_groups.insert((source_profile_id.to_string(), source_piece_id.to_string())) {
            continue;
        }

        let target = analyzer::load_target(
            transaction,
            target_icon_id,
            target_profile_id,
            Some(target_piece_id),
        )?;
        if target.output_format != variant_format {
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

        optimization_repository::insert_variant(
            transaction,
            &optimization_repository::NewProcessedAssetVariant {
                id: target_variant_id.clone(),
                icon_id: target_icon_id.to_string(),
                piece_id: Some(target_piece_id.clone()),
                profile_id: Some(target_profile_id.to_string()),
                source_file_id: Some(target.source_file_id.clone()),
                kind: variant.kind,
                preset: variant.preset,
                path: target_path,
                format: variant_format,
                width: variant.width,
                height: variant.height,
                byte_size: variant.byte_size,
                frame_count: variant.frame_count,
                duration_ms: variant.duration_ms,
                loop_mode: variant.loop_mode,
                settings_json: variant.settings_json,
                source_hash: target.source_hash,
                crop_hash: target.crop_hash,
                profile_hash: target.profile_hash,
                settings_hash: variant.settings_hash,
            },
        )?;
        optimization_repository::set_active_variant(transaction, &target_variant_id)?;
    }

    Ok(())
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
}

pub(crate) fn cleanup_cloned_collection_previews(paths: &AppPaths, collection_id: &str) {
    let Ok(collection_component) = safe_component(collection_id, "모음 ID") else {
        return;
    };
    let preview_target = paths.collection_previews_dir.join(collection_component);
    remove_directory_if_present(&paths.collection_previews_dir, &preview_target);
    let variant_target = paths
        .processed_variants_dir
        .join(CLONED_VARIANT_DIRECTORY)
        .join(collection_component);
    remove_directory_if_present(&paths.processed_variants_dir, &variant_target);
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
fn clone_preview_file(
    paths: &AppPaths,
    collection_id: &str,
    icon_id: &str,
    source_path: Option<&str>,
    target_stem: &str,
) -> AppResult<Option<String>> {
    let Some(source_path) = source_path.filter(|path| !path.trim().is_empty()) else {
        return Ok(None);
    };

    let source = PathBuf::from(source_path);
    let canonical_source = source.canonicalize().map_err(|error| {
        AppError::new(
            "preview_clone_failed",
            format!("복제할 미리보기 파일을 열 수 없습니다: {error}"),
        )
    })?;
    let canonical_root = paths.root.canonicalize()?;
    if !canonical_source.starts_with(&canonical_root) || !canonical_source.is_file() {
        return Err(AppError::new(
            "preview_clone_failed",
            "앱 라이브러리 밖의 미리보기 파일은 복제할 수 없습니다.",
        ));
    }

    let extension = canonical_source
        .extension()
        .and_then(|extension| extension.to_str())
        .filter(|extension| {
            !extension.is_empty()
                && extension.len() <= 10
                && extension
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric())
        })
        .ok_or_else(|| {
            AppError::new(
                "preview_clone_failed",
                "복제할 미리보기 파일의 확장자를 확인할 수 없습니다.",
            )
        })?;

    let target_directory =
        icon_preview_root(paths, collection_id, icon_id)?.join(CLONED_PREVIEW_DIRECTORY);
    prepare_target_directory(&paths.collection_previews_dir, &target_directory)?;
    let target = target_directory.join(format!("{target_stem}.{}", extension.to_ascii_lowercase()));
    if target.exists() {
        return Err(AppError::new(
            "preview_clone_failed",
            "복제 미리보기 대상 경로가 이미 존재합니다.",
        ));
    }

    fs::copy(&canonical_source, &target).map_err(|error| {
        AppError::new(
            "preview_clone_failed",
            format!("미리보기 파일을 복제하지 못했습니다: {error}"),
        )
    })?;

    Ok(Some(target.to_string_lossy().to_string()))
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
