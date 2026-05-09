use std::fs;
use std::path::{Path, PathBuf};

use crate::error::AppResult;

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub root: PathBuf,
    pub database_path: PathBuf,
    pub originals_dir: PathBuf,
    pub generated_crops_dir: PathBuf,
    pub exports_dir: PathBuf,
    pub thumbnails_dir: PathBuf,
    pub source_file_thumbnails_dir: PathBuf,
    pub previews_dir: PathBuf,
    pub collection_previews_dir: PathBuf,
    pub temp_import_dir: PathBuf,
    pub temp_export_dir: PathBuf,
}

impl AppPaths {
    pub fn prepare(root: PathBuf) -> AppResult<Self> {
        let paths = Self {
            database_path: root.join("library.sqlite"),
            originals_dir: root.join("originals"),
            generated_crops_dir: root.join("generated").join("crops"),
            exports_dir: root.join("exports"),
            thumbnails_dir: root.join("thumbnails"),
            source_file_thumbnails_dir: root.join("thumbnails").join("source-files"),
            previews_dir: root.join("previews"),
            collection_previews_dir: root.join("previews").join("collections"),
            temp_import_dir: root.join("temp").join("import"),
            temp_export_dir: root.join("temp").join("export"),
            root,
        };

        paths.create_directories()?;

        Ok(paths)
    }

    fn create_directories(&self) -> AppResult<()> {
        for directory in self.required_directories() {
            fs::create_dir_all(directory)?;
        }

        Ok(())
    }

    fn required_directories(&self) -> [&Path; 10] {
        [
            &self.root,
            &self.originals_dir,
            &self.generated_crops_dir,
            &self.exports_dir,
            &self.thumbnails_dir,
            &self.source_file_thumbnails_dir,
            &self.previews_dir,
            &self.collection_previews_dir,
            &self.temp_import_dir,
            &self.temp_export_dir,
        ]
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::AppPaths;

    #[test]
    fn prepare_creates_required_directories() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("pmtconcon-paths-{suffix}"));

        let paths = AppPaths::prepare(root.clone()).unwrap();

        assert!(paths.root.is_dir());
        assert!(paths.originals_dir.is_dir());
        assert!(paths.generated_crops_dir.is_dir());
        assert!(paths.exports_dir.is_dir());
        assert!(paths.source_file_thumbnails_dir.is_dir());
        assert!(paths.collection_previews_dir.is_dir());
        assert!(paths.temp_import_dir.is_dir());
        assert!(paths.temp_export_dir.is_dir());

        std::fs::remove_dir_all(root).unwrap();
    }
}
