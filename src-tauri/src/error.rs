use std::fmt;

use serde::Serialize;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: String,
    pub message: String,
}

impl AppError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new("not_found", message)
    }

    pub fn lock_failed() -> Self {
        Self::new("state_lock_failed", "저장소에 접근할 수 없습니다.")
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for AppError {}

impl From<std::io::Error> for AppError {
    fn from(error: std::io::Error) -> Self {
        Self::new("io", error.to_string())
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(error: rusqlite::Error) -> Self {
        Self::new("sqlite", error.to_string())
    }
}

impl From<tauri::Error> for AppError {
    fn from(error: tauri::Error) -> Self {
        Self::new("tauri", error.to_string())
    }
}

impl From<image::ImageError> for AppError {
    fn from(error: image::ImageError) -> Self {
        Self::new("image", error.to_string())
    }
}

impl From<gif::DecodingError> for AppError {
    fn from(error: gif::DecodingError) -> Self {
        Self::new("gif", error.to_string())
    }
}
