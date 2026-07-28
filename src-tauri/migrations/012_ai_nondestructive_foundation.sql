ALTER TABLE source_files
  ADD COLUMN has_alpha INTEGER
  CHECK (has_alpha IS NULL OR has_alpha IN (0, 1));

ALTER TABLE icons
  ADD COLUMN original_lineage_id TEXT NOT NULL DEFAULT '';

ALTER TABLE icons
  ADD COLUMN original_lineage_generation INTEGER NOT NULL DEFAULT 0
  CHECK (original_lineage_generation >= 0);

UPDATE icons
SET original_lineage_id = 'lineage_' || lower(hex(randomblob(16)))
WHERE trim(original_lineage_id) = '';

CREATE UNIQUE INDEX idx_icons_current_original_lineage
  ON icons(original_lineage_id)
  WHERE original_lineage_id <> '';

CREATE TABLE ai_requests (
  id TEXT PRIMARY KEY,
  origin_collection_id TEXT REFERENCES collections(id) ON DELETE SET NULL,
  origin_icon_id TEXT REFERENCES icons(id) ON DELETE SET NULL,
  origin_collection_name_snapshot TEXT,
  origin_icon_name_snapshot TEXT,
  provider_mode TEXT NOT NULL
    CHECK (provider_mode IN ('manual_web', 'api', 'local_endpoint')),
  service_surface TEXT NOT NULL
    CHECK (service_surface IN (
      'novelai_api', 'novelai_web', 'openai_api', 'chatgpt_web',
      'gemini_api', 'gemini_web', 'loopback_endpoint', 'other_manual'
    )),
  provider TEXT NOT NULL,
  adapter_id TEXT NOT NULL,
  adapter_contract_version TEXT NOT NULL,
  account_context TEXT NOT NULL DEFAULT 'unknown'
    CHECK (account_context IN ('personal', 'business_workspace', 'work_school', 'unknown')),
  model TEXT,
  operation TEXT NOT NULL,
  provenance_trust TEXT NOT NULL
    CHECK (provenance_trust IN ('api_verified', 'manual_declared', 'manual_unverified')),
  credential_mode_snapshot TEXT NOT NULL DEFAULT 'none'
    CHECK (credential_mode_snapshot IN ('none', 'session', 'environment')),
  capability_snapshot_json TEXT NOT NULL DEFAULT '{}'
    CHECK (json_valid(capability_snapshot_json) AND length(capability_snapshot_json) <= 65536),
  data_tier_snapshot_json TEXT NOT NULL DEFAULT '{}'
    CHECK (json_valid(data_tier_snapshot_json) AND length(data_tier_snapshot_json) <= 65536),
  retention_snapshot_json TEXT NOT NULL DEFAULT '{}'
    CHECK (json_valid(retention_snapshot_json) AND length(retention_snapshot_json) <= 65536),
  consent_snapshot_json TEXT NOT NULL DEFAULT '{}'
    CHECK (json_valid(consent_snapshot_json) AND length(consent_snapshot_json) <= 65536),
  policy_refs_json TEXT NOT NULL DEFAULT '[]'
    CHECK (json_valid(policy_refs_json) AND length(policy_refs_json) <= 65536),
  prompt_options_snapshot_json TEXT NOT NULL DEFAULT '{}'
    CHECK (json_valid(prompt_options_snapshot_json) AND length(prompt_options_snapshot_json) <= 65536),
  input_package_sha256 TEXT,
  mask_package_sha256 TEXT,
  reference_package_sha256 TEXT,
  original_lineage_id TEXT NOT NULL,
  original_lineage_generation INTEGER NOT NULL CHECK (original_lineage_generation >= 0),
  original_source_sha256 TEXT NOT NULL,
  effective_source_sha256 TEXT NOT NULL,
  payload_input_signature TEXT NOT NULL,
  request_recipe_signature TEXT NOT NULL,
  activation_revision INTEGER NOT NULL CHECK (activation_revision >= 0),
  status TEXT NOT NULL
    CHECK (status IN (
      'prepared', 'awaiting_result', 'running', 'completed', 'failed',
      'cancelled', 'expired'
    )),
  provider_request_id TEXT,
  provider_usage_json TEXT
    CHECK (provider_usage_json IS NULL OR (
      json_valid(provider_usage_json) AND length(provider_usage_json) <= 65536
    )),
  estimated_provider_units REAL,
  estimated_cost REAL,
  provider_reported_cost REAL,
  error_code TEXT,
  error_message TEXT CHECK (error_message IS NULL OR length(error_message) <= 4096),
  superseded_at TEXT,
  superseded_reason TEXT CHECK (superseded_reason IS NULL OR length(superseded_reason) <= 1024),
  metadata_scrubbed_at TEXT,
  expires_at TEXT,
  started_at TEXT,
  completed_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX idx_ai_requests_origin_icon
  ON ai_requests(origin_icon_id, created_at);

CREATE INDEX idx_ai_requests_status
  ON ai_requests(status, expires_at, created_at);

CREATE TABLE ai_candidates (
  id TEXT PRIMARY KEY,
  request_id TEXT NOT NULL REFERENCES ai_requests(id) ON DELETE RESTRICT,
  candidate_index INTEGER NOT NULL CHECK (candidate_index >= 0),
  raw_source_file_id TEXT NOT NULL REFERENCES source_files(id) ON DELETE RESTRICT,
  raw_source_sha256 TEXT NOT NULL,
  output_format TEXT NOT NULL CHECK (output_format IN ('jpg', 'jpeg', 'png', 'gif')),
  width INTEGER NOT NULL CHECK (width > 0),
  height INTEGER NOT NULL CHECK (height > 0),
  is_animated INTEGER NOT NULL CHECK (is_animated IN (0, 1)),
  has_alpha INTEGER NOT NULL CHECK (has_alpha IN (0, 1)),
  provider_capabilities_snapshot_json TEXT NOT NULL DEFAULT '{}'
    CHECK (
      json_valid(provider_capabilities_snapshot_json)
      AND length(provider_capabilities_snapshot_json) <= 65536
    ),
  created_at TEXT NOT NULL,
  UNIQUE(request_id, candidate_index)
);

CREATE INDEX idx_ai_candidates_source
  ON ai_candidates(raw_source_file_id, created_at);

CREATE TABLE icon_ai_versions (
  id TEXT PRIMARY KEY,
  icon_id TEXT NOT NULL REFERENCES icons(id) ON DELETE CASCADE,
  candidate_id TEXT NOT NULL REFERENCES ai_candidates(id) ON DELETE RESTRICT,
  base_original_source_file_id TEXT NOT NULL REFERENCES source_files(id) ON DELETE RESTRICT,
  base_original_lineage_id TEXT NOT NULL,
  base_original_lineage_generation INTEGER NOT NULL
    CHECK (base_original_lineage_generation >= 0),
  parent_version_id TEXT,
  effective_source_file_id TEXT NOT NULL REFERENCES source_files(id) ON DELETE RESTRICT,
  input_stage TEXT NOT NULL
    CHECK (input_stage IN ('base_source', 'rendered_viewport', 'gif_poster')),
  apply_kind TEXT NOT NULL CHECK (apply_kind IN ('active_source', 'new_icon_root')),
  provider_native_width INTEGER NOT NULL CHECK (provider_native_width > 0),
  provider_native_height INTEGER NOT NULL CHECK (provider_native_height > 0),
  target_canvas_width INTEGER NOT NULL CHECK (target_canvas_width > 0),
  target_canvas_height INTEGER NOT NULL CHECK (target_canvas_height > 0),
  normalization_recipe_json TEXT NOT NULL
    CHECK (json_valid(normalization_recipe_json) AND length(normalization_recipe_json) <= 65536),
  normalization_recipe_hash TEXT NOT NULL,
  canvas_kind TEXT NOT NULL DEFAULT 'source',
  animation_kind TEXT NOT NULL DEFAULT 'preserve',
  payload_input_signature TEXT NOT NULL,
  created_at TEXT NOT NULL,
  UNIQUE(icon_id, id),
  UNIQUE(icon_id, base_original_lineage_id, base_original_lineage_generation, id),
  FOREIGN KEY (
    icon_id,
    base_original_lineage_id,
    base_original_lineage_generation,
    parent_version_id
  ) REFERENCES icon_ai_versions (
    icon_id,
    base_original_lineage_id,
    base_original_lineage_generation,
    id
  ) DEFERRABLE INITIALLY DEFERRED
);

CREATE INDEX idx_icon_ai_versions_history
  ON icon_ai_versions(icon_id, created_at, id);

CREATE TABLE icon_ai_state (
  icon_id TEXT PRIMARY KEY REFERENCES icons(id) ON DELETE CASCADE,
  active_version_id TEXT,
  revision INTEGER NOT NULL DEFAULT 0 CHECK (revision >= 0),
  updated_at TEXT NOT NULL,
  FOREIGN KEY (icon_id, active_version_id)
    REFERENCES icon_ai_versions(icon_id, id)
    DEFERRABLE INITIALLY DEFERRED
);

INSERT INTO icon_ai_state (icon_id, active_version_id, revision, updated_at)
SELECT id, NULL, 0, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
FROM icons;

CREATE TRIGGER trg_icons_ai_state_after_insert
AFTER INSERT ON icons
BEGIN
  UPDATE icons
  SET original_lineage_id = 'lineage_' || lower(hex(randomblob(16)))
  WHERE id = NEW.id
    AND trim(original_lineage_id) = '';

  INSERT INTO icon_ai_state (icon_id, active_version_id, revision, updated_at)
  VALUES (NEW.id, NULL, 0, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
END;

CREATE TRIGGER trg_icons_lineage_nonempty_before_update
BEFORE UPDATE OF original_lineage_id ON icons
WHEN trim(NEW.original_lineage_id) = ''
BEGIN
  SELECT RAISE(ABORT, 'icon original lineage must not be empty');
END;

CREATE TRIGGER trg_icons_lineage_monotonic_before_update
BEFORE UPDATE OF source_file_id, original_lineage_id, original_lineage_generation ON icons
WHEN OLD.original_lineage_id <> '' AND (
  NEW.original_lineage_generation < OLD.original_lineage_generation
  OR (
    NEW.original_lineage_id = OLD.original_lineage_id
    AND NEW.original_lineage_generation <> OLD.original_lineage_generation
  )
  OR (
    NEW.original_lineage_id <> OLD.original_lineage_id
    AND NEW.original_lineage_generation <= OLD.original_lineage_generation
  )
  OR (
    NEW.source_file_id <> OLD.source_file_id
    AND (
      NEW.original_lineage_id = OLD.original_lineage_id
      OR NEW.original_lineage_generation <= OLD.original_lineage_generation
    )
  )
)
BEGIN
  SELECT RAISE(ABORT, 'icon original lineage must advance monotonically');
END;

CREATE TRIGGER trg_ai_candidate_source_guard_before_insert
BEFORE INSERT ON ai_candidates
WHEN NOT EXISTS (
  SELECT 1
  FROM source_files s
  WHERE s.id = NEW.raw_source_file_id
    AND s.sha256 = NEW.raw_source_sha256
    AND s.width = NEW.width
    AND s.height = NEW.height
    AND s.is_animated = NEW.is_animated
    AND s.has_alpha = NEW.has_alpha
)
BEGIN
  SELECT RAISE(ABORT, 'AI candidate source metadata mismatch');
END;

CREATE TRIGGER trg_ai_candidates_immutable_before_update
BEFORE UPDATE ON ai_candidates
BEGIN
  SELECT RAISE(ABORT, 'AI candidates are immutable');
END;

CREATE TRIGGER trg_icon_ai_version_lineage_guard_before_insert
BEFORE INSERT ON icon_ai_versions
WHEN NOT EXISTS (
  SELECT 1
  FROM icons i
  WHERE i.id = NEW.icon_id
    AND i.source_file_id = NEW.base_original_source_file_id
    AND i.original_lineage_id = NEW.base_original_lineage_id
    AND i.original_lineage_generation = NEW.base_original_lineage_generation
)
BEGIN
  SELECT RAISE(ABORT, 'AI version base lineage mismatch');
END;

CREATE TRIGGER trg_icon_ai_versions_immutable_before_update
BEFORE UPDATE ON icon_ai_versions
BEGIN
  SELECT RAISE(ABORT, 'AI icon versions are immutable');
END;

CREATE TRIGGER trg_icon_ai_state_revision_before_update
BEFORE UPDATE ON icon_ai_state
WHEN NEW.revision < OLD.revision
  OR (NEW.active_version_id IS NOT OLD.active_version_id AND NEW.revision <= OLD.revision)
BEGIN
  SELECT RAISE(ABORT, 'AI activation revision must advance');
END;

CREATE TRIGGER trg_icon_ai_state_lineage_guard_before_update
BEFORE UPDATE OF active_version_id ON icon_ai_state
WHEN NEW.active_version_id IS NOT NULL
  AND NOT EXISTS (
    SELECT 1
    FROM icon_ai_versions v
    JOIN icons i ON i.id = NEW.icon_id
    WHERE v.id = NEW.active_version_id
      AND v.icon_id = NEW.icon_id
      AND v.base_original_source_file_id = i.source_file_id
      AND v.base_original_lineage_id = i.original_lineage_id
      AND v.base_original_lineage_generation = i.original_lineage_generation
  )
BEGIN
  SELECT RAISE(ABORT, 'AI active version lineage mismatch');
END;

CREATE VIEW effective_visual_sources AS
SELECT
  i.id AS icon_id,
  i.collection_id,
  i.source_file_id AS original_source_file_id,
  i.original_lineage_id,
  i.original_lineage_generation,
  st.active_version_id,
  v.candidate_id AS active_candidate_id,
  COALESCE(v.effective_source_file_id, i.source_file_id) AS effective_source_file_id,
  v.normalization_recipe_hash,
  st.revision AS activation_revision,
  original.sha256 AS original_source_sha256,
  render.sha256 AS effective_source_sha256,
  render.original_path_in_library AS effective_source_path,
  render.original_extension AS effective_source_extension,
  render.mime_type AS effective_mime_type,
  render.width AS effective_width,
  render.height AS effective_height,
  render.byte_size AS effective_byte_size,
  render.has_alpha AS effective_has_alpha,
  render.is_animated AS effective_is_animated,
  render.frame_count AS effective_frame_count,
  render.original_loop_mode AS effective_loop_mode,
  render.original_loop_count AS effective_loop_count
FROM icons i
JOIN icon_ai_state st ON st.icon_id = i.id
LEFT JOIN icon_ai_versions v
  ON v.id = st.active_version_id
 AND v.icon_id = i.id
 AND v.base_original_source_file_id = i.source_file_id
 AND v.base_original_lineage_id = i.original_lineage_id
 AND v.base_original_lineage_generation = i.original_lineage_generation
JOIN source_files original ON original.id = i.source_file_id
JOIN source_files render
  ON render.id = COALESCE(v.effective_source_file_id, i.source_file_id)
WHERE st.active_version_id IS NULL OR v.id IS NOT NULL;
