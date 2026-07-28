CREATE TABLE processed_asset_variants_v2 (
  id TEXT PRIMARY KEY,
  icon_id TEXT NOT NULL REFERENCES icons(id) ON DELETE CASCADE,
  piece_id TEXT REFERENCES icon_pieces(id) ON DELETE CASCADE,
  profile_id TEXT REFERENCES export_profiles(id) ON DELETE CASCADE,
  source_file_id TEXT REFERENCES source_files(id) ON DELETE RESTRICT,
  kind TEXT NOT NULL
    CHECK (kind IN ('baseline_export', 'optimized_gif', 'optimized_png', 'optimized_jpg', 'final_export')),
  preset TEXT
    CHECK (preset IS NULL OR preset IN ('quality', 'balanced', 'smallest', 'custom', 'rescale_only', 'baseline')),
  path TEXT NOT NULL,
  format TEXT NOT NULL
    CHECK (format IN ('gif', 'png', 'jpg', 'jpeg')),
  width INTEGER NOT NULL,
  height INTEGER NOT NULL,
  byte_size INTEGER NOT NULL,
  frame_count INTEGER,
  duration_ms INTEGER,
  loop_mode TEXT,
  settings_json TEXT NOT NULL,
  source_hash TEXT NOT NULL,
  crop_hash TEXT NOT NULL,
  profile_hash TEXT NOT NULL,
  settings_hash TEXT NOT NULL,
  output_sha256 TEXT
    CHECK (output_sha256 IS NULL OR length(output_sha256) = 64),
  created_at TEXT NOT NULL,
  is_active_for_export INTEGER NOT NULL DEFAULT 0
    CHECK (is_active_for_export IN (0, 1))
);

INSERT INTO processed_asset_variants_v2 (
  id,
  icon_id,
  piece_id,
  profile_id,
  source_file_id,
  kind,
  preset,
  path,
  format,
  width,
  height,
  byte_size,
  frame_count,
  duration_ms,
  loop_mode,
  settings_json,
  source_hash,
  crop_hash,
  profile_hash,
  settings_hash,
  output_sha256,
  created_at,
  is_active_for_export
)
SELECT
  variant.id,
  variant.icon_id,
  variant.piece_id,
  variant.profile_id,
  CASE
    WHEN explicit_source.id IS NOT NULL
      AND explicit_source.sha256 = variant.source_hash
      THEN explicit_source.id
    WHEN variant.source_file_id IS NULL
      AND original_source.sha256 = variant.source_hash
      THEN original_source.id
    ELSE NULL
  END,
  variant.kind,
  variant.preset,
  variant.path,
  variant.format,
  variant.width,
  variant.height,
  variant.byte_size,
  variant.frame_count,
  variant.duration_ms,
  variant.loop_mode,
  variant.settings_json,
  variant.source_hash,
  variant.crop_hash,
  variant.profile_hash,
  variant.settings_hash,
  NULL,
  variant.created_at,
  CASE
    WHEN (
      (explicit_source.id IS NOT NULL AND explicit_source.sha256 = variant.source_hash)
      OR (
        variant.source_file_id IS NULL
        AND original_source.sha256 = variant.source_hash
      )
    )
    THEN variant.is_active_for_export
    ELSE 0
  END
FROM processed_asset_variants variant
JOIN icons icon ON icon.id = variant.icon_id
JOIN source_files original_source ON original_source.id = icon.source_file_id
LEFT JOIN source_files explicit_source ON explicit_source.id = variant.source_file_id;

DROP TABLE processed_asset_variants;
ALTER TABLE processed_asset_variants_v2 RENAME TO processed_asset_variants;

CREATE INDEX idx_processed_variants_lookup
  ON processed_asset_variants(
    icon_id,
    piece_id,
    profile_id,
    source_file_id,
    source_hash,
    crop_hash,
    profile_hash,
    is_active_for_export
  );

CREATE INDEX idx_processed_variants_candidates
  ON processed_asset_variants(icon_id, piece_id, profile_id, created_at);
