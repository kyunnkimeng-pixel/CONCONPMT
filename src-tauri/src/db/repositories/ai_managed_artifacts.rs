use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::error::{AppError, AppResult};

const PATH_ERROR_CODE: &str = "ai_managed_artifact_path";

pub(crate) fn prepare_owned_directory(
    app_root: &Path,
    allowed_root: &Path,
    target: &Path,
) -> AppResult<PathBuf> {
    canonical_owned_directory(app_root, allowed_root, target, true)?.ok_or_else(|| {
        AppError::new(
            PATH_ERROR_CODE,
            "AI 미리보기 관리 경로를 준비할 수 없습니다.",
        )
    })
}

pub(crate) fn promote_owned_directory(
    app_root: &Path,
    staging_root: &Path,
    previews_root: &Path,
    staging_dir: &Path,
    final_dir: &Path,
) -> AppResult<PathBuf> {
    let canonical_staging = canonical_owned_directory(app_root, staging_root, staging_dir, false)?
        .ok_or_else(|| {
            AppError::new(
                PATH_ERROR_CODE,
                "AI 미리보기 staging 경로를 찾을 수 없습니다.",
            )
        })?;
    let final_parent = final_dir.parent().ok_or_else(|| {
        AppError::new(
            PATH_ERROR_CODE,
            "AI 미리보기의 상위 경로를 확인할 수 없습니다.",
        )
    })?;
    let canonical_final_parent = prepare_owned_directory(app_root, previews_root, final_parent)?;
    let final_name = final_dir.file_name().ok_or_else(|| {
        AppError::new(
            PATH_ERROR_CODE,
            "AI 미리보기 경로의 이름을 확인할 수 없습니다.",
        )
    })?;
    let canonical_final = canonical_final_parent.join(final_name);
    ensure_path_absent(&canonical_final)?;
    fs::rename(&canonical_staging, &canonical_final)?;
    canonical_owned_directory(app_root, previews_root, final_dir, false)?.ok_or_else(|| {
        AppError::new(
            PATH_ERROR_CODE,
            "승격한 AI 미리보기 경로를 확인할 수 없습니다.",
        )
    })?;
    Ok(final_dir.to_path_buf())
}

pub(crate) fn remove_owned_directory_if_present(
    app_root: &Path,
    allowed_root: &Path,
    target: &Path,
) -> AppResult<()> {
    let Some(target) = canonical_owned_directory(app_root, allowed_root, target, false)? else {
        return Ok(());
    };
    let allowed_root = canonical_owned_directory(app_root, allowed_root, allowed_root, false)?
        .ok_or_else(|| {
            AppError::new(PATH_ERROR_CODE, "AI 미리보기 관리 루트를 찾을 수 없습니다.")
        })?;
    if target == allowed_root {
        return Err(AppError::new(
            PATH_ERROR_CODE,
            "AI 미리보기 관리 루트 자체는 정리할 수 없습니다.",
        ));
    }
    fs::remove_dir_all(target)?;
    Ok(())
}

fn canonical_owned_directory(
    app_root: &Path,
    allowed_root: &Path,
    target: &Path,
    create_missing: bool,
) -> AppResult<Option<PathBuf>> {
    let allowed_relative = allowed_root.strip_prefix(app_root).map_err(|_| {
        AppError::new(
            PATH_ERROR_CODE,
            "AI 미리보기 관리 루트가 앱 데이터 루트 밖에 있습니다.",
        )
    })?;
    validate_relative_components(allowed_relative)?;
    let target_relative_to_allowed = target.strip_prefix(allowed_root).map_err(|_| {
        AppError::new(
            PATH_ERROR_CODE,
            "AI 미리보기 경로가 허용된 관리 루트 밖에 있습니다.",
        )
    })?;
    validate_relative_components(target_relative_to_allowed)?;
    let target_relative = target.strip_prefix(app_root).map_err(|_| {
        AppError::new(
            PATH_ERROR_CODE,
            "AI 미리보기 경로가 앱 데이터 루트 밖에 있습니다.",
        )
    })?;
    validate_relative_components(target_relative)?;

    if !fs::metadata(app_root)?.is_dir() {
        return Err(AppError::new(
            PATH_ERROR_CODE,
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
                PATH_ERROR_CODE,
                "AI 미리보기 경로에 안전하지 않은 구성 요소가 있습니다.",
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
            PATH_ERROR_CODE,
            "정규화된 AI 미리보기 경로가 관리 루트 밖에 있습니다.",
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
            PATH_ERROR_CODE,
            "AI 미리보기 경로에 안전하지 않은 구성 요소가 있습니다.",
        ))
    }
}

fn validate_owned_component(
    path: &Path,
    expected: &Path,
    metadata: &fs::Metadata,
) -> AppResult<()> {
    if !metadata.file_type().is_dir() || is_link_or_reparse_point(metadata) {
        return Err(AppError::new(
            PATH_ERROR_CODE,
            "AI 미리보기 경로에 링크 또는 파일이 포함되어 있습니다.",
        ));
    }
    let canonical = path.canonicalize()?;
    if canonical != expected {
        return Err(AppError::new(
            PATH_ERROR_CODE,
            "AI 미리보기 경로의 실제 위치가 관리 경로와 다릅니다.",
        ));
    }
    Ok(())
}

fn is_link_or_reparse_point(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return true;
        }
    }
    false
}

fn ensure_path_absent(path: &Path) -> AppResult<()> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
        Ok(_) => Err(AppError::new(
            PATH_ERROR_CODE,
            "AI 미리보기 경로가 이미 존재합니다.",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_roots(label: &str) -> (PathBuf, PathBuf, PathBuf) {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "pmtconcon-ai-managed-{label}-{}-{suffix}",
            std::process::id()
        ));
        let staging = root.join("ai").join("staging").join("activations");
        let previews = root.join("previews").join("ai-activations");
        fs::create_dir_all(&staging).unwrap();
        fs::create_dir_all(&previews).unwrap();
        (root, staging, previews)
    }

    fn try_create_directory_link(target: &Path, link: &Path) -> bool {
        #[cfg(windows)]
        {
            std::os::windows::fs::symlink_dir(target, link).is_ok()
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(target, link).is_ok()
        }
        #[cfg(not(any(windows, unix)))]
        {
            let _ = (target, link);
            false
        }
    }

    fn remove_directory_link(link: &Path) {
        #[cfg(windows)]
        {
            let _ = fs::remove_dir(link);
        }
        #[cfg(unix)]
        {
            let _ = fs::remove_file(link);
        }
    }

    #[test]
    fn promotes_only_between_owned_roots() {
        let (root, staging_root, previews_root) = test_roots("promote");
        let staging = staging_root.join("operation");
        let prepared_staging = prepare_owned_directory(&root, &staging_root, &staging).unwrap();
        fs::write(prepared_staging.join("preview.png"), b"preview").unwrap();
        let final_dir = previews_root
            .join("collection")
            .join("icon")
            .join("operation");

        let promoted =
            promote_owned_directory(&root, &staging_root, &previews_root, &staging, &final_dir)
                .unwrap();

        assert!(promoted.join("preview.png").is_file());
        assert!(!prepared_staging.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_linked_preview_parent_without_external_writes() {
        let (root, staging_root, previews_root) = test_roots("linked-parent");
        let outside = std::env::temp_dir().join(format!(
            "pmtconcon-ai-managed-outside-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&outside).unwrap();
        let linked_collection = previews_root.join("collection");
        if !try_create_directory_link(&outside, &linked_collection) {
            fs::remove_dir_all(root).unwrap();
            fs::remove_dir_all(outside).unwrap();
            return;
        }
        let staging = staging_root.join("operation");
        let prepared_staging = prepare_owned_directory(&root, &staging_root, &staging).unwrap();
        fs::write(prepared_staging.join("preview.png"), b"preview").unwrap();
        let final_dir = linked_collection.join("icon").join("operation");

        let error =
            promote_owned_directory(&root, &staging_root, &previews_root, &staging, &final_dir)
                .unwrap_err();

        assert_eq!(error.code, PATH_ERROR_CODE);
        assert!(staging.join("preview.png").is_file());
        assert!(!outside.join("icon").exists());
        remove_directory_link(&linked_collection);
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(outside).unwrap();
    }
}
