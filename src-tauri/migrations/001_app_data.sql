CREATE TABLE source_files (
  id TEXT PRIMARY KEY,
  original_filename TEXT NOT NULL,
  original_path_in_library TEXT NOT NULL UNIQUE,
  original_extension TEXT NOT NULL,
  mime_type TEXT NOT NULL,
  width INTEGER NOT NULL,
  height INTEGER NOT NULL,
  byte_size INTEGER NOT NULL,
  sha256 TEXT NOT NULL,
  is_animated INTEGER NOT NULL DEFAULT 0,
  frame_count INTEGER,
  original_loop_mode TEXT DEFAULT 'preserve'
    CHECK (original_loop_mode IN ('preserve', 'infinite', 'once', 'count')),
  original_loop_count INTEGER,
  imported_from_path TEXT,
  created_at TEXT NOT NULL
);

CREATE UNIQUE INDEX idx_source_files_sha256 ON source_files(sha256);

CREATE TABLE collections (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  cover_source_file_id TEXT REFERENCES source_files(id),
  cover_icon_id TEXT REFERENCES icons(id),
  default_cell_width INTEGER NOT NULL DEFAULT 200,
  default_cell_height INTEGER NOT NULL DEFAULT 200,
  preview_width INTEGER NOT NULL DEFAULT 100,
  preview_height INTEGER NOT NULL DEFAULT 100,
  export_format TEXT NOT NULL DEFAULT 'png'
    CHECK (export_format IN ('jpg', 'png', 'gif', 'source')),
  max_bytes INTEGER NOT NULL DEFAULT 2097152,
  allowed_formats_json TEXT NOT NULL DEFAULT '["jpg","jpeg","png","gif"]',
  order_index INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT
);

CREATE INDEX idx_collections_order
  ON collections(deleted_at, order_index, created_at);

CREATE TABLE icons (
  id TEXT PRIMARY KEY,
  collection_id TEXT NOT NULL REFERENCES collections(id),
  source_file_id TEXT NOT NULL REFERENCES source_files(id),
  display_name TEXT NOT NULL,
  shape TEXT NOT NULL DEFAULT 'single'
    CHECK (shape IN ('single', 'horizontal_double', 'vertical_double')),
  order_index INTEGER NOT NULL,
  cell_width_override INTEGER,
  cell_height_override INTEGER,
  thumbnail_path TEXT,
  current_preview_path TEXT,
  gif_loop_mode TEXT NOT NULL DEFAULT 'preserve'
    CHECK (gif_loop_mode IN ('preserve', 'infinite', 'once', 'count')),
  gif_loop_count INTEGER,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT
);

CREATE INDEX idx_icons_collection_order
  ON icons(collection_id, deleted_at, order_index, created_at);

CREATE TABLE crop_settings (
  id TEXT PRIMARY KEY,
  icon_id TEXT NOT NULL UNIQUE REFERENCES icons(id) ON DELETE CASCADE,
  crop_mode TEXT NOT NULL DEFAULT 'free'
    CHECK (crop_mode IN ('free', 'fixed')),
  crop_x REAL NOT NULL,
  crop_y REAL NOT NULL,
  crop_w REAL NOT NULL,
  crop_h REAL NOT NULL,
  preset_position TEXT NOT NULL DEFAULT 'center'
    CHECK (preset_position IN (
      'center',
      'top_left',
      'top',
      'top_right',
      'left',
      'right',
      'bottom_left',
      'bottom',
      'bottom_right',
      'custom'
    )),
  source_width_at_apply INTEGER,
  source_height_at_apply INTEGER,
  viewport_width_at_apply INTEGER NOT NULL,
  viewport_height_at_apply INTEGER NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE icon_pieces (
  id TEXT PRIMARY KEY,
  icon_id TEXT NOT NULL REFERENCES icons(id) ON DELETE CASCADE,
  piece_index INTEGER NOT NULL,
  piece_role TEXT NOT NULL
    CHECK (piece_role IN ('single', 'left', 'right', 'top', 'bottom')),
  alt_text TEXT NOT NULL DEFAULT '',
  generated_preview_path TEXT,
  last_export_path TEXT,
  export_status TEXT NOT NULL DEFAULT 'not_exported'
    CHECK (export_status IN ('not_exported', 'ready', 'warning', 'error')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE(icon_id, piece_index)
);

CREATE INDEX idx_icon_pieces_icon_order
  ON icon_pieces(icon_id, piece_index);

CREATE TABLE export_profiles (
  id TEXT PRIMARY KEY,
  collection_id TEXT NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  profile_type TEXT NOT NULL
    CHECK (profile_type IN ('dcinside', 'custom')),
  target_format TEXT NOT NULL DEFAULT 'png'
    CHECK (target_format IN ('jpg', 'png', 'gif', 'source')),
  target_cell_width INTEGER NOT NULL DEFAULT 200,
  target_cell_height INTEGER NOT NULL DEFAULT 200,
  preview_width INTEGER NOT NULL DEFAULT 100,
  preview_height INTEGER NOT NULL DEFAULT 100,
  max_bytes INTEGER NOT NULL DEFAULT 2097152,
  allowed_formats_json TEXT NOT NULL DEFAULT '["jpg","jpeg","png","gif"]',
  filename_mode TEXT NOT NULL DEFAULT 'sequence'
    CHECK (filename_mode IN ('sequence', 'alt')),
  include_alt_txt INTEGER NOT NULL DEFAULT 1,
  strict_warnings INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE(collection_id, name)
);

CREATE TABLE app_settings (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  last_open_collection_id TEXT REFERENCES collections(id),
  last_view_mode TEXT NOT NULL DEFAULT 'explorer'
    CHECK (last_view_mode IN ('explorer', 'usagePreview')),
  last_export_directory TEXT,
  locale TEXT NOT NULL DEFAULT 'ko-KR',
  theme TEXT NOT NULL DEFAULT 'system'
    CHECK (theme IN ('system', 'light', 'dark')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

INSERT INTO app_settings (id, created_at, updated_at)
VALUES (
  1,
  strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
  strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
);
