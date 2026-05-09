use std::sync::{Mutex, MutexGuard};

use rusqlite::Connection;
use tauri::{AppHandle, Manager, Runtime};

use crate::db::connection::open_database;
use crate::error::{AppError, AppResult};
use crate::paths::AppPaths;

pub struct AppState {
    connection: Mutex<Connection>,
    paths: AppPaths,
}

impl AppState {
    pub fn initialize<R: Runtime>(app: &AppHandle<R>) -> AppResult<Self> {
        let app_data_dir = app.path().app_data_dir()?;
        let paths = AppPaths::prepare(app_data_dir)?;
        let connection = open_database(&paths.database_path)?;

        Ok(Self {
            connection: Mutex::new(connection),
            paths,
        })
    }

    pub fn connection(&self) -> AppResult<MutexGuard<'_, Connection>> {
        self.connection.lock().map_err(|_| AppError::lock_failed())
    }

    pub fn paths(&self) -> &AppPaths {
        &self.paths
    }
}
