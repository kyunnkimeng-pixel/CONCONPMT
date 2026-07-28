use std::path::Path;

use rusqlite::Connection;

use crate::db::migrations;
use crate::error::{AppError, AppResult};
use crate::paths::AppPaths;

pub fn open_database(database_path: &Path) -> AppResult<Connection> {
    let root = database_path.parent().ok_or_else(|| {
        AppError::new(
            "database_path",
            "데이터베이스의 앱 데이터 경로를 확인할 수 없습니다.",
        )
    })?;
    let paths = AppPaths::prepare(root.to_path_buf())?;
    open_database_with_paths(database_path, &paths)
}

pub fn open_database_with_paths(database_path: &Path, paths: &AppPaths) -> AppResult<Connection> {
    let mut connection = open_existing_database(database_path)?;
    migrations::run(&mut connection)?;
    crate::db::repositories::optimization::reconcile_legacy_variants(&connection, paths)?;
    crate::db::repositories::optimization::reconcile_missing_effective_previews(
        &connection,
        paths,
    )?;
    Ok(connection)
}

pub fn open_existing_database(database_path: &Path) -> AppResult<Connection> {
    let connection = Connection::open(database_path)?;
    connection.pragma_update(None, "foreign_keys", "ON")?;
    connection.pragma_update(None, "journal_mode", "WAL")?;
    connection.busy_timeout(std::time::Duration::from_secs(5))?;
    Ok(connection)
}
