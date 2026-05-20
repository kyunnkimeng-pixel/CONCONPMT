CREATE TABLE icon_notes (
  icon_id TEXT PRIMARY KEY REFERENCES icons(id) ON DELETE CASCADE,
  note TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE sheet_grid_presets (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  scope TEXT NOT NULL
    CHECK (scope IN ('global', 'collection')),
  collection_id TEXT REFERENCES collections(id),
  kind TEXT NOT NULL
    CHECK (kind IN ('static_import_export', 'static_import', 'static_export', 'gif_frame_export')),
  cell_width INTEGER NOT NULL,
  cell_height INTEGER NOT NULL,
  rows INTEGER,
  columns INTEGER,
  mode TEXT NOT NULL
    CHECK (mode IN ('rows_columns', 'cell_size')),
  gap_x INTEGER NOT NULL,
  gap_y INTEGER NOT NULL,
  border_left INTEGER NOT NULL,
  border_top INTEGER NOT NULL,
  border_right INTEGER NOT NULL,
  border_bottom INTEGER NOT NULL,
  read_order TEXT NOT NULL
    CHECK (read_order IN ('row_major', 'column_major')),
  background TEXT NOT NULL
    CHECK (background IN ('transparent', 'checker', 'white', 'black')),
  max_sheet_width INTEGER NOT NULL,
  max_sheet_height INTEGER NOT NULL,
  frames_per_page INTEGER,
  include_clean_sheet INTEGER NOT NULL DEFAULT 1
    CHECK (include_clean_sheet IN (0, 1)),
  include_guide_sheet INTEGER NOT NULL DEFAULT 1
    CHECK (include_guide_sheet IN (0, 1)),
  include_manifest INTEGER NOT NULL DEFAULT 1
    CHECK (include_manifest IN (0, 1)),
  guide_label_options_json TEXT NOT NULL,
  is_default_for_import INTEGER NOT NULL DEFAULT 0
    CHECK (is_default_for_import IN (0, 1)),
  is_default_for_export INTEGER NOT NULL DEFAULT 0
    CHECK (is_default_for_export IN (0, 1)),
  is_default_for_gif_frame INTEGER NOT NULL DEFAULT 0
    CHECK (is_default_for_gif_frame IN (0, 1)),
  is_builtin INTEGER NOT NULL DEFAULT 0
    CHECK (is_builtin IN (0, 1)),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX idx_sheet_grid_presets_scope
  ON sheet_grid_presets(scope, collection_id, kind, name);

INSERT INTO sheet_grid_presets (
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
  is_default_for_import,
  is_default_for_export,
  is_default_for_gif_frame,
  is_builtin,
  created_at,
  updated_at
)
VALUES
(
  'builtin_dcinside_200_5cols',
  'DCInside 200x200 / 5 columns',
  'global',
  NULL,
  'static_import_export',
  200,
  200,
  NULL,
  5,
  'rows_columns',
  8,
  8,
  16,
  16,
  16,
  16,
  'row_major',
  'transparent',
  2048,
  2048,
  NULL,
  1,
  1,
  1,
  '{"cellNumber":true,"iconName":true,"altValue":true,"exportNumber":true}',
  1,
  1,
  0,
  1,
  strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
  strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
),
(
  'builtin_gif_frames_200_8cols_64',
  'GIF Frames 200x200 / 8 columns / 64 frames',
  'global',
  NULL,
  'gif_frame_export',
  200,
  200,
  NULL,
  8,
  'rows_columns',
  8,
  8,
  16,
  16,
  16,
  16,
  'row_major',
  'transparent',
  2048,
  2048,
  64,
  1,
  1,
  1,
  '{"cellNumber":true,"iconName":false,"altValue":false,"exportNumber":true}',
  0,
  0,
  1,
  1,
  strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
  strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
);
