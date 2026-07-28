CREATE TABLE ai_icon_root_creations (
  creation_order INTEGER PRIMARY KEY AUTOINCREMENT,
  icon_id TEXT NOT NULL UNIQUE REFERENCES icons(id) ON DELETE CASCADE,
  source_icon_id TEXT REFERENCES icons(id) ON DELETE SET NULL,
  candidate_id TEXT NOT NULL REFERENCES ai_candidates(id) ON DELETE RESTRICT,
  normalization_recipe_hash TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE INDEX idx_ai_icon_root_creations_candidate
  ON ai_icon_root_creations(candidate_id, creation_order DESC);

CREATE INDEX idx_ai_icon_root_creations_source
  ON ai_icon_root_creations(source_icon_id, creation_order DESC);
