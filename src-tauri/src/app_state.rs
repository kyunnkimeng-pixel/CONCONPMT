use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

use rusqlite::Connection;
use tauri::{AppHandle, Manager, Runtime};

use crate::db::connection::{open_database_with_paths, open_existing_database};
use crate::error::{AppError, AppResult};
use crate::paths::AppPaths;

pub struct AppState {
    connection: Mutex<Connection>,
    paths: AppPaths,
}

impl AppState {
    pub fn initialize<R: Runtime>(app: &AppHandle<R>) -> AppResult<Self> {
        let app_data_dir = app_data_dir(app)?;
        let paths = AppPaths::prepare(app_data_dir)?;
        let connection = open_database_with_paths(&paths.database_path, &paths)?;
        let _ = crate::db::repositories::ai::cleanup_ai_crash_orphans(&connection, &paths);

        Ok(Self {
            connection: Mutex::new(connection),
            paths,
        })
    }

    pub fn connection(&self) -> AppResult<MutexGuard<'_, Connection>> {
        self.connection.lock().map_err(|_| AppError::lock_failed())
    }

    pub fn render_connection(&self) -> AppResult<Connection> {
        open_existing_database(&self.paths.database_path)
    }

    pub fn paths(&self) -> &AppPaths {
        &self.paths
    }
}

fn app_data_dir<R: Runtime>(app: &AppHandle<R>) -> AppResult<PathBuf> {
    if std::env::var("PMTCONCON_ENABLE_APP_DATA_OVERRIDE")
        .ok()
        .as_deref()
        == Some("1")
    {
        if let Some(path) = std::env::var_os("PMTCONCON_APP_DATA_DIR") {
            let path = PathBuf::from(path);
            if !path.as_os_str().is_empty() {
                return Ok(path);
            }
        }
    }
    Ok(app.path().app_data_dir()?)
}
