use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::ids::create_id;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetGridPresetDto {
    pub id: String,
    pub name: String,
    pub scope: String,
    pub collection_id: Option<String>,
    pub kind: String,
    pub cell_width: i64,
    pub cell_height: i64,
    pub rows: Option<i64>,
    pub columns: Option<i64>,
    pub mode: String,
    pub gap_x: i64,
    pub gap_y: i64,
    pub border_left: i64,
    pub border_top: i64,
    pub border_right: i64,
    pub border_bottom: i64,
    pub read_order: String,
    pub background: String,
    pub max_sheet_width: i64,
    pub max_sheet_height: i64,
    pub frames_per_page: Option<i64>,
    pub include_clean_sheet: bool,
    pub include_guide_sheet: bool,
    pub include_manifest: bool,
    pub guide_label_options_json: String,
    pub is_default_for_import: bool,
    pub is_default_for_export: bool,
    pub is_default_for_gif_frame: bool,
    pub is_builtin: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetGridPresetInput {
    pub name: String,
    #[serde(default = "default_scope")]
    pub scope: String,
    pub collection_id: Option<String>,
    #[serde(default = "default_kind")]
    pub kind: String,
    pub cell_width: i64,
    pub cell_height: i64,
    pub rows: Option<i64>,
    pub columns: Option<i64>,
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default)]
    pub gap_x: i64,
    #[serde(default)]
    pub gap_y: i64,
    #[serde(default)]
    pub border_left: i64,
    #[serde(default)]
    pub border_top: i64,
    #[serde(default)]
    pub border_right: i64,
    #[serde(default)]
    pub border_bottom: i64,
    #[serde(default = "default_read_order")]
    pub read_order: String,
    #[serde(default = "default_background")]
    pub background: String,
    #[serde(default = "default_max_sheet_size")]
    pub max_sheet_width: i64,
    #[serde(default = "default_max_sheet_size")]
    pub max_sheet_height: i64,
    pub frames_per_page: Option<i64>,
    #[serde(default = "default_true")]
    pub include_clean_sheet: bool,
    #[serde(default = "default_true")]
    pub include_guide_sheet: bool,
    #[serde(default = "default_true")]
    pub include_manifest: bool,
    #[serde(default = "default_guide_options")]
    pub guide_label_options_json: String,
}

pub fn list_sheet_grid_presets(
    connection: &Connection,
    collection_id: Option<String>,
) -> AppResult<Vec<SheetGridPresetDto>> {
    let mut statement = connection.prepare(
        "SELECT *
         FROM sheet_grid_presets
         WHERE scope = 'global'
            OR collection_id = ?1
         ORDER BY is_builtin DESC, name COLLATE NOCASE ASC, created_at ASC",
    )?;
    let presets = statement
        .query_map(params![collection_id], preset_from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(presets)
}

pub fn create_sheet_grid_preset(
    connection: &Connection,
    input: SheetGridPresetInput,
) -> AppResult<SheetGridPresetDto> {
    let input = validate_input(input)?;
    let id = create_id("sheet_preset");
    connection.execute(
        "INSERT INTO sheet_grid_presets (
           id,
           name,
           scope,
           collection_id,
           kind,
           cell_width,
           cell_height,
           rows,
           columns,
           mode,
           gap_x,
           gap_y,
           border_left,
           border_top,
           border_right,
           border_bottom,
           read_order,
           background,
           max_sheet_width,
           max_sheet_height,
           frames_per_page,
           include_clean_sheet,
           include_guide_sheet,
           include_manifest,
           guide_label_options_json,
           created_at,
           updated_at
         )
         VALUES (
           ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
           ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20,
           ?21, ?22, ?23, ?24, ?25,
           strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
           strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         )",
        params![
            id,
            input.name,
            input.scope,
            input.collection_id,
            input.kind,
            input.cell_width,
            input.cell_height,
            input.rows,
            input.columns,
            input.mode,
            input.gap_x,
            input.gap_y,
            input.border_left,
            input.border_top,
            input.border_right,
            input.border_bottom,
            input.read_order,
            input.background,
            input.max_sheet_width,
            input.max_sheet_height,
            input.frames_per_page,
            bool_to_int(input.include_clean_sheet),
            bool_to_int(input.include_guide_sheet),
            bool_to_int(input.include_manifest),
            input.guide_label_options_json,
        ],
    )?;
    get_preset(connection, &id)
}

pub fn update_sheet_grid_preset(
    connection: &Connection,
    id: String,
    input: SheetGridPresetInput,
) -> AppResult<SheetGridPresetDto> {
    ensure_user_preset(
        connection,
        &id,
        "기본 제공 프리셋은 직접 수정할 수 없습니다.",
    )?;
    let input = validate_input(input)?;
    let changed = connection.execute(
        "UPDATE sheet_grid_presets
         SET name = ?1,
             scope = ?2,
             collection_id = ?3,
             kind = ?4,
             cell_width = ?5,
             cell_height = ?6,
             rows = ?7,
             columns = ?8,
             mode = ?9,
             gap_x = ?10,
             gap_y = ?11,
             border_left = ?12,
             border_top = ?13,
             border_right = ?14,
             border_bottom = ?15,
             read_order = ?16,
             background = ?17,
             max_sheet_width = ?18,
             max_sheet_height = ?19,
             frames_per_page = ?20,
             include_clean_sheet = ?21,
             include_guide_sheet = ?22,
             include_manifest = ?23,
             guide_label_options_json = ?24,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?25
           AND is_builtin = 0",
        params![
            input.name,
            input.scope,
            input.collection_id,
            input.kind,
            input.cell_width,
            input.cell_height,
            input.rows,
            input.columns,
            input.mode,
            input.gap_x,
            input.gap_y,
            input.border_left,
            input.border_top,
            input.border_right,
            input.border_bottom,
            input.read_order,
            input.background,
            input.max_sheet_width,
            input.max_sheet_height,
            input.frames_per_page,
            bool_to_int(input.include_clean_sheet),
            bool_to_int(input.include_guide_sheet),
            bool_to_int(input.include_manifest),
            input.guide_label_options_json,
            id,
        ],
    )?;
    if changed == 0 {
        return Err(AppError::not_found(
            "수정할 시트 프리셋을 찾을 수 없습니다.",
        ));
    }
    get_preset(connection, &id)
}

pub fn delete_sheet_grid_preset(connection: &Connection, id: String) -> AppResult<()> {
    ensure_user_preset(connection, &id, "기본 제공 프리셋은 삭제할 수 없습니다.")?;
    let changed = connection.execute(
        "DELETE FROM sheet_grid_presets
         WHERE id = ?1
           AND is_builtin = 0",
        params![id],
    )?;
    if changed == 0 {
        return Err(AppError::not_found(
            "삭제할 시트 프리셋을 찾을 수 없습니다.",
        ));
    }
    Ok(())
}

pub fn duplicate_sheet_grid_preset(
    connection: &Connection,
    id: String,
) -> AppResult<SheetGridPresetDto> {
    let original = get_preset(connection, &id)?;
    let copy_name = next_copy_name(
        connection,
        &original.name,
        original.collection_id.as_deref(),
    )?;
    create_sheet_grid_preset(
        connection,
        SheetGridPresetInput {
            name: copy_name,
            scope: original.scope,
            collection_id: original.collection_id,
            kind: original.kind,
            cell_width: original.cell_width,
            cell_height: original.cell_height,
            rows: original.rows,
            columns: original.columns,
            mode: original.mode,
            gap_x: original.gap_x,
            gap_y: original.gap_y,
            border_left: original.border_left,
            border_top: original.border_top,
            border_right: original.border_right,
            border_bottom: original.border_bottom,
            read_order: original.read_order,
            background: original.background,
            max_sheet_width: original.max_sheet_width,
            max_sheet_height: original.max_sheet_height,
            frames_per_page: original.frames_per_page,
            include_clean_sheet: original.include_clean_sheet,
            include_guide_sheet: original.include_guide_sheet,
            include_manifest: original.include_manifest,
            guide_label_options_json: original.guide_label_options_json,
        },
    )
}

pub fn set_default_sheet_grid_preset(
    connection: &Connection,
    id: String,
    target: String,
    collection_id: Option<String>,
) -> AppResult<SheetGridPresetDto> {
    let preset = get_preset(connection, &id)?;
    let effective_collection_id = collection_id.or(preset.collection_id.clone());
    let target_column = default_target_column(&target)?;
    connection.execute(
        &format!(
            "UPDATE sheet_grid_presets
             SET {target_column} = 0,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE COALESCE(collection_id, '') = COALESCE(?1, '')"
        ),
        params![effective_collection_id],
    )?;
    connection.execute(
        &format!(
            "UPDATE sheet_grid_presets
             SET {target_column} = 1,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?1"
        ),
        params![id],
    )?;
    get_preset(connection, &preset.id)
}

pub fn get_default_sheet_grid_preset(
    connection: &Connection,
    target: String,
    collection_id: Option<String>,
) -> AppResult<Option<SheetGridPresetDto>> {
    let target_column = default_target_column(&target)?;
    if let Some(collection_id) = collection_id {
        let collection_default = query_default(connection, target_column, Some(&collection_id))?;
        if collection_default.is_some() {
            return Ok(collection_default);
        }
    }
    query_default(connection, target_column, None)
}

fn query_default(
    connection: &Connection,
    target_column: &str,
    collection_id: Option<&str>,
) -> AppResult<Option<SheetGridPresetDto>> {
    let sql = format!(
        "SELECT *
         FROM sheet_grid_presets
         WHERE {target_column} = 1
           AND COALESCE(collection_id, '') = COALESCE(?1, '')
         ORDER BY is_builtin DESC, updated_at DESC
         LIMIT 1"
    );
    connection
        .query_row(&sql, params![collection_id], preset_from_row)
        .optional()
        .map_err(AppError::from)
}

fn get_preset(connection: &Connection, id: &str) -> AppResult<SheetGridPresetDto> {
    connection
        .query_row(
            "SELECT *
             FROM sheet_grid_presets
             WHERE id = ?1",
            params![id],
            preset_from_row,
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("시트 프리셋을 찾을 수 없습니다."))
}

fn ensure_user_preset(connection: &Connection, id: &str, builtin_message: &str) -> AppResult<()> {
    let is_builtin = connection
        .query_row(
            "SELECT is_builtin
             FROM sheet_grid_presets
             WHERE id = ?1",
            params![id],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("시트 프리셋을 찾을 수 없습니다."))?;
    if is_builtin == 1 {
        return Err(AppError::new("validation", builtin_message));
    }
    Ok(())
}

fn next_copy_name(
    connection: &Connection,
    original_name: &str,
    collection_id: Option<&str>,
) -> AppResult<String> {
    let base = format!("{original_name} 복사본");
    let names = existing_names(connection, collection_id)?;
    if !names.contains(&base) {
        return Ok(base);
    }
    for copy_number in 2..10_000 {
        let candidate = format!("{base} {copy_number}");
        if !names.contains(&candidate) {
            return Ok(candidate);
        }
    }
    Err(AppError::new(
        "validation",
        "복제할 프리셋 이름을 만들 수 없습니다.",
    ))
}

fn existing_names(connection: &Connection, collection_id: Option<&str>) -> AppResult<Vec<String>> {
    let mut statement = connection.prepare(
        "SELECT name
         FROM sheet_grid_presets
         WHERE COALESCE(collection_id, '') = COALESCE(?1, '')",
    )?;
    let names = statement
        .query_map(params![collection_id], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(names)
}

fn validate_input(mut input: SheetGridPresetInput) -> AppResult<SheetGridPresetInput> {
    input.name = input.name.trim().to_string();
    if input.name.is_empty() {
        return Err(AppError::new(
            "validation",
            "프리셋 이름을 입력해야 합니다.",
        ));
    }
    if !matches!(input.scope.as_str(), "global" | "collection") {
        return Err(AppError::new(
            "validation",
            "지원하지 않는 프리셋 범위입니다.",
        ));
    }
    if input.scope == "collection" && input.collection_id.as_deref().unwrap_or("").is_empty() {
        return Err(AppError::new(
            "validation",
            "모음 프리셋은 collection_id가 필요합니다.",
        ));
    }
    if !matches!(
        input.kind.as_str(),
        "static_import_export" | "static_import" | "static_export" | "gif_frame_export"
    ) {
        return Err(AppError::new(
            "validation",
            "지원하지 않는 프리셋 종류입니다.",
        ));
    }
    if !matches!(input.mode.as_str(), "rows_columns" | "cell_size") {
        return Err(AppError::new(
            "validation",
            "지원하지 않는 분할 모드입니다.",
        ));
    }
    if !matches!(input.read_order.as_str(), "row_major" | "column_major") {
        return Err(AppError::new(
            "validation",
            "지원하지 않는 읽기 순서입니다.",
        ));
    }
    if !matches!(
        input.background.as_str(),
        "transparent" | "checker" | "white" | "black"
    ) {
        return Err(AppError::new("validation", "지원하지 않는 배경입니다."));
    }
    if input.cell_width <= 0
        || input.cell_height <= 0
        || input.max_sheet_width <= 0
        || input.max_sheet_height <= 0
        || input.gap_x < 0
        || input.gap_y < 0
        || input.border_left < 0
        || input.border_top < 0
        || input.border_right < 0
        || input.border_bottom < 0
    {
        return Err(AppError::new(
            "validation",
            "프리셋 수치는 0 이상의 유효한 값이어야 합니다.",
        ));
    }
    if input.columns.is_some_and(|columns| columns <= 0)
        || input.rows.is_some_and(|rows| rows <= 0)
        || input.frames_per_page.is_some_and(|frames| frames <= 0)
    {
        return Err(AppError::new(
            "validation",
            "행/열/프레임 수는 1 이상이어야 합니다.",
        ));
    }
    Ok(input)
}

fn default_target_column(target: &str) -> AppResult<&'static str> {
    match target {
        "import" => Ok("is_default_for_import"),
        "export" => Ok("is_default_for_export"),
        "gif_frame" => Ok("is_default_for_gif_frame"),
        _ => Err(AppError::new(
            "validation",
            "지원하지 않는 기본 프리셋 대상입니다.",
        )),
    }
}

fn preset_from_row(row: &Row<'_>) -> rusqlite::Result<SheetGridPresetDto> {
    Ok(SheetGridPresetDto {
        id: row.get("id")?,
        name: row.get("name")?,
        scope: row.get("scope")?,
        collection_id: row.get("collection_id")?,
        kind: row.get("kind")?,
        cell_width: row.get("cell_width")?,
        cell_height: row.get("cell_height")?,
        rows: row.get("rows")?,
        columns: row.get("columns")?,
        mode: row.get("mode")?,
        gap_x: row.get("gap_x")?,
        gap_y: row.get("gap_y")?,
        border_left: row.get("border_left")?,
        border_top: row.get("border_top")?,
        border_right: row.get("border_right")?,
        border_bottom: row.get("border_bottom")?,
        read_order: row.get("read_order")?,
        background: row.get("background")?,
        max_sheet_width: row.get("max_sheet_width")?,
        max_sheet_height: row.get("max_sheet_height")?,
        frames_per_page: row.get("frames_per_page")?,
        include_clean_sheet: int_to_bool(row.get("include_clean_sheet")?),
        include_guide_sheet: int_to_bool(row.get("include_guide_sheet")?),
        include_manifest: int_to_bool(row.get("include_manifest")?),
        guide_label_options_json: row.get("guide_label_options_json")?,
        is_default_for_import: int_to_bool(row.get("is_default_for_import")?),
        is_default_for_export: int_to_bool(row.get("is_default_for_export")?),
        is_default_for_gif_frame: int_to_bool(row.get("is_default_for_gif_frame")?),
        is_builtin: int_to_bool(row.get("is_builtin")?),
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn int_to_bool(value: i64) -> bool {
    value != 0
}

fn bool_to_int(value: bool) -> i64 {
    if value {
        1
    } else {
        0
    }
}

fn default_scope() -> String {
    "collection".to_string()
}

fn default_kind() -> String {
    "static_import_export".to_string()
}

fn default_mode() -> String {
    "rows_columns".to_string()
}

fn default_read_order() -> String {
    "row_major".to_string()
}

fn default_background() -> String {
    "transparent".to_string()
}

fn default_max_sheet_size() -> i64 {
    2048
}

fn default_true() -> bool {
    true
}

fn default_guide_options() -> String {
    r#"{"cellNumber":true,"iconName":true,"altValue":true,"exportNumber":true}"#.to_string()
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use crate::db::migrations;
    use crate::db::repositories::collections::create_collection;

    use super::{
        create_sheet_grid_preset, delete_sheet_grid_preset, duplicate_sheet_grid_preset,
        get_default_sheet_grid_preset, list_sheet_grid_presets, set_default_sheet_grid_preset,
        SheetGridPresetInput,
    };

    fn connection() -> Connection {
        let mut connection = Connection::open_in_memory().unwrap();
        migrations::run(&mut connection).unwrap();
        connection
    }

    fn input(collection_id: &str, name: &str) -> SheetGridPresetInput {
        SheetGridPresetInput {
            name: name.to_string(),
            scope: "collection".to_string(),
            collection_id: Some(collection_id.to_string()),
            kind: "static_import_export".to_string(),
            cell_width: 128,
            cell_height: 128,
            rows: Some(2),
            columns: Some(4),
            mode: "rows_columns".to_string(),
            gap_x: 4,
            gap_y: 4,
            border_left: 8,
            border_top: 8,
            border_right: 8,
            border_bottom: 8,
            read_order: "row_major".to_string(),
            background: "transparent".to_string(),
            max_sheet_width: 2048,
            max_sheet_height: 2048,
            frames_per_page: None,
            include_clean_sheet: true,
            include_guide_sheet: true,
            include_manifest: true,
            guide_label_options_json:
                r#"{"cellNumber":true,"iconName":true,"altValue":false,"exportNumber":true}"#
                    .to_string(),
        }
    }

    #[test]
    fn built_in_presets_are_listed_and_not_deleted() {
        let connection = connection();

        let presets =
            list_sheet_grid_presets(&connection, Some("collection_1".to_string())).unwrap();

        assert!(presets
            .iter()
            .any(|preset| preset.id == "builtin_dcinside_200_5cols"));
        assert!(
            delete_sheet_grid_preset(&connection, "builtin_dcinside_200_5cols".to_string())
                .is_err()
        );
    }

    #[test]
    fn user_preset_persists_can_be_defaulted_and_duplicated() {
        let mut connection = connection();
        let collection =
            create_collection(&mut connection, Some("preset collection".to_string())).unwrap();
        let created =
            create_sheet_grid_preset(&connection, input(&collection.id, "QA preset")).unwrap();
        assert_eq!(created.cell_width, 128);

        let defaulted = set_default_sheet_grid_preset(
            &connection,
            created.id.clone(),
            "import".to_string(),
            Some(collection.id.clone()),
        )
        .unwrap();
        assert!(defaulted.is_default_for_import);

        let default = get_default_sheet_grid_preset(
            &connection,
            "import".to_string(),
            Some(collection.id.clone()),
        )
        .unwrap()
        .unwrap();
        assert_eq!(default.id, created.id);

        let duplicated = duplicate_sheet_grid_preset(&connection, created.id.clone()).unwrap();
        assert_ne!(duplicated.id, created.id);
        assert!(duplicated.name.contains("복사본"));
    }
}
