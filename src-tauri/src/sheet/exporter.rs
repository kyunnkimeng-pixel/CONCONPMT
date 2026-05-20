use std::collections::HashSet;
use std::fmt::Write as FmtWrite;
use std::fs;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use image::codecs::gif::GifDecoder;
use image::imageops::{self, FilterType};
use image::{AnimationDecoder, DynamicImage, ImageFormat, Rgba, RgbaImage};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{AppError, AppResult};
use crate::paths::AppPaths;

use super::grid::{split_pages, PageCellPlacement, PageSplitSettings};
use super::importer::png_bytes_from_rgba;
use super::manifest::{
    write_static_manifest, StaticSheetManifest, StaticSheetManifestItem, StaticSheetPage,
    StaticSheetProfile, APP_NAME, STATIC_SHEET_SCHEMA,
};
use super::path_string;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportEditSheetRequest {
    pub collection_id: String,
    #[serde(default)]
    pub selected_icon_ids: Vec<String>,
    #[serde(default = "default_sheet_source")]
    pub source: String,
    pub cell_width: i64,
    pub cell_height: i64,
    pub columns: i64,
    #[serde(default)]
    pub gap_x: i64,
    #[serde(default)]
    pub gap_y: i64,
    #[serde(default)]
    pub border_x: i64,
    #[serde(default)]
    pub border_y: i64,
    #[serde(default = "default_background")]
    pub background: String,
    #[serde(default = "default_true")]
    pub include_clean_sheet: bool,
    #[serde(default = "default_true")]
    pub include_guide_sheet: bool,
    #[serde(default = "default_true")]
    pub include_manifest: bool,
    pub label_options: Option<GuideLabelOptions>,
    #[serde(default = "default_max_sheet_size")]
    pub max_sheet_width: i64,
    #[serde(default = "default_max_sheet_size")]
    pub max_sheet_height: i64,
    pub output_directory: Option<String>,
    #[serde(default)]
    pub open_output_folder: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuideLabelOptions {
    #[serde(default = "default_true")]
    pub cell_number: bool,
    #[serde(default)]
    pub icon_name: bool,
    #[serde(default)]
    pub alt_value: bool,
    #[serde(default)]
    pub export_number: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportEditSheetResult {
    pub clean_sheet_paths: Vec<String>,
    pub guide_sheet_paths: Vec<String>,
    pub manifest_path: Option<String>,
    pub output_directory: String,
    pub item_count: i64,
    pub page_count: i64,
    pub warnings: Vec<String>,
}

#[derive(Debug)]
struct CollectionRecord {
    id: String,
    name: String,
}

#[derive(Debug)]
struct IconRecord {
    id: String,
    display_name: String,
    shape: String,
    source_path: String,
    source_extension: String,
    source_hash: String,
    source_is_animated: bool,
    crop_x: f64,
    crop_y: f64,
    crop_w: f64,
    crop_h: f64,
}

#[derive(Debug)]
struct PieceRecord {
    id: String,
    piece_index: i64,
    alt_text: String,
}

#[derive(Debug)]
struct RenderedSheetItem {
    icon_id: String,
    piece_id: Option<String>,
    display_name: String,
    alt: String,
    icon_type: String,
    source_hash: Option<String>,
    render_hash: String,
    image: RgbaImage,
}

pub fn export_edit_sheet(
    connection: &Connection,
    paths: &AppPaths,
    request: ExportEditSheetRequest,
) -> AppResult<ExportEditSheetResult> {
    if request.cell_width <= 0 || request.cell_height <= 0 || request.columns <= 0 {
        return Err(AppError::new(
            "validation",
            "작업 시트 셀 크기와 열 수는 1 이상이어야 합니다.",
        ));
    }
    if !request.include_clean_sheet && !request.include_guide_sheet && !request.include_manifest {
        return Err(AppError::new(
            "validation",
            "내보낼 작업 시트 산출물을 하나 이상 선택해야 합니다.",
        ));
    }

    let collection = load_collection(connection, &request.collection_id)?;
    let icons = load_icons(connection, &request)?;
    if icons.is_empty() {
        return Err(AppError::new(
            "validation",
            "작업 시트로 내보낼 아이콘이 없습니다.",
        ));
    }

    let mut warnings = Vec::new();
    let mut rendered_items = Vec::new();
    for icon in icons {
        if icon.source_is_animated {
            warnings.push(format!(
                "{}: GIF 첫 프레임만 정적 작업 시트에 포함했습니다.",
                icon.display_name
            ));
        }
        match render_icon_items(connection, &icon, request.cell_width, request.cell_height) {
            Ok(items) => rendered_items.extend(items),
            Err(error) => warnings.push(format!("{}: {}", icon.display_name, error.message)),
        }
    }

    if rendered_items.is_empty() {
        return Err(AppError::new(
            "validation",
            "작업 시트에 배치할 수 있는 렌더링 결과가 없습니다.",
        ));
    }

    let split = split_pages(
        rendered_items.len(),
        PageSplitSettings {
            cell_width: request.cell_width,
            cell_height: request.cell_height,
            columns: request.columns,
            gap_x: request.gap_x,
            gap_y: request.gap_y,
            border_x: request.border_x,
            border_y: request.border_y,
            max_sheet_width: request.max_sheet_width,
            max_sheet_height: request.max_sheet_height,
        },
    )?;
    let _rows_per_page = split.rows_per_page;
    warnings.extend(split.warnings);

    let output_root = output_directory(paths, &request, &collection.name)?;
    let clean_dir = output_root.join("clean");
    let guide_dir = output_root.join("guide");
    fs::create_dir_all(&clean_dir)?;
    fs::create_dir_all(&guide_dir)?;

    let mut clean_paths = Vec::new();
    let mut guide_paths = Vec::new();
    let mut manifest_pages = Vec::new();
    let mut manifest_items = Vec::new();

    for page in &split.pages {
        let page_index = page.page_index;
        let clean_file = format!("sheet_{:03}.png", page_index + 1);
        let guide_file = format!("sheet_guide_{:03}.png", page_index + 1);
        let page_placements = split
            .placements
            .iter()
            .filter(|placement| placement.page_index == page_index)
            .collect::<Vec<_>>();

        if request.include_clean_sheet {
            let clean_image = render_sheet_page(
                &rendered_items,
                &page_placements,
                page.width,
                page.height,
                &request.background,
                false,
                request.label_options.as_ref(),
            )?;
            let clean_path = clean_dir.join(&clean_file);
            clean_image.save_with_format(&clean_path, ImageFormat::Png)?;
            clean_paths.push(path_string(&clean_path));
        }

        if request.include_guide_sheet {
            let guide_image = render_sheet_page(
                &rendered_items,
                &page_placements,
                page.width,
                page.height,
                &request.background,
                true,
                request.label_options.as_ref(),
            )?;
            let guide_path = guide_dir.join(&guide_file);
            guide_image.save_with_format(&guide_path, ImageFormat::Png)?;
            guide_paths.push(path_string(&guide_path));
        }

        manifest_pages.push(StaticSheetPage {
            page_index,
            clean_sheet_file: clean_file,
            guide_sheet_file: request.include_guide_sheet.then_some(guide_file),
            width: page.width,
            height: page.height,
        });
    }

    for (export_index, placement) in split.placements.iter().enumerate() {
        let item = &rendered_items[placement.item_index];
        manifest_items.push(StaticSheetManifestItem {
            icon_id: item.icon_id.clone(),
            piece_id: item.piece_id.clone(),
            page_index: placement.page_index,
            row: placement.row,
            col: placement.col,
            index: export_index as i64,
            export_number: export_index as i64 + 1,
            x: placement.x,
            y: placement.y,
            w: placement.w,
            h: placement.h,
            display_name: item.display_name.clone(),
            alt: item.alt.clone(),
            icon_type: item.icon_type.clone(),
            format: "png".to_string(),
            source_hash: item.source_hash.clone(),
            render_hash: Some(item.render_hash.clone()),
        });
    }

    let manifest_path = if request.include_manifest {
        let manifest = StaticSheetManifest {
            schema: STATIC_SHEET_SCHEMA.to_string(),
            app: APP_NAME.to_string(),
            created_at: now_iso_like(),
            collection_id: collection.id,
            sheet_type: "static_edit_sheet".to_string(),
            profile: StaticSheetProfile {
                cell_width: request.cell_width,
                cell_height: request.cell_height,
                columns: split.columns_per_page,
                gap_x: request.gap_x.max(0),
                gap_y: request.gap_y.max(0),
                border_x: request.border_x.max(0),
                border_y: request.border_y.max(0),
                background: normalized_background(&request.background),
                read_order: "row_major".to_string(),
            },
            pages: manifest_pages,
            items: manifest_items,
        };
        let path = output_root.join("sheet_manifest.json");
        write_static_manifest(&path, &manifest)?;
        Some(path_string(&path))
    } else {
        None
    };

    if request.open_output_folder {
        crate::export::open_export_path(&path_string(&output_root))?;
    }

    Ok(ExportEditSheetResult {
        clean_sheet_paths: clean_paths,
        guide_sheet_paths: guide_paths,
        manifest_path,
        output_directory: path_string(&output_root),
        item_count: rendered_items.len() as i64,
        page_count: split.pages.len() as i64,
        warnings,
    })
}

fn load_collection(connection: &Connection, collection_id: &str) -> AppResult<CollectionRecord> {
    connection
        .query_row(
            "SELECT id, name
             FROM collections
             WHERE id = ?1
               AND deleted_at IS NULL",
            params![collection_id],
            |row| {
                Ok(CollectionRecord {
                    id: row.get("id")?,
                    name: row.get("name")?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("작업 시트로 내보낼 모음을 찾을 수 없습니다."))
}

fn load_icons(
    connection: &Connection,
    request: &ExportEditSheetRequest,
) -> AppResult<Vec<IconRecord>> {
    let selected_ids = request
        .selected_icon_ids
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let mut statement = connection.prepare(
        "SELECT
           i.id,
           i.display_name,
           i.shape,
           s.original_path_in_library,
           s.original_extension,
           s.sha256,
           s.is_animated,
           cs.crop_x,
           cs.crop_y,
           cs.crop_w,
           cs.crop_h
         FROM icons i
         JOIN source_files s ON s.id = i.source_file_id
         JOIN crop_settings cs ON cs.icon_id = i.id
         WHERE i.collection_id = ?1
           AND i.deleted_at IS NULL
           AND i.icon_kind = 'image'
         ORDER BY i.order_index ASC, i.created_at ASC",
    )?;
    let icons = statement
        .query_map(params![request.collection_id], |row| {
            Ok(IconRecord {
                id: row.get("id")?,
                display_name: row.get("display_name")?,
                shape: row.get("shape")?,
                source_path: row.get("original_path_in_library")?,
                source_extension: row.get("original_extension")?,
                source_hash: row.get("sha256")?,
                source_is_animated: row.get::<_, i64>("is_animated")? == 1,
                crop_x: row.get("crop_x")?,
                crop_y: row.get("crop_y")?,
                crop_w: row.get("crop_w")?,
                crop_h: row.get("crop_h")?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    if request.source == "selected_icons" && !selected_ids.is_empty() {
        Ok(icons
            .into_iter()
            .filter(|icon| selected_ids.contains(icon.id.as_str()))
            .collect())
    } else {
        Ok(icons)
    }
}

fn load_pieces(connection: &Connection, icon_id: &str) -> AppResult<Vec<PieceRecord>> {
    let mut statement = connection.prepare(
        "SELECT id, piece_index, alt_text
         FROM icon_pieces
         WHERE icon_id = ?1
         ORDER BY piece_index ASC",
    )?;
    let pieces = statement
        .query_map(params![icon_id], |row| {
            Ok(PieceRecord {
                id: row.get("id")?,
                piece_index: row.get("piece_index")?,
                alt_text: row.get("alt_text")?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(pieces)
}

fn render_icon_items(
    connection: &Connection,
    icon: &IconRecord,
    cell_width: i64,
    cell_height: i64,
) -> AppResult<Vec<RenderedSheetItem>> {
    let source = load_source_first_frame(Path::new(&icon.source_path), &icon.source_extension)?;
    let viewport = crop_and_resize(
        &source,
        icon.crop_x,
        icon.crop_y,
        icon.crop_w,
        icon.crop_h,
        viewport_width(&icon.shape, cell_width),
        viewport_height(&icon.shape, cell_height),
    )?;
    let split = split_viewport(&viewport, &icon.shape, cell_width, cell_height)?;
    let pieces = load_pieces(connection, &icon.id)?;
    let mut items = Vec::new();

    for (piece_position, piece_image) in split.into_iter().enumerate() {
        let piece = pieces
            .iter()
            .find(|piece| piece.piece_index as usize == piece_position);
        let render_hash = sha256_hex(&png_bytes_from_rgba(&piece_image)?);
        items.push(RenderedSheetItem {
            icon_id: icon.id.clone(),
            piece_id: piece.map(|piece| piece.id.clone()),
            display_name: icon.display_name.clone(),
            alt: piece
                .map(|piece| piece.alt_text.clone())
                .unwrap_or_default(),
            icon_type: icon.shape.clone(),
            source_hash: Some(icon.source_hash.clone()),
            render_hash,
            image: piece_image,
        });
    }

    Ok(items)
}

fn load_source_first_frame(path: &Path, extension: &str) -> AppResult<RgbaImage> {
    if extension == "gif" {
        let file = fs::File::open(path)?;
        let decoder = GifDecoder::new(BufReader::new(file))?;
        let mut frames = decoder.into_frames();
        let frame = frames
            .next()
            .transpose()?
            .ok_or_else(|| AppError::new("gif", "GIF 첫 프레임을 읽을 수 없습니다."))?;
        return Ok(frame.into_buffer());
    }

    Ok(image::open(path)?.to_rgba8())
}

fn crop_and_resize(
    image: &RgbaImage,
    crop_x: f64,
    crop_y: f64,
    crop_w: f64,
    crop_h: f64,
    width: i64,
    height: i64,
) -> AppResult<RgbaImage> {
    if crop_w <= 0.0 || crop_h <= 0.0 {
        return Err(AppError::new("validation", "잘못된 crop 영역입니다."));
    }
    let source = DynamicImage::ImageRgba8(image.clone());
    let cropped = crop_with_padding(&source, crop_x, crop_y, crop_w, crop_h);
    Ok(imageops::resize(
        &cropped,
        width.max(1) as u32,
        height.max(1) as u32,
        FilterType::Lanczos3,
    ))
}

fn crop_with_padding(
    image: &DynamicImage,
    crop_x: f64,
    crop_y: f64,
    crop_w: f64,
    crop_h: f64,
) -> RgbaImage {
    let source = image.to_rgba8();
    let crop_x = crop_x.round() as i64;
    let crop_y = crop_y.round() as i64;
    let crop_width = crop_w.round().max(1.0) as u32;
    let crop_height = crop_h.round().max(1.0) as u32;
    let mut output = RgbaImage::from_pixel(crop_width, crop_height, Rgba([0, 0, 0, 0]));
    let source_width = i64::from(source.width());
    let source_height = i64::from(source.height());
    let src_x = crop_x.max(0);
    let src_y = crop_y.max(0);
    let dst_x = (-crop_x).max(0);
    let dst_y = (-crop_y).max(0);
    let copy_width = (source_width - src_x)
        .min(i64::from(crop_width) - dst_x)
        .max(0) as u32;
    let copy_height = (source_height - src_y)
        .min(i64::from(crop_height) - dst_y)
        .max(0) as u32;

    for y in 0..copy_height {
        for x in 0..copy_width {
            output.put_pixel(
                (dst_x as u32) + x,
                (dst_y as u32) + y,
                *source.get_pixel((src_x as u32) + x, (src_y as u32) + y),
            );
        }
    }
    output
}

fn split_viewport(
    viewport: &RgbaImage,
    shape: &str,
    cell_width: i64,
    cell_height: i64,
) -> AppResult<Vec<RgbaImage>> {
    let width = cell_width.max(1) as u32;
    let height = cell_height.max(1) as u32;
    match shape {
        "horizontal_double" => Ok(vec![
            imageops::crop_imm(viewport, 0, 0, width, height).to_image(),
            imageops::crop_imm(viewport, width, 0, width, height).to_image(),
        ]),
        "vertical_double" => Ok(vec![
            imageops::crop_imm(viewport, 0, 0, width, height).to_image(),
            imageops::crop_imm(viewport, 0, height, width, height).to_image(),
        ]),
        "single" => Ok(vec![viewport.clone()]),
        _ => Err(AppError::new(
            "validation",
            "지원하지 않는 아이콘 모양입니다.",
        )),
    }
}

fn render_sheet_page(
    items: &[RenderedSheetItem],
    placements: &[&PageCellPlacement],
    width: i64,
    height: i64,
    background: &str,
    guide: bool,
    labels: Option<&GuideLabelOptions>,
) -> AppResult<RgbaImage> {
    let mut sheet = background_image(width.max(1) as u32, height.max(1) as u32, background, guide);
    for placement in placements {
        let item = &items[placement.item_index];
        imageops::overlay(&mut sheet, &item.image, placement.x, placement.y);
    }
    if guide {
        let _text_labels_requested =
            labels.is_some_and(|labels| labels.icon_name || labels.alt_value);
        for (local_index, placement) in placements.iter().enumerate() {
            draw_grid_rect(&mut sheet, placement);
            let label_number = if labels.is_some_and(|labels| labels.export_number) {
                placement.item_index + 1
            } else {
                local_index + 1
            };
            if labels
                .map(|labels| labels.cell_number || labels.export_number)
                .unwrap_or(true)
            {
                draw_number_label(&mut sheet, placement.x + 4, placement.y + 4, label_number);
            }
        }
    }
    Ok(sheet)
}

fn background_image(width: u32, height: u32, background: &str, guide: bool) -> RgbaImage {
    let normalized = normalized_background(background);
    if normalized == "checker" || (guide && normalized == "transparent") {
        return checkerboard(width, height);
    }
    let pixel = match normalized.as_str() {
        "white" => Rgba([255, 255, 255, 255]),
        "black" => Rgba([0, 0, 0, 255]),
        _ => Rgba([0, 0, 0, 0]),
    };
    RgbaImage::from_pixel(width, height, pixel)
}

fn checkerboard(width: u32, height: u32) -> RgbaImage {
    let mut image = RgbaImage::from_pixel(width, height, Rgba([235, 238, 242, 255]));
    for y in 0..height {
        for x in 0..width {
            if ((x / 12) + (y / 12)) % 2 == 0 {
                image.put_pixel(x, y, Rgba([255, 255, 255, 255]));
            }
        }
    }
    image
}

fn draw_grid_rect(sheet: &mut RgbaImage, placement: &PageCellPlacement) {
    let color = Rgba([37, 48, 61, 210]);
    let x0 = placement.x.max(0) as u32;
    let y0 = placement.y.max(0) as u32;
    let x1 = (placement.x + placement.w - 1).max(0) as u32;
    let y1 = (placement.y + placement.h - 1).max(0) as u32;

    for x in x0..=x1.min(sheet.width().saturating_sub(1)) {
        if y0 < sheet.height() {
            sheet.put_pixel(x, y0, color);
        }
        if y1 < sheet.height() {
            sheet.put_pixel(x, y1, color);
        }
    }
    for y in y0..=y1.min(sheet.height().saturating_sub(1)) {
        if x0 < sheet.width() {
            sheet.put_pixel(x0, y, color);
        }
        if x1 < sheet.width() {
            sheet.put_pixel(x1, y, color);
        }
    }
}

fn draw_number_label(sheet: &mut RgbaImage, x: i64, y: i64, number: usize) {
    let label = number.to_string();
    let mut cursor_x = x.max(0) as u32;
    let y = y.max(0) as u32;
    for character in label.chars() {
        draw_digit(sheet, cursor_x, y, character);
        cursor_x += 5;
    }
}

fn draw_digit(sheet: &mut RgbaImage, x: u32, y: u32, character: char) {
    let Some(pattern) = digit_pattern(character) else {
        return;
    };
    let background = Rgba([255, 255, 255, 210]);
    let foreground = Rgba([20, 28, 38, 255]);
    for yy in 0..7 {
        for xx in 0..4 {
            let px = x + xx;
            let py = y + yy;
            if px < sheet.width() && py < sheet.height() {
                sheet.put_pixel(px, py, background);
            }
        }
    }
    for (row, bits) in pattern.iter().enumerate() {
        for col in 0..3 {
            if bits & (1 << (2 - col)) != 0 {
                let px = x + col;
                let py = y + row as u32 + 1;
                if px < sheet.width() && py < sheet.height() {
                    sheet.put_pixel(px, py, foreground);
                }
            }
        }
    }
}

fn digit_pattern(character: char) -> Option<[u8; 5]> {
    match character {
        '0' => Some([0b111, 0b101, 0b101, 0b101, 0b111]),
        '1' => Some([0b010, 0b110, 0b010, 0b010, 0b111]),
        '2' => Some([0b111, 0b001, 0b111, 0b100, 0b111]),
        '3' => Some([0b111, 0b001, 0b111, 0b001, 0b111]),
        '4' => Some([0b101, 0b101, 0b111, 0b001, 0b001]),
        '5' => Some([0b111, 0b100, 0b111, 0b001, 0b111]),
        '6' => Some([0b111, 0b100, 0b111, 0b101, 0b111]),
        '7' => Some([0b111, 0b001, 0b010, 0b010, 0b010]),
        '8' => Some([0b111, 0b101, 0b111, 0b101, 0b111]),
        '9' => Some([0b111, 0b101, 0b111, 0b001, 0b111]),
        _ => None,
    }
}

fn output_directory(
    paths: &AppPaths,
    request: &ExportEditSheetRequest,
    collection_name: &str,
) -> AppResult<PathBuf> {
    let run_name = format!("{}-{}", sanitize_name(collection_name), timestamp_suffix());
    let output_root = request
        .output_directory
        .as_ref()
        .map(|path| PathBuf::from(path.trim()).join(&run_name))
        .unwrap_or_else(|| {
            paths
                .root
                .join("sheet_exports")
                .join("static")
                .join(&run_name)
        });
    fs::create_dir_all(&output_root)?;
    Ok(output_root)
}

fn viewport_width(shape: &str, cell_width: i64) -> i64 {
    if shape == "horizontal_double" {
        cell_width * 2
    } else {
        cell_width
    }
}

fn viewport_height(shape: &str, cell_height: i64) -> i64 {
    if shape == "vertical_double" {
        cell_height * 2
    } else {
        cell_height
    }
}

fn normalized_background(value: &str) -> String {
    match value {
        "checker" | "white" | "black" => value.to_string(),
        _ => "transparent".to_string(),
    }
}

fn sanitize_name(value: &str) -> String {
    let name = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    if name.trim_matches('_').is_empty() {
        "sheet".to_string()
    } else {
        name
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(64);
    for byte in digest {
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}

fn timestamp_suffix() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

fn now_iso_like() -> String {
    format!("{}Z", timestamp_suffix())
}

fn default_sheet_source() -> String {
    "current_collection".to_string()
}

fn default_background() -> String {
    "transparent".to_string()
}

fn default_true() -> bool {
    true
}

fn default_max_sheet_size() -> i64 {
    2048
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::time::{SystemTime, UNIX_EPOCH};

    use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};
    use rusqlite::Connection;

    use crate::db::migrations;
    use crate::db::repositories::collections::create_collection;
    use crate::db::repositories::imports::import_image_files;
    use crate::models::ImportImageFilePayload;
    use crate::paths::AppPaths;

    use super::{export_edit_sheet, ExportEditSheetRequest, GuideLabelOptions};

    fn connection() -> Connection {
        let mut connection = Connection::open_in_memory().unwrap();
        migrations::run(&mut connection).unwrap();
        connection
    }

    fn temp_paths() -> AppPaths {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        AppPaths::prepare(std::env::temp_dir().join(format!("pmtconcon-sheet-export-{suffix}")))
            .unwrap()
    }

    fn png_bytes() -> Vec<u8> {
        let image = ImageBuffer::from_pixel(20, 20, Rgba([0, 255, 0, 96]));
        let mut cursor = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image)
            .write_to(&mut cursor, ImageFormat::Png)
            .unwrap();
        cursor.into_inner()
    }

    #[test]
    fn edit_sheet_export_writes_clean_guide_and_manifest_with_alpha() {
        let mut connection = connection();
        let paths = temp_paths();
        let collection =
            create_collection(&mut connection, Some("sheet export".to_string())).unwrap();
        import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![ImportImageFilePayload {
                original_filename: "cell.png".to_string(),
                bytes: png_bytes(),
            }],
        )
        .unwrap();

        let result = export_edit_sheet(
            &connection,
            &paths,
            ExportEditSheetRequest {
                collection_id: collection.id,
                selected_icon_ids: Vec::new(),
                source: "current_collection".to_string(),
                cell_width: 20,
                cell_height: 20,
                columns: 1,
                gap_x: 8,
                gap_y: 8,
                border_x: 16,
                border_y: 16,
                background: "transparent".to_string(),
                include_clean_sheet: true,
                include_guide_sheet: true,
                include_manifest: true,
                label_options: Some(GuideLabelOptions {
                    cell_number: true,
                    icon_name: false,
                    alt_value: false,
                    export_number: false,
                }),
                max_sheet_width: 2048,
                max_sheet_height: 2048,
                output_directory: None,
                open_output_folder: false,
            },
        )
        .unwrap();

        assert_eq!(result.item_count, 1);
        assert_eq!(result.page_count, 1);
        assert!(std::path::Path::new(result.manifest_path.as_ref().unwrap()).is_file());
        let clean = image::open(&result.clean_sheet_paths[0])
            .unwrap()
            .to_rgba8();
        assert_eq!((clean.width(), clean.height()), (52, 52));
        assert_eq!(clean.get_pixel(0, 0).0[3], 0);
        assert_eq!(clean.get_pixel(16, 16).0[3], 96);
        let guide = image::open(&result.guide_sheet_paths[0])
            .unwrap()
            .to_rgba8();
        assert_ne!(guide.get_pixel(16, 16), clean.get_pixel(16, 16));

        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn edit_sheet_export_selected_icons_only_in_grid_order() {
        let mut connection = connection();
        let paths = temp_paths();
        let collection =
            create_collection(&mut connection, Some("selected sheet export".to_string())).unwrap();
        let import_result = import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![
                ImportImageFilePayload {
                    original_filename: "first.png".to_string(),
                    bytes: png_bytes(),
                },
                ImportImageFilePayload {
                    original_filename: "second.png".to_string(),
                    bytes: png_bytes(),
                },
            ],
        )
        .unwrap();
        let second_id = import_result.imported_icons[1].id.clone();

        let result = export_edit_sheet(
            &connection,
            &paths,
            ExportEditSheetRequest {
                collection_id: collection.id,
                selected_icon_ids: vec![second_id],
                source: "selected_icons".to_string(),
                cell_width: 20,
                cell_height: 20,
                columns: 1,
                gap_x: 0,
                gap_y: 0,
                border_x: 0,
                border_y: 0,
                background: "transparent".to_string(),
                include_clean_sheet: true,
                include_guide_sheet: false,
                include_manifest: true,
                label_options: Some(GuideLabelOptions {
                    cell_number: true,
                    icon_name: true,
                    alt_value: true,
                    export_number: true,
                }),
                max_sheet_width: 2048,
                max_sheet_height: 2048,
                output_directory: None,
                open_output_folder: false,
            },
        )
        .unwrap();

        assert_eq!(result.item_count, 1);
        let manifest = std::fs::read_to_string(result.manifest_path.as_ref().unwrap()).unwrap();
        assert!(manifest.contains("second"));
        assert!(!manifest.contains("first"));

        std::fs::remove_dir_all(paths.root).unwrap();
    }
}
