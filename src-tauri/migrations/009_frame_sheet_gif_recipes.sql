CREATE TABLE frame_sheet_gif_recipes (
  id TEXT PRIMARY KEY,
  generated_icon_id TEXT NOT NULL UNIQUE REFERENCES icons(id) ON DELETE CASCADE,
  original_sheet_filename TEXT NOT NULL
    CHECK (length(trim(original_sheet_filename)) > 0),
  original_sheet_path TEXT NOT NULL
    CHECK (length(trim(original_sheet_path)) > 0),
  original_sheet_sha256 TEXT NOT NULL
    CHECK (length(original_sheet_sha256) = 64),
  recipe_schema TEXT NOT NULL
    CHECK (length(trim(recipe_schema)) > 0),
  grid_settings_json TEXT NOT NULL
    CHECK (json_valid(grid_settings_json) AND json_type(grid_settings_json) = 'object'),
  frames_json TEXT NOT NULL
    CHECK (json_valid(frames_json) AND json_type(frames_json) = 'array'),
  direction TEXT NOT NULL
    CHECK (direction IN ('forward', 'reverse', 'pingpong')),
  loop_mode TEXT NOT NULL
    CHECK (loop_mode IN ('once', 'infinite', 'count')),
  loop_count INTEGER,
  measured_byte_size INTEGER NOT NULL
    CHECK (measured_byte_size > 0),
  render_hash TEXT NOT NULL
    CHECK (length(render_hash) = 64),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  CHECK (
    (loop_mode = 'count' AND loop_count BETWEEN 1 AND 65535)
    OR
    (loop_mode IN ('once', 'infinite') AND loop_count IS NULL)
  )
);

CREATE INDEX idx_frame_sheet_gif_recipes_source_render
  ON frame_sheet_gif_recipes(original_sheet_sha256, render_hash);
