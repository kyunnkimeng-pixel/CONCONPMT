CREATE TABLE processed_asset_variants (
  id TEXT PRIMARY KEY,
  icon_id TEXT NOT NULL REFERENCES icons(id) ON DELETE CASCADE,
  piece_id TEXT REFERENCES icon_pieces(id) ON DELETE CASCADE,
  profile_id TEXT REFERENCES export_profiles(id) ON DELETE CASCADE,
  source_file_id TEXT,
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
  created_at TEXT NOT NULL,
  is_active_for_export INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_processed_variants_lookup
  ON processed_asset_variants(icon_id, piece_id, profile_id, source_hash, crop_hash, profile_hash, is_active_for_export);

CREATE INDEX idx_processed_variants_candidates
  ON processed_asset_variants(icon_id, piece_id, profile_id, created_at);

CREATE TABLE optimization_jobs (
  id TEXT PRIMARY KEY,
  icon_id TEXT NOT NULL REFERENCES icons(id) ON DELETE CASCADE,
  profile_id TEXT NOT NULL REFERENCES export_profiles(id) ON DELETE CASCADE,
  status TEXT NOT NULL
    CHECK (status IN ('queued', 'running', 'completed', 'failed', 'cancelled')),
  progress_current INTEGER NOT NULL DEFAULT 0,
  progress_total INTEGER NOT NULL DEFAULT 0,
  message TEXT,
  started_at TEXT,
  finished_at TEXT,
  error TEXT
);
