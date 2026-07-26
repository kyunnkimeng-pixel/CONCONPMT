use std::path::Path;

use rusqlite::Connection;

use crate::db::migrations;
use crate::error::AppResult;

pub fn open_database(database_path: &Path) -> AppResult<Connection> {
    let mut connection = open_existing_database(database_path)?;
    migrations::run(&mut connection)?;
    Ok(connection)
}

pub fn open_existing_database(database_path: &Path) -> AppResult<Connection> {
    let connection = Connection::open(database_path)?;
    connection.pragma_update(None, "foreign_keys", "ON")?;
    connection.pragma_update(None, "journal_mode", "WAL")?;
    connection.busy_timeout(std::time::Duration::from_secs(5))?;
    Ok(connection)
}
