use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::error::{AppError, AppResult};
use crate::paths::AppPaths;

pub(crate) const NATIVE_FILE_DRAG_SUPPORTED: bool = cfg!(windows);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NativeFileDragOutcome {
    Dropped,
    Cancelled,
}

pub(crate) fn canonical_drag_file(path: &Path) -> AppResult<PathBuf> {
    let canonical = path.canonicalize().map_err(|_| {
        AppError::new(
            "native_drag_file_missing",
            "끌어 놓을 전달 파일을 찾을 수 없습니다. 전달을 다시 준비해 주세요.",
        )
    })?;
    if !canonical.is_file() {
        return Err(AppError::new(
            "native_drag_file_missing",
            "끌어 놓을 전달 파일을 찾을 수 없습니다. 전달을 다시 준비해 주세요.",
        ));
    }
    Ok(canonical)
}

pub(crate) fn canonical_managed_drag_file(paths: &AppPaths, path: &Path) -> AppResult<PathBuf> {
    let relative = path
        .strip_prefix(&paths.root)
        .map_err(|_| unmanaged_drag_path_error())?;
    if relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(unmanaged_drag_path_error());
    }

    let root_metadata =
        fs::symlink_metadata(&paths.root).map_err(|_| unmanaged_drag_path_error())?;
    if !root_metadata.file_type().is_dir() || is_link_or_reparse_point(&root_metadata) {
        return Err(unmanaged_drag_path_error());
    }

    let mut current = paths.root.clone();
    let component_count = relative.components().count();
    for (index, component) in relative.components().enumerate() {
        let Component::Normal(component) = component else {
            return Err(unmanaged_drag_path_error());
        };
        current.push(component);
        let metadata = fs::symlink_metadata(&current).map_err(|_| unmanaged_drag_path_error())?;
        if is_link_or_reparse_point(&metadata) {
            return Err(unmanaged_drag_path_error());
        }
        let is_last = index + 1 == component_count;
        if (is_last && !metadata.file_type().is_file())
            || (!is_last && !metadata.file_type().is_dir())
        {
            return Err(unmanaged_drag_path_error());
        }
    }

    let canonical_root = paths
        .root
        .canonicalize()
        .map_err(|_| unmanaged_drag_path_error())?;
    let canonical = canonical_drag_file(&current).map_err(|_| unmanaged_drag_path_error())?;
    if !canonical.starts_with(&canonical_root) {
        return Err(unmanaged_drag_path_error());
    }
    Ok(canonical)
}

#[cfg(windows)]
pub(crate) fn start_verified_file_drag<R: tauri::Runtime>(
    window: &tauri::Window<R>,
    paths: &AppPaths,
    path: &Path,
) -> AppResult<NativeFileDragOutcome> {
    use std::sync::atomic::{AtomicU8, Ordering};
    use std::sync::Arc;

    let path = canonical_managed_drag_file(paths, path)?;
    let outcome = Arc::new(AtomicU8::new(0));
    let callback_outcome = Arc::clone(&outcome);
    drag::start_drag(
        window,
        drag::DragItem::Files(vec![path.clone()]),
        drag::Image::File(path),
        move |result, _cursor_position| {
            callback_outcome.store(
                match result {
                    drag::DragResult::Dropped => 1,
                    drag::DragResult::Cancel => 2,
                },
                Ordering::Release,
            );
        },
        drag::Options::default(),
    )
    .map_err(|error| {
        AppError::new(
            "native_drag_start_failed",
            format!("파일 끌기를 시작하지 못했습니다. {error}"),
        )
    })?;

    match outcome.load(Ordering::Acquire) {
        1 => Ok(NativeFileDragOutcome::Dropped),
        _ => Ok(NativeFileDragOutcome::Cancelled),
    }
}

#[cfg(not(windows))]
pub(crate) fn start_verified_file_drag<R: tauri::Runtime>(
    _window: &tauri::Window<R>,
    _paths: &AppPaths,
    _path: &Path,
) -> AppResult<NativeFileDragOutcome> {
    Err(AppError::new(
        "native_drag_unsupported",
        "현재 운영체제에서는 앱 밖으로 파일을 직접 끌 수 없습니다. 탐색기에서 파일 선택을 사용해 주세요.",
    ))
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

fn unmanaged_drag_path_error() -> AppError {
    AppError::new(
        "native_drag_unmanaged_path",
        "드래그할 파일이 PMTCONCON Studio의 안전한 관리 경로에 없거나 링크로 바뀌었습니다. 파일을 다시 준비해 주세요.",
    )
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::canonical_drag_file;

    #[test]
    fn canonical_drag_file_rejects_missing_paths() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let missing = std::env::temp_dir().join(format!("pmtcon-drag-missing-{suffix}.png"));
        let error = canonical_drag_file(&missing).unwrap_err();
        assert_eq!(error.code, "native_drag_file_missing");
    }

    #[test]
    fn canonical_drag_file_accepts_files_and_rejects_directories() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory =
            std::env::temp_dir().join(format!("pmtcon-drag-path-{}-{suffix}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let file = directory.join("upload.png");
        fs::write(&file, b"verified-upload").unwrap();

        assert_eq!(
            canonical_drag_file(&file).unwrap(),
            file.canonicalize().unwrap()
        );
        let directory_error = canonical_drag_file(&directory).unwrap_err();
        assert_eq!(directory_error.code, "native_drag_file_missing");

        fs::remove_file(file).unwrap();
        fs::remove_dir(directory).unwrap();
    }
}
