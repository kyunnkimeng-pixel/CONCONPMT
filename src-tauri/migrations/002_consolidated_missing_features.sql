ALTER TABLE icons
  ADD COLUMN thumbnail_override_source_file_id TEXT REFERENCES source_files(id);

ALTER TABLE icons
  ADD COLUMN thumbnail_override_path TEXT;

CREATE TABLE app_settings_new (
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

INSERT INTO app_settings_new (
  id,
  last_open_collection_id,
  last_view_mode,
  last_export_directory,
  locale,
  theme,
  created_at,
  updated_at
)
SELECT
  id,
  last_open_collection_id,
  CASE last_view_mode
    WHEN 'usagePreview' THEN 'usagePreview'
    WHEN 'list' THEN 'usagePreview'
    ELSE 'explorer'
  END,
  last_export_directory,
  locale,
  theme,
  created_at,
  strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
FROM app_settings;

DROP TABLE app_settings;
ALTER TABLE app_settings_new RENAME TO app_settings;
