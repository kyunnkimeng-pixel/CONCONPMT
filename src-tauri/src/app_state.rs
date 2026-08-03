use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use rusqlite::Connection;
use tauri::{AppHandle, Manager, Runtime};

use crate::ai_provider::credentials::AiSessionCredentialStore;
use crate::db::connection::{open_database_with_paths, open_existing_database};
use crate::error::{AppError, AppResult};
use crate::paths::AppPaths;

pub struct AppState {
    connection: Mutex<Connection>,
    paths: AppPaths,
    ai_credentials: AiSessionCredentialStore,
}

impl AppState {
    pub fn initialize<R: Runtime>(app: &AppHandle<R>) -> AppResult<Self> {
        let app_data_dir = app_data_dir(app)?;
        let paths = AppPaths::prepare(app_data_dir)?;
        let connection = open_database_with_paths(&paths.database_path, &paths)?;
        crate::ai_provider::provider::recover_interrupted_session_requests(&connection)?;
        let _ = crate::db::repositories::ai::cleanup_ai_crash_orphans(&connection, &paths);
        let _ = crate::db::repositories::source_files::cleanup_source_file_crash_orphans(
            &connection,
            &paths,
        );
        match crate::db::repositories::ai_handoff::cleanup_ai_web_handoffs(&connection, &paths) {
            Ok(report) if report.deferred > 0 => eprintln!(
                "PMTCONCON Studio: AI 웹 전달 임시 파일 {}건의 정리가 지연됐습니다.",
                report.deferred
            ),
            Err(_) => {
                eprintln!("PMTCONCON Studio: AI 웹 전달 임시 파일 정리를 시작하지 못했습니다.")
            }
            _ => {}
        }

        Ok(Self {
            connection: Mutex::new(connection),
            paths,
            ai_credentials: AiSessionCredentialStore::default(),
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

    pub fn start_ai_handoff_maintenance_worker(&self) {
        let paths = self.paths.clone();
        let database_path = self.paths.database_path.clone();
        let _ = std::thread::Builder::new()
            .name("pmtcon-ai-handoff-maintenance".to_string())
            .spawn(move || loop {
                std::thread::sleep(Duration::from_secs(15 * 60));
                let connection = match open_existing_database(&database_path) {
                    Ok(connection) => connection,
                    Err(_) => {
                        eprintln!(
                            "PMTCONCON Studio: AI 웹 전달 주기 정리용 저장소를 열지 못했습니다."
                        );
                        continue;
                    }
                };
                match crate::db::repositories::ai_handoff::run_ai_web_handoff_maintenance(
                    &connection,
                    &paths,
                ) {
                    Ok(report) if report.deferred_count > 0 => eprintln!(
                        "PMTCONCON Studio: AI 웹 전달 임시 파일 {}건의 주기 정리가 지연됐습니다.",
                        report.deferred_count
                    ),
                    Err(_) => eprintln!(
                        "PMTCONCON Studio: AI 웹 전달 임시 파일 주기 정리를 완료하지 못했습니다."
                    ),
                    _ => {}
                }
            });
    }

    pub fn ai_credentials(&self) -> &AiSessionCredentialStore {
        &self.ai_credentials
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
