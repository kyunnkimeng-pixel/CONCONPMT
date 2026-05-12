pub mod exporter;
pub mod gif_frames;
pub mod grid;
pub mod importer;
pub mod manifest;
pub mod preview;
pub mod reimport;
pub mod slices;

use std::fs;
use std::path::{Path, PathBuf};

use image::ImageFormat;

use crate::error::{AppError, AppResult};
use crate::models::ImportImageFilePayload;

#[derive(Debug, Clone)]
pub(crate) struct SheetImageInput {
    pub original_filename: String,
    pub bytes: Vec<u8>,
    pub extension: String,
}

pub(crate) fn read_sheet_image_input(
    sheet_path: Option<&str>,
    sheet_file: Option<&ImportImageFilePayload>,
    allow_gif: bool,
) -> AppResult<SheetImageInput> {
    match (sheet_path, sheet_file) {
        (_, Some(file)) => {
            let extension = normalized_extension(&file.original_filename, allow_gif)?;
            Ok(SheetImageInput {
                original_filename: file.original_filename.clone(),
                bytes: file.bytes.clone(),
                extension,
            })
        }
        (Some(path), None) if !path.trim().is_empty() => {
            let source_path = PathBuf::from(path.trim());
            let filename = source_path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| AppError::new("validation", "시트 파일 이름을 확인할 수 없습니다."))?
                .to_string();
            let extension = normalized_extension(&filename, allow_gif)?;
            let bytes = fs::read(&source_path)?;
            Ok(SheetImageInput {
                original_filename: filename,
                bytes,
                extension,
            })
        }
        _ => Err(AppError::new(
            "validation",
            "시트 이미지 파일 또는 파일 경로가 필요합니다.",
        )),
    }
}

pub(crate) fn normalized_extension(filename: &str, allow_gif: bool) -> AppResult<String> {
    let extension = Path::new(filename)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::trim)
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| AppError::new("validation", "시트 파일 확장자를 확인할 수 없습니다."))?;

    match extension.as_str() {
        "png" | "jpg" | "jpeg" => Ok(extension),
        "gif" if allow_gif => Ok(extension),
        "gif" => Err(AppError::new(
            "validation",
            "정적 시트 가져오기는 GIF를 받지 않습니다. GIF는 프레임 시트 도구를 사용해야 합니다.",
        )),
        _ => Err(AppError::new(
            "validation",
            "시트는 PNG, JPG, JPEG 파일만 사용할 수 있습니다.",
        )),
    }
}

pub(crate) fn image_format_for_extension(extension: &str) -> AppResult<ImageFormat> {
    match extension {
        "png" => Ok(ImageFormat::Png),
        "jpg" | "jpeg" => Ok(ImageFormat::Jpeg),
        "gif" => Ok(ImageFormat::Gif),
        _ => Err(AppError::new(
            "validation",
            "지원하지 않는 시트 이미지 형식입니다.",
        )),
    }
}

pub(crate) fn path_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}
