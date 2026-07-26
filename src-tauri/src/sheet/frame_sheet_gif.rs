use std::collections::{HashMap, HashSet};
use std::fmt::Write as FmtWrite;
use std::fs;
use std::io::{self, Write as IoWrite};
use std::path::{Path, PathBuf};

use image::codecs::gif::{GifEncoder, Repeat as ImageGifRepeat};
use image::{Delay, Frame, RgbaImage};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::db::repositories::icons as icon_repository;
use crate::db::repositories::source_files::{
    import_source_file_from_bytes, SourceFileImportOptions, StoredSourceFile,
};
use crate::error::{AppError, AppResult};
use crate::ids::create_id;
use crate::imaging::gif_pipeline::{
    output_repeat_for_settings, pingpong_sequence, GifOutputRepeat,
};
use crate::imaging::import_limits::{
    decode_import_image, validate_gif_workload, MAX_GIF_FRAMES, MAX_IMPORT_FILE_BYTES,
};
use crate::models::{IconDto, ImportImageFilePayload};
use crate::paths::AppPaths;

use super::grid::{
    alpha_warning_for_extension, analyze_rgba_grid, resolve_grid, SheetCell, SheetGridSettings,
};
use super::importer::{crop_cell, preserve_original_sheet};
use super::{image_format_for_extension, path_string, read_sheet_image_input};

const RECIPE_SCHEMA: &str = "pmtcon-frame-sheet-gif-v1";
const MAX_ANALYZED_GRID_CELLS: i64 = 10_000;
const MIN_FRAME_DURATION_MS: i64 = 10;
const MAX_FRAME_DURATION_MS: i64 = 60_000;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameSheetGifRequest {
    pub sheet_path: Option<String>,
    pub sheet_file: Option<ImportImageFilePayload>,
    pub target_collection_id: String,
    pub grid_settings: SheetGridSettings,
    pub frames: Vec<FrameSheetGifFrameInput>,
    pub direction: String,
    pub loop_mode: String,
    pub loop_count: Option<i64>,
    pub display_name: String,
    pub expected_render_hash: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FrameSheetGifFrameInput {
    pub source_cell_index: i64,
    pub duration_ms: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameSheetGifMeasurement {
    pub preview_path: String,
    pub render_hash: String,
    pub byte_size: i64,
    pub max_bytes: i64,
    pub passes_byte_limit: bool,
    pub source_frame_count: i64,
    pub generated_frame_count: i64,
    pub duration_ms: i64,
    pub width: i64,
    pub height: i64,
    pub normalized_frame_durations_ms: Vec<i64>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameSheetGifCreateResult {
    pub icon: IconDto,
    pub measurement: FrameSheetGifMeasurement,
    pub preserved_sheet_path: String,
    pub recipe_id: String,
}

#[derive(Debug)]
struct RenderedFrameSheetGif {
    original_sheet_filename: String,
    original_sheet_extension: String,
    original_sheet_bytes: Vec<u8>,
    original_sheet_sha256: String,
    display_name: String,
    normalized_source_frames: Vec<FrameSheetGifFrameInput>,
    normalized_generated_durations_ms: Vec<i64>,
    direction: String,
    loop_mode: String,
    loop_count: Option<i64>,
    width: i64,
    height: i64,
    duration_ms: i64,
    gif_bytes: Vec<u8>,
    render_hash: String,
    warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
struct CollectionMeasurementSettings {
    max_bytes: i64,
}

struct LimitedBuffer {
    bytes: Vec<u8>,
    max_bytes: usize,
    exceeded: bool,
}

impl LimitedBuffer {
    fn new(max_bytes: usize) -> Self {
        Self {
            bytes: Vec::new(),
            max_bytes,
            exceeded: false,
        }
    }
}

impl IoWrite for LimitedBuffer {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let Some(next_len) = self.bytes.len().checked_add(buffer.len()) else {
            self.exceeded = true;
            return Err(io::Error::other("generated GIF size overflow"));
        };
        if next_len > self.max_bytes {
            self.exceeded = true;
            return Err(io::Error::other("generated GIF exceeds import limit"));
        }
        self.bytes.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub fn measure_frame_sheet_gif(
    connection: &Connection,
    paths: &AppPaths,
    request: FrameSheetGifRequest,
) -> AppResult<FrameSheetGifMeasurement> {
    let collection =
        load_collection_measurement_settings(connection, request.target_collection_id.trim())?;
    let rendered = render_frame_sheet_gif(&request)?;
    measurement_from_rendered(paths, &rendered, collection.max_bytes)
}

pub fn create_frame_sheet_gif(
    connection: &mut Connection,
    paths: &AppPaths,
    request: FrameSheetGifRequest,
) -> AppResult<FrameSheetGifCreateResult> {
    let collection_id = request.target_collection_id.trim().to_string();
    let collection = load_collection_measurement_settings(connection, &collection_id)?;
    let rendered = render_frame_sheet_gif(&request)?;
    verify_expected_render_hash(
        request.expected_render_hash.as_deref(),
        &rendered.render_hash,
    )?;
    let measurement = measurement_from_rendered(paths, &rendered, collection.max_bytes)?;

    let preserved_sheet = preserve_original_sheet(
        paths,
        &rendered.original_sheet_filename,
        &rendered.original_sheet_extension,
        &rendered.original_sheet_bytes,
    )?;

    let generated_filename = generated_gif_filename(&rendered.display_name);
    let generated_file = ImportImageFilePayload {
        original_filename: generated_filename,
        bytes: rendered.gif_bytes.clone(),
    };
    let recipe_id = create_id("frame_sheet_gif_recipe");
    let icon_id = create_id("icon");
    let piece_id = create_id("piece");
    let crop_id = create_id("crop");
    let grid_settings_json = serialize_json(&request.grid_settings)?;
    let frames_json = serialize_json(&rendered.normalized_source_frames)?;

    let transaction = connection.transaction()?;
    ensure_collection_exists(&transaction, &collection_id)?;
    let next_order_index = next_icon_order_index(&transaction, &collection_id)?;
    let source_file = import_source_file_from_bytes(
        &transaction,
        paths,
        &generated_file,
        SourceFileImportOptions {
            allow_gif: true,
            exact_dimensions: None,
        },
    )?;

    insert_generated_icon(
        &transaction,
        &icon_id,
        &collection_id,
        &source_file,
        &rendered.display_name,
        next_order_index,
        rendered.width,
        rendered.height,
    )?;
    insert_full_crop(
        &transaction,
        &crop_id,
        &icon_id,
        rendered.width,
        rendered.height,
    )?;
    insert_single_piece(&transaction, &piece_id, &icon_id)?;
    set_collection_cover_if_empty(&transaction, &collection_id, &icon_id, &source_file.id)?;
    insert_recipe(
        &transaction,
        &recipe_id,
        &icon_id,
        &rendered,
        &preserved_sheet,
        &grid_settings_json,
        &frames_json,
        measurement.byte_size,
    )?;
    transaction.commit()?;

    let icon = icon_repository::get_icon(connection, &collection_id, &icon_id)?;
    Ok(FrameSheetGifCreateResult {
        icon,
        measurement,
        preserved_sheet_path: path_string(&preserved_sheet),
        recipe_id,
    })
}

fn render_frame_sheet_gif(request: &FrameSheetGifRequest) -> AppResult<RenderedFrameSheetGif> {
    let collection_id = request.target_collection_id.trim();
    if collection_id.is_empty() {
        return Err(AppError::new(
            "validation",
            "GIF를 추가할 모음이 필요합니다.",
        ));
    }
    let display_name = normalized_display_name(&request.display_name)?;
    let direction = normalized_direction(&request.direction)?;
    let (repeat, loop_mode, loop_count) =
        normalized_repeat(&request.loop_mode, request.loop_count)?;

    let source = read_sheet_image_input(
        request.sheet_path.as_deref(),
        request.sheet_file.as_ref(),
        false,
    )?;
    let format = image_format_for_extension(&source.extension)?;
    let sheet = decode_import_image(&source.bytes, format)?.to_rgba8();
    let sheet_width = i64::from(sheet.width());
    let sheet_height = i64::from(sheet.height());

    let resolved = resolve_grid(&request.grid_settings, sheet_width, sheet_height)?;
    validate_grid_cell_count(resolved.rows, resolved.columns)?;
    let analysis = analyze_rgba_grid(&sheet, &request.grid_settings, sheet_width, sheet_height)?;

    let cells = analysis
        .cells
        .iter()
        .cloned()
        .map(|cell| (cell.index, cell))
        .collect::<HashMap<_, _>>();
    validate_requested_frame_count(request.frames.len(), &direction, &loop_mode)?;
    let normalized_source_frames = normalize_source_frames(&request.frames, &cells)?;
    if normalized_source_frames.len() < 2 {
        return Err(AppError::new(
            "validation",
            "GIF를 만들려면 프레임을 2개 이상 포함해야 합니다.",
        ));
    }

    let mut generated_frames = normalized_source_frames.clone();
    match direction.as_str() {
        "forward" => {}
        "reverse" => generated_frames.reverse(),
        "pingpong" => {
            pingpong_sequence(&mut generated_frames);
            if loop_mode == "once" {
                generated_frames.push(normalized_source_frames[0].clone());
            }
        }
        _ => unreachable!("direction is normalized before frame generation"),
    }

    let generated_frame_count = i64::try_from(generated_frames.len())
        .map_err(|_| AppError::new("validation", "생성할 GIF 프레임 수가 너무 큽니다."))?;
    let width = u32::try_from(resolved.cell_width)
        .map_err(|_| AppError::new("validation", "GIF 프레임 너비가 올바르지 않습니다."))?;
    let height = u32::try_from(resolved.cell_height)
        .map_err(|_| AppError::new("validation", "GIF 프레임 높이가 올바르지 않습니다."))?;
    validate_gif_workload(width, height, generated_frame_count)
        .map_err(|message| AppError::new("validation", message))?;

    let normalized_generated_durations_ms = generated_frames
        .iter()
        .map(|frame| frame.duration_ms)
        .collect::<Vec<_>>();
    let duration_ms = normalized_generated_durations_ms
        .iter()
        .try_fold(0_i64, |total, duration| total.checked_add(*duration))
        .ok_or_else(|| AppError::new("validation", "GIF 총 재생시간이 너무 깁니다."))?;
    let gif_bytes = encode_gif_frames(&sheet, &cells, &generated_frames, repeat)?;
    let render_hash = sha256_hex(&gif_bytes);

    let mut warnings = analysis.warnings;
    if let Some(warning) = alpha_warning_for_extension(&source.extension) {
        warnings.push(warning.to_string());
    }
    let selected_indexes = normalized_source_frames
        .iter()
        .map(|frame| frame.source_cell_index)
        .collect::<HashSet<_>>();
    let empty_selected_count = selected_indexes
        .iter()
        .filter(|index| cells.get(index).is_some_and(|cell| cell.empty_candidate))
        .count();
    if empty_selected_count > 0 {
        warnings.push(format!(
            "투명하거나 빈 셀 후보 {empty_selected_count}개가 GIF 프레임에 포함되었습니다."
        ));
    }
    let (has_partial_alpha, exceeds_gif_palette) =
        selected_pixel_characteristics(&sheet, &cells, &selected_indexes);
    if has_partial_alpha {
        warnings.push(
            "선택 프레임에 부분 투명도(alpha 1~254)가 있습니다. GIF의 1비트 투명도로 변환되면서 가장자리가 달라질 수 있습니다."
                .to_string(),
        );
    }
    if exceeds_gif_palette {
        warnings.push(
            "선택 프레임에 256색을 초과하는 색상이 있습니다. GIF 팔레트 양자화로 일부 색이 달라질 수 있습니다."
                .to_string(),
        );
    }

    Ok(RenderedFrameSheetGif {
        original_sheet_filename: source.original_filename,
        original_sheet_extension: source.extension,
        original_sheet_sha256: sha256_hex(&source.bytes),
        original_sheet_bytes: source.bytes,
        display_name,
        normalized_source_frames,
        normalized_generated_durations_ms,
        direction,
        loop_mode,
        loop_count,
        width: i64::from(width),
        height: i64::from(height),
        duration_ms,
        gif_bytes,
        render_hash,
        warnings,
    })
}

fn encode_gif_frames(
    sheet: &RgbaImage,
    cells: &HashMap<i64, SheetCell>,
    frames: &[FrameSheetGifFrameInput],
    repeat: GifOutputRepeat,
) -> AppResult<Vec<u8>> {
    let mut writer = LimitedBuffer::new(MAX_IMPORT_FILE_BYTES);
    let encode_result = {
        let mut encoder = GifEncoder::new(&mut writer);
        let repeat_result = match repeat {
            GifOutputRepeat::Once => Ok(()),
            GifOutputRepeat::Infinite => encoder.set_repeat(ImageGifRepeat::Infinite),
            GifOutputRepeat::Finite(count) => encoder.set_repeat(ImageGifRepeat::Finite(count)),
        };

        repeat_result.and_then(|_| {
            for frame in frames {
                let cell = cells.get(&frame.source_cell_index).ok_or_else(|| {
                    image::ImageError::IoError(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "frame cell disappeared during GIF encoding",
                    ))
                })?;
                let cropped = crop_cell(sheet, cell);
                encoder.encode_frame(Frame::from_parts(
                    cropped,
                    0,
                    0,
                    Delay::from_numer_denom_ms(frame.duration_ms as u32, 1),
                ))?;
            }
            Ok(())
        })
    };

    if writer.exceeded {
        return Err(AppError::new(
            "validation",
            "생성 GIF가 앱의 64MB 가져오기 한도를 초과합니다.",
        ));
    }
    encode_result.map_err(AppError::from)?;
    Ok(writer.bytes)
}

fn normalize_source_frames(
    requested: &[FrameSheetGifFrameInput],
    cells: &HashMap<i64, SheetCell>,
) -> AppResult<Vec<FrameSheetGifFrameInput>> {
    if requested.is_empty() {
        return Err(AppError::new(
            "validation",
            "GIF에 포함할 프레임이 없습니다.",
        ));
    }

    requested
        .iter()
        .map(|frame| {
            let cell = cells.get(&frame.source_cell_index).ok_or_else(|| {
                AppError::new(
                    "validation",
                    format!(
                        "시트 셀 {}을(를) 찾을 수 없습니다.",
                        frame.source_cell_index
                    ),
                )
            })?;
            if cell.out_of_bounds {
                return Err(AppError::new(
                    "validation",
                    format!(
                        "시트 셀 {}이(가) 이미지 바깥으로 나갑니다.",
                        frame.source_cell_index
                    ),
                ));
            }
            Ok(FrameSheetGifFrameInput {
                source_cell_index: frame.source_cell_index,
                duration_ms: quantize_duration_ms(frame.duration_ms)?,
            })
        })
        .collect()
}

fn validate_requested_frame_count(
    source_frame_count: usize,
    direction: &str,
    loop_mode: &str,
) -> AppResult<()> {
    let source_frame_count = i64::try_from(source_frame_count)
        .map_err(|_| AppError::new("validation", "요청한 GIF 프레임 수가 너무 큽니다."))?;
    if source_frame_count > MAX_GIF_FRAMES {
        return Err(AppError::new(
            "validation",
            format!("GIF는 최대 {MAX_GIF_FRAMES}프레임까지 만들 수 있습니다."),
        ));
    }

    let generated_frame_count = if direction == "pingpong" && source_frame_count >= 2 {
        source_frame_count
            .checked_mul(2)
            .and_then(|count| count.checked_sub(if loop_mode == "once" { 1 } else { 2 }))
            .ok_or_else(|| {
                AppError::new("validation", "핑퐁 GIF의 최종 프레임 수가 너무 큽니다.")
            })?
    } else {
        source_frame_count
    };
    if generated_frame_count > MAX_GIF_FRAMES {
        return Err(AppError::new(
            "validation",
            format!("생성 방향을 적용한 최종 GIF는 최대 {MAX_GIF_FRAMES}프레임이어야 합니다."),
        ));
    }
    Ok(())
}

fn quantize_duration_ms(duration_ms: i64) -> AppResult<i64> {
    if !(1..=MAX_FRAME_DURATION_MS).contains(&duration_ms) {
        return Err(AppError::new(
            "validation",
            format!("프레임 재생시간은 1ms 이상 {MAX_FRAME_DURATION_MS}ms 이하여야 합니다."),
        ));
    }
    Ok(((duration_ms + 5) / 10 * 10).max(MIN_FRAME_DURATION_MS))
}

fn normalized_direction(direction: &str) -> AppResult<String> {
    match direction.trim().to_ascii_lowercase().as_str() {
        "forward" => Ok("forward".to_string()),
        "reverse" => Ok("reverse".to_string()),
        "pingpong" => Ok("pingpong".to_string()),
        _ => Err(AppError::new(
            "validation",
            "GIF 생성 방향은 forward, reverse, pingpong 중 하나여야 합니다.",
        )),
    }
}

fn normalized_repeat(
    loop_mode: &str,
    loop_count: Option<i64>,
) -> AppResult<(GifOutputRepeat, String, Option<i64>)> {
    let loop_mode = loop_mode.trim().to_ascii_lowercase();
    let normalized_count = match loop_mode.as_str() {
        "once" | "infinite" => None,
        "count" => {
            let count = loop_count.unwrap_or(1);
            if !(1..=i64::from(u16::MAX)).contains(&count) {
                return Err(AppError::new(
                    "validation",
                    format!(
                        "사용자 지정 반복 횟수는 1 이상 {} 이하여야 합니다.",
                        u16::MAX
                    ),
                ));
            }
            Some(count)
        }
        _ => {
            return Err(AppError::new(
                "validation",
                "GIF 반복은 once, infinite, count 중 하나여야 합니다.",
            ))
        }
    };
    let repeat = output_repeat_for_settings(&loop_mode, normalized_count, "once", None)?;
    Ok((repeat, loop_mode, normalized_count))
}

fn normalized_display_name(display_name: &str) -> AppResult<String> {
    let display_name = display_name.trim();
    if display_name.is_empty() {
        return Err(AppError::new(
            "validation",
            "새 GIF 아이콘 이름을 입력해 주세요.",
        ));
    }
    if display_name.chars().count() > 200 {
        return Err(AppError::new(
            "validation",
            "GIF 아이콘 이름은 200자 이하여야 합니다.",
        ));
    }
    Ok(display_name.to_string())
}

fn validate_grid_cell_count(rows: i64, columns: i64) -> AppResult<()> {
    let count = rows.checked_mul(columns).ok_or_else(|| {
        AppError::new(
            "validation",
            "시트 행과 열 조합이 너무 커서 분석할 수 없습니다.",
        )
    })?;
    if count <= 0 || count > MAX_ANALYZED_GRID_CELLS {
        return Err(AppError::new(
            "validation",
            format!("한 시트에서 분석할 수 있는 셀은 최대 {MAX_ANALYZED_GRID_CELLS}개입니다."),
        ));
    }
    Ok(())
}

fn selected_pixel_characteristics(
    sheet: &RgbaImage,
    cells: &HashMap<i64, SheetCell>,
    selected_indexes: &HashSet<i64>,
) -> (bool, bool) {
    let mut has_partial_alpha = false;
    let mut colors = HashSet::with_capacity(257);

    for index in selected_indexes {
        let Some(cell) = cells.get(index) else {
            continue;
        };
        let start_x = cell.x.max(0) as u32;
        let start_y = cell.y.max(0) as u32;
        let end_x = (cell.x + cell.w).max(0) as u32;
        let end_y = (cell.y + cell.h).max(0) as u32;
        for y in start_y..end_y.min(sheet.height()) {
            for x in start_x..end_x.min(sheet.width()) {
                let color = sheet.get_pixel(x, y).0;
                has_partial_alpha |= (1..=254).contains(&color[3]);
                if colors.len() <= 256 {
                    colors.insert(color);
                }
            }
        }
    }

    (has_partial_alpha, colors.len() > 256)
}

fn measurement_from_rendered(
    paths: &AppPaths,
    rendered: &RenderedFrameSheetGif,
    max_bytes: i64,
) -> AppResult<FrameSheetGifMeasurement> {
    let preview_path = ensure_temp_preview(paths, &rendered.render_hash, &rendered.gif_bytes)?;
    let byte_size = i64::try_from(rendered.gif_bytes.len()).unwrap_or(i64::MAX);
    let max_bytes = max_bytes.max(1);
    let passes_byte_limit = byte_size <= max_bytes;
    let mut warnings = rendered.warnings.clone();
    if !passes_byte_limit {
        warnings.push(format!(
            "생성 GIF가 현재 모음 제한을 초과합니다: {byte_size} / {max_bytes} bytes."
        ));
    }

    Ok(FrameSheetGifMeasurement {
        preview_path: path_string(&preview_path),
        render_hash: rendered.render_hash.clone(),
        byte_size,
        max_bytes,
        passes_byte_limit,
        source_frame_count: i64::try_from(rendered.normalized_source_frames.len())
            .unwrap_or(i64::MAX),
        generated_frame_count: i64::try_from(rendered.normalized_generated_durations_ms.len())
            .unwrap_or(i64::MAX),
        duration_ms: rendered.duration_ms,
        width: rendered.width,
        height: rendered.height,
        normalized_frame_durations_ms: rendered.normalized_generated_durations_ms.clone(),
        warnings,
    })
}

fn ensure_temp_preview(
    paths: &AppPaths,
    render_hash: &str,
    gif_bytes: &[u8],
) -> AppResult<PathBuf> {
    let directory = paths.temp_import_dir.join("frame_sheet_gif_previews");
    fs::create_dir_all(&directory)?;
    let target = directory.join(format!("{render_hash}.gif"));
    if target.is_file() {
        return Ok(target);
    }

    let temp = directory.join(format!("{}.gif.tmp", create_id("frame_sheet_gif_preview")));
    fs::write(&temp, gif_bytes)?;
    match fs::rename(&temp, &target) {
        Ok(()) => {}
        Err(error) if target.is_file() => {
            let _ = fs::remove_file(&temp);
            let _ = error;
        }
        Err(error) => {
            let _ = fs::remove_file(&temp);
            return Err(AppError::from(error));
        }
    }
    Ok(target)
}

fn verify_expected_render_hash(expected: Option<&str>, actual: &str) -> AppResult<()> {
    let expected = expected
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::new(
                "validation",
                "GIF를 만들기 전에 현재 설정으로 용량을 다시 측정해 주세요.",
            )
        })?;
    if !expected.eq_ignore_ascii_case(actual) {
        return Err(AppError::new(
            "validation",
            "프레임이나 재생 설정이 측정 후 바뀌었습니다. 용량을 다시 측정해 주세요.",
        ));
    }
    Ok(())
}

fn load_collection_measurement_settings(
    connection: &Connection,
    collection_id: &str,
) -> AppResult<CollectionMeasurementSettings> {
    if collection_id.is_empty() {
        return Err(AppError::new(
            "validation",
            "GIF를 추가할 모음이 필요합니다.",
        ));
    }
    connection
        .query_row(
            "SELECT max_bytes
             FROM collections
             WHERE id = ?1
               AND deleted_at IS NULL",
            params![collection_id],
            |row| {
                Ok(CollectionMeasurementSettings {
                    max_bytes: row.get("max_bytes")?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("GIF를 추가할 모음을 찾을 수 없습니다."))
}

fn ensure_collection_exists(transaction: &Transaction<'_>, collection_id: &str) -> AppResult<()> {
    let exists = transaction
        .query_row(
            "SELECT 1
             FROM collections
             WHERE id = ?1
               AND deleted_at IS NULL",
            params![collection_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
        .is_some();
    if !exists {
        return Err(AppError::not_found("GIF를 추가할 모음을 찾을 수 없습니다."));
    }
    Ok(())
}

fn next_icon_order_index(transaction: &Transaction<'_>, collection_id: &str) -> AppResult<i64> {
    Ok(transaction.query_row(
        "SELECT COALESCE(MAX(order_index) + 1, 0)
         FROM icons
         WHERE collection_id = ?1
           AND deleted_at IS NULL",
        params![collection_id],
        |row| row.get(0),
    )?)
}

#[allow(clippy::too_many_arguments)]
fn insert_generated_icon(
    transaction: &Transaction<'_>,
    icon_id: &str,
    collection_id: &str,
    source_file: &StoredSourceFile,
    display_name: &str,
    order_index: i64,
    width: i64,
    height: i64,
) -> AppResult<()> {
    transaction.execute(
        "INSERT INTO icons (
           id,
           collection_id,
           source_file_id,
           display_name,
           icon_kind,
           readiness,
           shape,
           order_index,
           cell_width_override,
           cell_height_override,
           thumbnail_path,
           current_preview_path,
           gif_loop_mode,
           gif_loop_count,
           gif_pingpong,
           created_at,
           updated_at
         )
         VALUES (
           ?1,
           ?2,
           ?3,
           ?4,
           'image',
           'complete',
           'single',
           ?5,
           ?6,
           ?7,
           ?8,
           ?9,
           'preserve',
           NULL,
           0,
           strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
           strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         )",
        params![
            icon_id,
            collection_id,
            source_file.id,
            display_name,
            order_index,
            width,
            height,
            source_file.thumbnail_path,
            source_file.original_path_in_library,
        ],
    )?;
    Ok(())
}

fn insert_full_crop(
    transaction: &Transaction<'_>,
    crop_id: &str,
    icon_id: &str,
    width: i64,
    height: i64,
) -> AppResult<()> {
    transaction.execute(
        "INSERT INTO crop_settings (
           id,
           icon_id,
           crop_mode,
           crop_x,
           crop_y,
           crop_w,
           crop_h,
           preset_position,
           source_width_at_apply,
           source_height_at_apply,
           viewport_width_at_apply,
           viewport_height_at_apply,
           updated_at
         )
         VALUES (
           ?1,
           ?2,
           'free',
           0,
           0,
           ?3,
           ?4,
           'center',
           ?3,
           ?4,
           ?3,
           ?4,
           strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         )",
        params![crop_id, icon_id, width, height],
    )?;
    Ok(())
}

fn insert_single_piece(
    transaction: &Transaction<'_>,
    piece_id: &str,
    icon_id: &str,
) -> AppResult<()> {
    transaction.execute(
        "INSERT INTO icon_pieces (
           id,
           icon_id,
           piece_index,
           piece_role,
           alt_text,
           export_status,
           created_at,
           updated_at
         )
         VALUES (
           ?1,
           ?2,
           0,
           'single',
           '',
           'not_exported',
           strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
           strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         )",
        params![piece_id, icon_id],
    )?;
    Ok(())
}

fn set_collection_cover_if_empty(
    transaction: &Transaction<'_>,
    collection_id: &str,
    icon_id: &str,
    source_file_id: &str,
) -> AppResult<()> {
    transaction.execute(
        "UPDATE collections
         SET cover_icon_id = ?1,
             cover_source_file_id = ?2,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?3
           AND deleted_at IS NULL
           AND cover_icon_id IS NULL
           AND cover_source_file_id IS NULL",
        params![icon_id, source_file_id, collection_id],
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn insert_recipe(
    transaction: &Transaction<'_>,
    recipe_id: &str,
    icon_id: &str,
    rendered: &RenderedFrameSheetGif,
    preserved_sheet_path: &Path,
    grid_settings_json: &str,
    frames_json: &str,
    measured_byte_size: i64,
) -> AppResult<()> {
    transaction.execute(
        "INSERT INTO frame_sheet_gif_recipes (
           id,
           generated_icon_id,
           original_sheet_filename,
           original_sheet_path,
           original_sheet_sha256,
           recipe_schema,
           grid_settings_json,
           frames_json,
           direction,
           loop_mode,
           loop_count,
           measured_byte_size,
           render_hash,
           created_at,
           updated_at
         )
         VALUES (
           ?1,
           ?2,
           ?3,
           ?4,
           ?5,
           ?6,
           ?7,
           ?8,
           ?9,
           ?10,
           ?11,
           ?12,
           ?13,
           strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
           strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         )",
        params![
            recipe_id,
            icon_id,
            rendered.original_sheet_filename,
            path_string(preserved_sheet_path),
            rendered.original_sheet_sha256,
            RECIPE_SCHEMA,
            grid_settings_json,
            frames_json,
            rendered.direction,
            rendered.loop_mode,
            rendered.loop_count,
            measured_byte_size,
            rendered.render_hash,
        ],
    )?;
    Ok(())
}

fn serialize_json<T: Serialize>(value: &T) -> AppResult<String> {
    serde_json::to_string(value).map_err(|error| AppError::new("serialization", error.to_string()))
}

fn generated_gif_filename(display_name: &str) -> String {
    let mut stem = display_name
        .chars()
        .map(|character| {
            if character.is_control()
                || matches!(
                    character,
                    '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
                )
            {
                '_'
            } else {
                character
            }
        })
        .collect::<String>();
    stem = stem.trim_matches([' ', '.']).to_string();
    if stem.is_empty() {
        stem = "frame_sheet_gif".to_string();
    }
    format!("{stem}.gif")
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(64);
    for byte in digest {
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::time::{SystemTime, UNIX_EPOCH};

    use image::codecs::gif::GifDecoder;
    use image::{AnimationDecoder, DynamicImage, ImageBuffer, ImageFormat, Rgba};
    use rusqlite::{params, Connection};

    use crate::db::migrations;
    use crate::db::repositories::collections::create_collection;
    use crate::imaging::import_limits::validate_gif_workload;
    use crate::models::ImportImageFilePayload;
    use crate::paths::AppPaths;
    use crate::sheet::grid::SheetGridSettings;

    use super::{
        create_frame_sheet_gif, measure_frame_sheet_gif, FrameSheetGifFrameInput,
        FrameSheetGifRequest, RECIPE_SCHEMA,
    };

    fn connection() -> Connection {
        let mut connection = Connection::open_in_memory().unwrap();
        migrations::run(&mut connection).unwrap();
        connection
    }

    fn temp_paths(prefix: &str) -> AppPaths {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        AppPaths::prepare(std::env::temp_dir().join(format!("{prefix}-{suffix}"))).unwrap()
    }

    fn png_sheet_bytes(cells: &[[u8; 4]], cell_width: u32, cell_height: u32) -> Vec<u8> {
        let mut image = ImageBuffer::from_pixel(
            cell_width * cells.len() as u32,
            cell_height,
            Rgba([0, 0, 0, 0]),
        );
        for (cell_index, color) in cells.iter().enumerate() {
            for y in 0..cell_height {
                for x in 0..cell_width {
                    image.put_pixel(cell_index as u32 * cell_width + x, y, Rgba(*color));
                }
            }
        }
        let mut cursor = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image)
            .write_to(&mut cursor, ImageFormat::Png)
            .unwrap();
        cursor.into_inner()
    }

    fn colorful_partial_alpha_sheet_bytes() -> Vec<u8> {
        let mut image = ImageBuffer::from_pixel(40, 20, Rgba([0, 0, 0, 255]));
        for y in 0..20_u32 {
            for x in 0..20_u32 {
                let alpha = if x == 0 && y == 0 { 128 } else { 255 };
                image.put_pixel(
                    x,
                    y,
                    Rgba([(x * 11) as u8, (y * 11) as u8, ((x + y) * 5) as u8, alpha]),
                );
            }
        }
        let mut cursor = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image)
            .write_to(&mut cursor, ImageFormat::Png)
            .unwrap();
        cursor.into_inner()
    }

    fn request(collection_id: &str, bytes: Vec<u8>, frame_count: usize) -> FrameSheetGifRequest {
        FrameSheetGifRequest {
            sheet_path: None,
            sheet_file: Some(ImportImageFilePayload {
                original_filename: "frames.png".to_string(),
                bytes,
            }),
            target_collection_id: collection_id.to_string(),
            grid_settings: SheetGridSettings {
                mode: "rows_columns".to_string(),
                rows: Some(1),
                columns: Some(frame_count as i64),
                cell_width: Some(4),
                cell_height: Some(4),
                border_left: 0,
                border_top: 0,
                border_right: 0,
                border_bottom: 0,
                gap_x: 0,
                gap_y: 0,
                read_order: "row_major".to_string(),
                empty_cell_threshold: Some(0.98),
            },
            frames: (0..frame_count)
                .map(|index| FrameSheetGifFrameInput {
                    source_cell_index: index as i64,
                    duration_ms: 100,
                })
                .collect(),
            direction: "forward".to_string(),
            loop_mode: "infinite".to_string(),
            loop_count: None,
            display_name: "프레임 GIF".to_string(),
            expected_render_hash: None,
        }
    }

    fn decoded_frames(path: &str) -> Vec<image::Frame> {
        let file = std::fs::File::open(path).unwrap();
        let decoder = GifDecoder::new(std::io::BufReader::new(file)).unwrap();
        decoder.into_frames().collect_frames().unwrap()
    }

    fn frame_delay_ms(frame: &image::Frame) -> u32 {
        let (numerator, denominator) = frame.delay().numer_denom_ms();
        numerator / denominator.max(1)
    }

    #[test]
    fn measurement_uses_requested_order_reverse_timing_and_palette() {
        let mut connection = connection();
        let paths = temp_paths("pmtcon-frame-gif-order");
        let collection = create_collection(&mut connection, Some("frame gif".to_string())).unwrap();
        let bytes = png_sheet_bytes(
            &[[255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255]],
            4,
            4,
        );
        let mut request = request(&collection.id, bytes, 3);
        request.frames = vec![
            FrameSheetGifFrameInput {
                source_cell_index: 2,
                duration_ms: 96,
            },
            FrameSheetGifFrameInput {
                source_cell_index: 0,
                duration_ms: 25,
            },
            FrameSheetGifFrameInput {
                source_cell_index: 1,
                duration_ms: 44,
            },
        ];
        request.direction = "reverse".to_string();

        let measurement = measure_frame_sheet_gif(&connection, &paths, request).unwrap();
        assert_eq!(measurement.source_frame_count, 3);
        assert_eq!(measurement.generated_frame_count, 3);
        assert_eq!(measurement.normalized_frame_durations_ms, vec![40, 30, 100]);
        assert_eq!(measurement.duration_ms, 170);
        assert_eq!(
            measurement.byte_size,
            std::fs::metadata(&measurement.preview_path).unwrap().len() as i64
        );

        let frames = decoded_frames(&measurement.preview_path);
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0].buffer().get_pixel(0, 0).0, [0, 255, 0, 255]);
        assert_eq!(frames[1].buffer().get_pixel(0, 0).0, [255, 0, 0, 255]);
        assert_eq!(frames[2].buffer().get_pixel(0, 0).0, [0, 0, 255, 255]);
        assert_eq!(
            frames.iter().map(frame_delay_ms).collect::<Vec<_>>(),
            vec![40, 30, 100]
        );

        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn pingpong_omits_duplicate_endpoints_and_preserves_transparency() {
        let mut connection = connection();
        let paths = temp_paths("pmtcon-frame-gif-pingpong");
        let collection = create_collection(&mut connection, Some("pingpong".to_string())).unwrap();
        let bytes = png_sheet_bytes(&[[255, 0, 0, 0], [0, 255, 0, 255], [0, 0, 255, 255]], 4, 4);
        let mut request = request(&collection.id, bytes, 3);
        request.frames[0].duration_ms = 10;
        request.frames[1].duration_ms = 20;
        request.frames[2].duration_ms = 30;
        request.direction = "pingpong".to_string();
        request.loop_mode = "count".to_string();
        request.loop_count = Some(3);

        let measurement = measure_frame_sheet_gif(&connection, &paths, request).unwrap();
        assert_eq!(measurement.generated_frame_count, 4);
        assert_eq!(
            measurement.normalized_frame_durations_ms,
            vec![10, 20, 30, 20]
        );

        let frames = decoded_frames(&measurement.preview_path);
        assert_eq!(frames.len(), 4);
        assert_eq!(frames[0].buffer().get_pixel(0, 0).0[3], 0);
        assert_eq!(frames[1].buffer().get_pixel(0, 0).0, [0, 255, 0, 255]);
        assert_eq!(frames[2].buffer().get_pixel(0, 0).0, [0, 0, 255, 255]);
        assert_eq!(frames[3].buffer().get_pixel(0, 0).0, [0, 255, 0, 255]);

        let gif_bytes = std::fs::read(&measurement.preview_path).unwrap();
        let mut options = gif::DecodeOptions::new();
        options.set_color_output(gif::ColorOutput::RGBA);
        let reader = options.read_info(Cursor::new(gif_bytes)).unwrap();
        assert_eq!(reader.repeat(), gif::Repeat::Finite(3));

        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn one_shot_pingpong_returns_to_the_start_frame() {
        let mut connection = connection();
        let paths = temp_paths("pmtcon-frame-gif-pingpong-once");
        let collection =
            create_collection(&mut connection, Some("pingpong once".to_string())).unwrap();
        let bytes = png_sheet_bytes(
            &[[255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255]],
            4,
            4,
        );
        let mut request = request(&collection.id, bytes, 3);
        request.direction = "pingpong".to_string();
        request.loop_mode = "once".to_string();

        let measurement = measure_frame_sheet_gif(&connection, &paths, request).unwrap();
        let frames = decoded_frames(&measurement.preview_path);

        assert_eq!(measurement.generated_frame_count, 5);
        assert_eq!(frames[0].buffer().get_pixel(0, 0).0, [255, 0, 0, 255]);
        assert_eq!(frames[4].buffer().get_pixel(0, 0).0, [255, 0, 0, 255]);

        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn final_frame_and_pixel_limits_are_checked_after_direction_expansion() {
        let mut connection = connection();
        let paths = temp_paths("pmtcon-frame-gif-limits");
        let collection = create_collection(&mut connection, Some("limits".to_string())).unwrap();
        let bytes = png_sheet_bytes(&[[255, 0, 0, 255]], 4, 4);
        let mut request = request(&collection.id, bytes, 1);
        request.frames = (0..252)
            .map(|_| FrameSheetGifFrameInput {
                source_cell_index: 0,
                duration_ms: 10,
            })
            .collect();
        request.direction = "pingpong".to_string();

        let error = measure_frame_sheet_gif(&connection, &paths, request).unwrap_err();
        assert!(error.message.contains("500"));
        assert!(validate_gif_workload(2_000, 2_000, 32).is_ok());
        assert!(validate_gif_workload(2_000, 2_000, 33).is_err());

        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn measurement_warns_when_real_gif_exceeds_collection_limit() {
        let mut connection = connection();
        let paths = temp_paths("pmtcon-frame-gif-byte-limit");
        let collection =
            create_collection(&mut connection, Some("byte limit".to_string())).unwrap();
        connection
            .execute(
                "UPDATE collections SET max_bytes = 1 WHERE id = ?1",
                [&collection.id],
            )
            .unwrap();
        let bytes = png_sheet_bytes(&[[255, 0, 0, 255], [0, 255, 0, 255]], 4, 4);
        let measurement =
            measure_frame_sheet_gif(&connection, &paths, request(&collection.id, bytes, 2))
                .unwrap();

        assert!(!measurement.passes_byte_limit);
        assert_eq!(measurement.max_bytes, 1);
        assert!(measurement
            .warnings
            .iter()
            .any(|warning| warning.contains("모음 제한")));

        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn measurement_warns_for_partial_alpha_and_palette_quantization() {
        let mut connection = connection();
        let paths = temp_paths("pmtcon-frame-gif-color-warning");
        let collection =
            create_collection(&mut connection, Some("color warning".to_string())).unwrap();
        let mut request = request(&collection.id, colorful_partial_alpha_sheet_bytes(), 2);
        request.grid_settings.cell_width = Some(20);
        request.grid_settings.cell_height = Some(20);

        let measurement = measure_frame_sheet_gif(&connection, &paths, request).unwrap();

        assert!(measurement
            .warnings
            .iter()
            .any(|warning| warning.contains("부분 투명도")));
        assert!(measurement
            .warnings
            .iter()
            .any(|warning| warning.contains("256색")));

        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn create_preserves_sheet_and_commits_animated_icon_crop_piece_cover_and_recipe() {
        let mut connection = connection();
        let paths = temp_paths("pmtcon-frame-gif-create");
        let collection =
            create_collection(&mut connection, Some("native workflow".to_string())).unwrap();
        let sheet_bytes = png_sheet_bytes(&[[255, 0, 0, 128], [0, 255, 0, 255]], 4, 4);
        let mut request = request(&collection.id, sheet_bytes.clone(), 2);
        request.loop_mode = "once".to_string();

        let measurement = measure_frame_sheet_gif(&connection, &paths, request.clone()).unwrap();
        request.expected_render_hash = Some(measurement.render_hash.clone());
        let created = create_frame_sheet_gif(&mut connection, &paths, request).unwrap();

        assert_eq!(created.icon.collection_id, collection.id);
        assert_eq!(created.icon.shape, "single");
        assert_eq!(created.icon.cell_width_override, Some(4));
        assert_eq!(created.icon.cell_height_override, Some(4));
        assert_eq!(created.icon.pieces.len(), 1);
        assert!(created
            .icon
            .current_preview_url
            .as_deref()
            .is_some_and(|path| path.ends_with(".gif")));
        assert_eq!(
            std::fs::read(&created.preserved_sheet_path).unwrap(),
            sheet_bytes
        );

        let source: (i64, Option<i64>, String, i64, i64) = connection
            .query_row(
                "SELECT
                   is_animated,
                   frame_count,
                   original_loop_mode,
                   width,
                   height
                 FROM source_files
                 WHERE id = ?1",
                [&created.icon.source_file_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(source, (1, Some(2), "once".to_string(), 4, 4));

        let crop: (f64, f64, f64, f64, i64, i64) = connection
            .query_row(
                "SELECT
                   crop_x,
                   crop_y,
                   crop_w,
                   crop_h,
                   viewport_width_at_apply,
                   viewport_height_at_apply
                 FROM crop_settings
                 WHERE icon_id = ?1",
                [&created.icon.id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(crop, (0.0, 0.0, 4.0, 4.0, 4, 4));

        let cover_icon_id: Option<String> = connection
            .query_row(
                "SELECT cover_icon_id FROM collections WHERE id = ?1",
                [&collection.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(cover_icon_id, Some(created.icon.id.clone()));

        let recipe: (String, String, i64, String) = connection
            .query_row(
                "SELECT
                   recipe_schema,
                   render_hash,
                   measured_byte_size,
                   original_sheet_sha256
                 FROM frame_sheet_gif_recipes
                 WHERE id = ?1",
                [&created.recipe_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(recipe.0, RECIPE_SCHEMA);
        assert_eq!(recipe.1, created.measurement.render_hash);
        assert_eq!(recipe.2, created.measurement.byte_size);
        assert_eq!(recipe.3.len(), 64);

        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn create_rejects_stale_measurement_before_persisting_rows() {
        let mut connection = connection();
        let paths = temp_paths("pmtcon-frame-gif-stale");
        let collection = create_collection(&mut connection, Some("stale".to_string())).unwrap();
        let bytes = png_sheet_bytes(&[[255, 0, 0, 255], [0, 255, 0, 255]], 4, 4);
        let mut request = request(&collection.id, bytes, 2);
        request.expected_render_hash = Some("0".repeat(64));

        let error = create_frame_sheet_gif(&mut connection, &paths, request).unwrap_err();
        assert!(error.message.contains("측정"));
        let icon_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM icons WHERE collection_id = ?1",
                params![collection.id],
                |row| row.get(0),
            )
            .unwrap();
        let recipe_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM frame_sheet_gif_recipes", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(icon_count, 0);
        assert_eq!(recipe_count, 0);

        std::fs::remove_dir_all(paths.root).unwrap();
    }
}
