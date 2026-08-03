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
    let root_metadata =
        fs::symlink_metadata(&paths.root).map_err(|_| unmanaged_drag_path_error())?;
    if !root_metadata.file_type().is_dir() || is_link_or_reparse_point(&root_metadata) {
        return Err(unmanaged_drag_path_error());
    }

    let canonical_root = paths
        .root
        .canonicalize()
        .map_err(|_| unmanaged_drag_path_error())?;
    let (walk_root, relative) = if let Ok(relative) = path.strip_prefix(&paths.root) {
        (paths.root.as_path(), relative)
    } else if let Ok(relative) = path.strip_prefix(&canonical_root) {
        (canonical_root.as_path(), relative)
    } else {
        return Err(unmanaged_drag_path_error());
    };
    if relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(unmanaged_drag_path_error());
    }

    let mut current = walk_root.to_path_buf();
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
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::paths::AppPaths;

    use super::{canonical_drag_file, canonical_managed_drag_file};

    struct ManagedDragFixture {
        base: PathBuf,
        paths: AppPaths,
    }

    impl ManagedDragFixture {
        fn new(label: &str) -> Self {
            let suffix = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let base = std::env::temp_dir().join(format!(
                "pmtcon-managed-drag-{label}-{}-{suffix}",
                std::process::id()
            ));
            let actual_root = base.join("managed-root");
            fs::create_dir_all(&actual_root).unwrap();

            #[cfg(windows)]
            let configured_root = actual_root.with_file_name("MANAGED-ROOT");
            #[cfg(not(windows))]
            let configured_root = actual_root;

            let paths = AppPaths::prepare(configured_root).unwrap();
            Self { base, paths }
        }

        fn managed_file(&self, name: &str) -> PathBuf {
            let directory = self.paths.ai_handoffs_dir.join("request");
            fs::create_dir_all(&directory).unwrap();
            let file = directory.join(name);
            fs::write(&file, b"verified-upload").unwrap();
            file
        }
    }

    impl Drop for ManagedDragFixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.base);
        }
    }

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

    #[test]
    fn canonical_managed_drag_file_accepts_raw_and_canonical_managed_paths() {
        let fixture = ManagedDragFixture::new("aliases");
        let raw_file = fixture.managed_file("upload.png");
        let canonical_file = raw_file.canonicalize().unwrap();

        #[cfg(windows)]
        assert!(
            canonical_file.strip_prefix(&fixture.paths.root).is_err(),
            "the regression requires a canonical Windows path that is not a lexical child of the raw root"
        );

        assert_eq!(
            canonical_managed_drag_file(&fixture.paths, &raw_file).unwrap(),
            canonical_file
        );
        assert_eq!(
            canonical_managed_drag_file(&fixture.paths, &canonical_file).unwrap(),
            canonical_file
        );
    }

    #[test]
    fn canonical_managed_drag_file_rejects_outside_missing_and_directory_paths() {
        let fixture = ManagedDragFixture::new("invalid-targets");
        let outside_file = fixture.base.join("outside.png");
        fs::write(&outside_file, b"outside").unwrap();
        let managed_directory = fixture.paths.ai_handoffs_dir.join("request-directory");
        fs::create_dir_all(&managed_directory).unwrap();
        let missing_file = fixture.paths.ai_handoffs_dir.join("missing.png");

        for invalid_path in [
            outside_file,
            managed_directory.clone(),
            managed_directory.canonicalize().unwrap(),
            missing_file,
        ] {
            let error = canonical_managed_drag_file(&fixture.paths, &invalid_path).unwrap_err();
            assert_eq!(error.code, "native_drag_unmanaged_path");
        }
    }

    #[test]
    fn canonical_managed_drag_file_rejects_linked_descendants_for_raw_and_canonical_roots() {
        let fixture = ManagedDragFixture::new("linked-descendant");
        let outside_directory = fixture.base.join("outside-directory");
        fs::create_dir_all(&outside_directory).unwrap();
        fs::write(outside_directory.join("upload.png"), b"outside").unwrap();
        let linked_directory = fixture.paths.root.join("linked-directory");

        let link_result = create_directory_symlink(&outside_directory, &linked_directory);
        if let Err(error) = link_result {
            eprintln!("symlink/reparse assertion skipped: {error}");
            return;
        }

        let raw_linked_file = linked_directory.join("upload.png");
        let canonical_root_linked_file = fixture
            .paths
            .root
            .canonicalize()
            .unwrap()
            .join("linked-directory")
            .join("upload.png");
        let resolved_outside_file = raw_linked_file.canonicalize().unwrap();

        for invalid_path in [
            raw_linked_file,
            canonical_root_linked_file,
            resolved_outside_file,
        ] {
            let error = canonical_managed_drag_file(&fixture.paths, &invalid_path).unwrap_err();
            assert_eq!(error.code, "native_drag_unmanaged_path");
        }
    }

    #[cfg(windows)]
    fn create_directory_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::windows::fs::symlink_dir(target, link)
    }

    #[cfg(unix)]
    fn create_directory_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::unix::fs::symlink(target, link)
    }

    #[cfg(not(any(windows, unix)))]
    fn create_directory_symlink(_target: &Path, _link: &Path) -> std::io::Result<()> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "directory symlinks are unsupported on this target",
        ))
    }
}
