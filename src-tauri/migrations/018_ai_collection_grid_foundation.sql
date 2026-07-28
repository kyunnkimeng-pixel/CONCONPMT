-- This migration rebuilds tables that are parents of RESTRICT foreign keys.
-- migrations.rs runs this file in its guarded foreign-key-off migration path,
-- performs PRAGMA foreign_key_check before commit, and restores the prior
-- foreign_keys setting on every exit path.

CREATE TABLE ai_requests_v2 (
  id TEXT PRIMARY KEY,
  request_scope TEXT NOT NULL DEFAULT 'icon_edit'
    CHECK (request_scope IN (
      'icon_edit', 'grid_edit', 'single_generate', 'grid_generate'
    )),
  retry_of_request_id TEXT
    REFERENCES ai_requests_v2(id) ON DELETE RESTRICT,
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
  original_lineage_id TEXT,
  original_lineage_generation INTEGER
    CHECK (original_lineage_generation IS NULL OR original_lineage_generation >= 0),
  original_source_sha256 TEXT,
  effective_source_sha256 TEXT,
  payload_input_signature TEXT NOT NULL,
  request_recipe_signature TEXT,
  activation_revision INTEGER
    CHECK (activation_revision IS NULL OR activation_revision >= 0),
  status TEXT NOT NULL
    CHECK (status IN (
      'draft', 'prepared', 'awaiting_result', 'running',
      'layout_review_pending', 'completed', 'failed',
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
  updated_at TEXT NOT NULL,
  UNIQUE(id, request_scope),
  CHECK (retry_of_request_id IS NULL OR retry_of_request_id <> id),
  CHECK (
    (
      request_scope = 'icon_edit'
      AND original_lineage_id IS NOT NULL
      AND original_lineage_generation IS NOT NULL
      AND original_source_sha256 IS NOT NULL
      AND effective_source_sha256 IS NOT NULL
      AND request_recipe_signature IS NOT NULL
      AND activation_revision IS NOT NULL
    )
    OR
    (
      request_scope IN ('grid_edit', 'single_generate', 'grid_generate')
      AND origin_icon_id IS NULL
      AND origin_icon_name_snapshot IS NULL
      AND original_lineage_id IS NULL
      AND original_lineage_generation IS NULL
      AND original_source_sha256 IS NULL
      AND effective_source_sha256 IS NULL
      AND request_recipe_signature IS NULL
      AND activation_revision IS NULL
    )
  )
);

INSERT INTO ai_requests_v2 (
  id,
  request_scope,
  retry_of_request_id,
  origin_collection_id,
  origin_icon_id,
  origin_collection_name_snapshot,
  origin_icon_name_snapshot,
  provider_mode,
  service_surface,
  provider,
  adapter_id,
  adapter_contract_version,
  account_context,
  model,
  operation,
  provenance_trust,
  credential_mode_snapshot,
  capability_snapshot_json,
  data_tier_snapshot_json,
  retention_snapshot_json,
  consent_snapshot_json,
  policy_refs_json,
  prompt_options_snapshot_json,
  input_package_sha256,
  mask_package_sha256,
  reference_package_sha256,
  original_lineage_id,
  original_lineage_generation,
  original_source_sha256,
  effective_source_sha256,
  payload_input_signature,
  request_recipe_signature,
  activation_revision,
  status,
  provider_request_id,
  provider_usage_json,
  estimated_provider_units,
  estimated_cost,
  provider_reported_cost,
  error_code,
  error_message,
  superseded_at,
  superseded_reason,
  metadata_scrubbed_at,
  expires_at,
  started_at,
  completed_at,
  created_at,
  updated_at
)
SELECT
  id,
  'icon_edit',
  NULL,
  origin_collection_id,
  origin_icon_id,
  origin_collection_name_snapshot,
  origin_icon_name_snapshot,
  provider_mode,
  service_surface,
  provider,
  adapter_id,
  adapter_contract_version,
  account_context,
  model,
  operation,
  provenance_trust,
  credential_mode_snapshot,
  capability_snapshot_json,
  data_tier_snapshot_json,
  retention_snapshot_json,
  consent_snapshot_json,
  policy_refs_json,
  prompt_options_snapshot_json,
  input_package_sha256,
  mask_package_sha256,
  reference_package_sha256,
  original_lineage_id,
  original_lineage_generation,
  original_source_sha256,
  effective_source_sha256,
  payload_input_signature,
  request_recipe_signature,
  activation_revision,
  status,
  provider_request_id,
  provider_usage_json,
  estimated_provider_units,
  estimated_cost,
  provider_reported_cost,
  error_code,
  error_message,
  superseded_at,
  superseded_reason,
  metadata_scrubbed_at,
  expires_at,
  started_at,
  completed_at,
  created_at,
  updated_at
FROM ai_requests;

DROP TRIGGER IF EXISTS trg_ai_web_handoff_request_guard_before_insert;
DROP TRIGGER IF EXISTS trg_ai_web_handoff_snapshots_immutable_before_update;
DROP TRIGGER IF EXISTS trg_ai_web_handoff_extension_once_before_update;
DROP TRIGGER IF EXISTS trg_ai_web_handoff_result_guard_before_update;
DROP TRIGGER IF EXISTS trg_ai_web_handoff_cleanup_request_once_before_update;
DROP TRIGGER IF EXISTS trg_ai_web_handoff_payload_delete_once_before_update;

DROP TABLE ai_requests;
ALTER TABLE ai_requests_v2 RENAME TO ai_requests;

CREATE INDEX idx_ai_requests_origin_icon
  ON ai_requests(origin_icon_id, created_at);

CREATE INDEX idx_ai_requests_status
  ON ai_requests(status, expires_at, created_at);

CREATE INDEX idx_ai_requests_scope_status
  ON ai_requests(request_scope, status, created_at);

CREATE INDEX idx_ai_requests_retry
  ON ai_requests(retry_of_request_id, created_at);

CREATE TRIGGER trg_ai_web_handoff_request_guard_before_insert
BEFORE INSERT ON ai_web_handoff_packages
WHEN NEW.result_sha256 IS NOT NULL
  OR NEW.candidate_id IS NOT NULL
  OR NEW.result_received_at IS NOT NULL
  OR NEW.cleanup_requested_at IS NOT NULL
  OR NEW.payload_deleted_at IS NOT NULL
  OR NEW.extended_at IS NOT NULL
  OR abs((julianday(NEW.expires_at) - julianday(NEW.created_at)) - 7.0) >= 0.000001
  OR NOT EXISTS (
    SELECT 1
    FROM ai_requests request
    WHERE request.id = NEW.request_id
      AND request.provider_mode = 'manual_web'
      AND request.service_surface = NEW.service_surface
      AND request.operation = 'static_image_edit_web_handoff'
      AND request.status = 'awaiting_result'
      AND request.created_at = NEW.created_at
      AND request.expires_at = NEW.expires_at
  )
BEGIN
  SELECT RAISE(ABORT, 'AI web handoff request mismatch');
END;

CREATE TRIGGER trg_ai_web_handoff_snapshots_immutable_before_update
BEFORE UPDATE OF
  request_id,
  handoff_kind,
  layout_mode,
  operation,
  service_surface,
  upload_file_name,
  upload_sha256,
  manifest_file_name,
  manifest_sha256,
  prompt_file_name,
  prompt_sha256,
  expected_width,
  expected_height,
  expected_has_alpha,
  created_at
ON ai_web_handoff_packages
WHEN
  NEW.request_id IS NOT OLD.request_id
  OR NEW.handoff_kind IS NOT OLD.handoff_kind
  OR NEW.layout_mode IS NOT OLD.layout_mode
  OR NEW.operation IS NOT OLD.operation
  OR NEW.service_surface IS NOT OLD.service_surface
  OR NEW.upload_file_name IS NOT OLD.upload_file_name
  OR NEW.upload_sha256 IS NOT OLD.upload_sha256
  OR NEW.manifest_file_name IS NOT OLD.manifest_file_name
  OR NEW.manifest_sha256 IS NOT OLD.manifest_sha256
  OR NEW.prompt_file_name IS NOT OLD.prompt_file_name
  OR NEW.prompt_sha256 IS NOT OLD.prompt_sha256
  OR NEW.expected_width IS NOT OLD.expected_width
  OR NEW.expected_height IS NOT OLD.expected_height
  OR NEW.expected_has_alpha IS NOT OLD.expected_has_alpha
  OR NEW.created_at IS NOT OLD.created_at
BEGIN
  SELECT RAISE(ABORT, 'AI web handoff snapshots are immutable');
END;

CREATE TRIGGER trg_ai_web_handoff_extension_once_before_update
BEFORE UPDATE OF expires_at, extended_at ON ai_web_handoff_packages
WHEN NOT (
  OLD.extended_at IS NULL
  AND OLD.cleanup_requested_at IS NULL
  AND OLD.result_received_at IS NULL
  AND OLD.payload_deleted_at IS NULL
  AND NEW.extended_at IS NOT NULL
  AND EXISTS (
    SELECT 1 FROM ai_requests request
    WHERE request.id = NEW.request_id
      AND request.status = 'awaiting_result'
  )
  AND julianday(NEW.extended_at) IS NOT NULL
  AND julianday(NEW.expires_at) IS NOT NULL
  AND julianday(NEW.extended_at) <= julianday(NEW.expires_at)
  AND abs((julianday(NEW.expires_at) - julianday(OLD.expires_at)) - 30.0) < 0.000001
)
BEGIN
  SELECT RAISE(ABORT, 'AI web handoff retention must extend exactly once by 30 days');
END;

CREATE TRIGGER trg_ai_web_handoff_result_guard_before_update
BEFORE UPDATE OF candidate_id, result_sha256, result_received_at ON ai_web_handoff_packages
WHEN NOT (
  OLD.candidate_id IS NULL
  AND OLD.result_sha256 IS NULL
  AND OLD.result_received_at IS NULL
  AND NEW.candidate_id IS NOT NULL
  AND NEW.result_sha256 IS NOT NULL
  AND NEW.result_received_at IS NOT NULL
  AND NEW.cleanup_requested_at IS NOT NULL
  AND EXISTS (
    SELECT 1 FROM ai_requests request
    WHERE request.id = NEW.request_id
      AND request.status = 'completed'
  )
  AND EXISTS (
    SELECT 1
    FROM ai_candidates candidate
    WHERE candidate.id = NEW.candidate_id
      AND candidate.request_id = NEW.request_id
      AND candidate.raw_source_sha256 = NEW.result_sha256
      AND candidate.candidate_index = 0
      AND candidate.width = NEW.expected_width
      AND candidate.height = NEW.expected_height
      AND candidate.is_animated = 0
      AND (NEW.expected_has_alpha = 0 OR candidate.has_alpha = 1)
  )
)
BEGIN
  SELECT RAISE(ABORT, 'AI web handoff result provenance mismatch');
END;

CREATE TRIGGER trg_ai_web_handoff_cleanup_request_once_before_update
BEFORE UPDATE OF cleanup_requested_at ON ai_web_handoff_packages
WHEN NOT (
  OLD.cleanup_requested_at IS NULL
  AND NEW.cleanup_requested_at IS NOT NULL
  AND julianday(NEW.cleanup_requested_at) IS NOT NULL
  AND EXISTS (
    SELECT 1
    FROM ai_requests request
    WHERE request.id = NEW.request_id
      AND request.status IN ('completed', 'failed', 'cancelled', 'expired')
  )
)
BEGIN
  SELECT RAISE(ABORT, 'AI web handoff cleanup can only be requested once');
END;

CREATE TRIGGER trg_ai_web_handoff_payload_delete_once_before_update
BEFORE UPDATE OF payload_deleted_at ON ai_web_handoff_packages
WHEN NOT (
  OLD.payload_deleted_at IS NULL
  AND NEW.payload_deleted_at IS NOT NULL
  AND OLD.cleanup_requested_at IS NOT NULL
  AND julianday(NEW.payload_deleted_at) >= julianday(OLD.cleanup_requested_at)
)
BEGIN
  SELECT RAISE(ABORT, 'AI web handoff payload deletion is monotonic');
END;

CREATE TRIGGER trg_ai_request_snapshots_immutable_before_update
BEFORE UPDATE OF
  request_scope,
  retry_of_request_id,
  origin_collection_name_snapshot,
  origin_icon_name_snapshot,
  provider_mode,
  service_surface,
  provider,
  adapter_id,
  adapter_contract_version,
  account_context,
  model,
  operation,
  provenance_trust,
  credential_mode_snapshot,
  capability_snapshot_json,
  data_tier_snapshot_json,
  retention_snapshot_json,
  consent_snapshot_json,
  policy_refs_json,
  prompt_options_snapshot_json,
  input_package_sha256,
  mask_package_sha256,
  reference_package_sha256,
  original_lineage_id,
  original_lineage_generation,
  original_source_sha256,
  effective_source_sha256,
  payload_input_signature,
  request_recipe_signature,
  activation_revision,
  created_at
ON ai_requests
WHEN
  NEW.request_scope IS NOT OLD.request_scope
  OR NEW.retry_of_request_id IS NOT OLD.retry_of_request_id
  OR NEW.origin_collection_name_snapshot IS NOT OLD.origin_collection_name_snapshot
  OR NEW.origin_icon_name_snapshot IS NOT OLD.origin_icon_name_snapshot
  OR NEW.provider_mode IS NOT OLD.provider_mode
  OR NEW.service_surface IS NOT OLD.service_surface
  OR NEW.provider IS NOT OLD.provider
  OR NEW.adapter_id IS NOT OLD.adapter_id
  OR NEW.adapter_contract_version IS NOT OLD.adapter_contract_version
  OR NEW.account_context IS NOT OLD.account_context
  OR NEW.model IS NOT OLD.model
  OR NEW.operation IS NOT OLD.operation
  OR NEW.provenance_trust IS NOT OLD.provenance_trust
  OR NEW.credential_mode_snapshot IS NOT OLD.credential_mode_snapshot
  OR NEW.capability_snapshot_json IS NOT OLD.capability_snapshot_json
  OR NEW.data_tier_snapshot_json IS NOT OLD.data_tier_snapshot_json
  OR NEW.retention_snapshot_json IS NOT OLD.retention_snapshot_json
  OR NEW.consent_snapshot_json IS NOT OLD.consent_snapshot_json
  OR NEW.policy_refs_json IS NOT OLD.policy_refs_json
  OR NEW.prompt_options_snapshot_json IS NOT OLD.prompt_options_snapshot_json
  OR NEW.input_package_sha256 IS NOT OLD.input_package_sha256
  OR NEW.mask_package_sha256 IS NOT OLD.mask_package_sha256
  OR NEW.reference_package_sha256 IS NOT OLD.reference_package_sha256
  OR NEW.original_lineage_id IS NOT OLD.original_lineage_id
  OR NEW.original_lineage_generation IS NOT OLD.original_lineage_generation
  OR NEW.original_source_sha256 IS NOT OLD.original_source_sha256
  OR NEW.effective_source_sha256 IS NOT OLD.effective_source_sha256
  OR NEW.payload_input_signature IS NOT OLD.payload_input_signature
  OR NEW.request_recipe_signature IS NOT OLD.request_recipe_signature
  OR NEW.activation_revision IS NOT OLD.activation_revision
  OR NEW.created_at IS NOT OLD.created_at
BEGIN
  SELECT RAISE(ABORT, 'AI request provenance snapshots are immutable');
END;

CREATE TRIGGER trg_ai_grid_request_insert_guard
BEFORE INSERT ON ai_requests
WHEN NEW.request_scope <> 'icon_edit'
  AND (
    NEW.status <> 'draft'
    OR NEW.origin_collection_id IS NULL
    OR NEW.origin_collection_name_snapshot IS NULL
    OR NOT EXISTS (
      SELECT 1
      FROM collections collection
      WHERE collection.id = NEW.origin_collection_id
        AND collection.deleted_at IS NULL
        AND collection.name = NEW.origin_collection_name_snapshot
    )
    OR (
      NEW.retry_of_request_id IS NOT NULL
      AND NOT EXISTS (
        SELECT 1
        FROM ai_requests original
        WHERE original.id = NEW.retry_of_request_id
          AND original.request_scope = NEW.request_scope
          AND original.status IN ('failed', 'cancelled', 'expired')
          AND julianday(original.created_at) IS NOT NULL
          AND julianday(NEW.created_at) IS NOT NULL
          AND julianday(NEW.created_at) >= julianday(original.created_at)
      )
    )
  )
BEGIN
  SELECT RAISE(ABORT, 'AI grid request draft provenance mismatch');
END;

CREATE TABLE ai_request_items (
  id TEXT PRIMARY KEY,
  request_id TEXT NOT NULL,
  request_scope TEXT NOT NULL
    CHECK (request_scope IN ('grid_edit', 'single_generate', 'grid_generate')),
  item_index INTEGER NOT NULL CHECK (item_index BETWEEN 0 AND 15),
  origin_icon_id TEXT REFERENCES icons(id) ON DELETE SET NULL,
  origin_icon_id_snapshot TEXT,
  target_name_snapshot TEXT NOT NULL
    CHECK (length(target_name_snapshot) <= 512),
  shape TEXT NOT NULL CHECK (shape = 'single'),
  row_index INTEGER NOT NULL CHECK (row_index BETWEEN 0 AND 3),
  column_index INTEGER NOT NULL CHECK (column_index BETWEEN 0 AND 3),
  input_cell_x INTEGER NOT NULL CHECK (input_cell_x BETWEEN 0 AND 12000),
  input_cell_y INTEGER NOT NULL CHECK (input_cell_y BETWEEN 0 AND 12000),
  cell_width INTEGER NOT NULL CHECK (cell_width BETWEEN 1 AND 12000),
  cell_height INTEGER NOT NULL CHECK (cell_height BETWEEN 1 AND 12000),
  original_lineage_id TEXT,
  original_lineage_generation INTEGER
    CHECK (original_lineage_generation IS NULL OR original_lineage_generation >= 0),
  original_source_sha256 TEXT,
  effective_source_sha256 TEXT,
  activation_revision INTEGER
    CHECK (activation_revision IS NULL OR activation_revision >= 0),
  native_recipe_signature TEXT,
  input_render_recipe_hash TEXT,
  input_render_sha256 TEXT,
  output_candidate_id TEXT UNIQUE
    REFERENCES ai_candidates(id) ON DELETE NO ACTION
    DEFERRABLE INITIALLY DEFERRED,
  review_status TEXT NOT NULL DEFAULT 'pending'
    CHECK (review_status IN (
      'pending', 'included', 'excluded', 'candidate_created', 'icon_created'
    )),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE(request_id, item_index),
  UNIQUE(request_id, row_index, column_index),
  UNIQUE(id, request_id),
  FOREIGN KEY (request_id, request_scope)
    REFERENCES ai_requests(id, request_scope)
    ON DELETE CASCADE
    DEFERRABLE INITIALLY DEFERRED,
  CHECK (cell_width = cell_height),
  CHECK (
    (
      request_scope = 'grid_edit'
      AND origin_icon_id_snapshot IS NOT NULL
      AND trim(origin_icon_id_snapshot) <> ''
      AND original_lineage_id IS NOT NULL
      AND trim(original_lineage_id) <> ''
      AND original_lineage_generation IS NOT NULL
      AND original_source_sha256 IS NOT NULL
      AND length(original_source_sha256) = 64
      AND original_source_sha256 NOT GLOB '*[^0-9a-f]*'
      AND effective_source_sha256 IS NOT NULL
      AND length(effective_source_sha256) = 64
      AND effective_source_sha256 NOT GLOB '*[^0-9a-f]*'
      AND activation_revision IS NOT NULL
      AND native_recipe_signature IS NOT NULL
      AND length(native_recipe_signature) = 64
      AND native_recipe_signature NOT GLOB '*[^0-9a-f]*'
      AND input_render_recipe_hash IS NOT NULL
      AND length(input_render_recipe_hash) = 64
      AND input_render_recipe_hash NOT GLOB '*[^0-9a-f]*'
      AND input_render_sha256 IS NOT NULL
      AND length(input_render_sha256) = 64
      AND input_render_sha256 NOT GLOB '*[^0-9a-f]*'
    )
    OR
    (
      request_scope IN ('single_generate', 'grid_generate')
      AND origin_icon_id IS NULL
      AND origin_icon_id_snapshot IS NULL
      AND original_lineage_id IS NULL
      AND original_lineage_generation IS NULL
      AND original_source_sha256 IS NULL
      AND effective_source_sha256 IS NULL
      AND activation_revision IS NULL
      AND native_recipe_signature IS NULL
      AND input_render_recipe_hash IS NULL
      AND input_render_sha256 IS NULL
    )
  )
);

CREATE INDEX idx_ai_request_items_request
  ON ai_request_items(request_id, item_index);

CREATE INDEX idx_ai_request_items_origin
  ON ai_request_items(origin_icon_id, request_scope, review_status);

CREATE INDEX idx_ai_request_items_output
  ON ai_request_items(output_candidate_id);

CREATE TRIGGER trg_ai_request_item_insert_guard
BEFORE INSERT ON ai_request_items
WHEN NOT EXISTS (
  SELECT 1
  FROM ai_requests request
  JOIN collections collection
    ON collection.id = request.origin_collection_id
   AND collection.deleted_at IS NULL
  WHERE request.id = NEW.request_id
    AND request.request_scope = NEW.request_scope
    AND request.status = 'draft'
    AND (
      (
        NEW.request_scope = 'grid_edit'
        AND NEW.origin_icon_id IS NOT NULL
        AND NEW.origin_icon_id = NEW.origin_icon_id_snapshot
        AND EXISTS (
          SELECT 1
          FROM icons icon
          JOIN effective_visual_sources source ON source.icon_id = icon.id
          WHERE icon.id = NEW.origin_icon_id
            AND icon.collection_id = request.origin_collection_id
            AND icon.deleted_at IS NULL
            AND icon.display_name = NEW.target_name_snapshot
            AND icon.shape = 'single'
            AND source.original_lineage_id = NEW.original_lineage_id
            AND source.original_lineage_generation = NEW.original_lineage_generation
            AND source.original_source_sha256 = NEW.original_source_sha256
            AND source.effective_source_sha256 = NEW.effective_source_sha256
            AND source.activation_revision = NEW.activation_revision
            AND source.effective_is_animated = 0
        )
      )
      OR
      (
        NEW.request_scope IN ('single_generate', 'grid_generate')
        AND NEW.origin_icon_id IS NULL
      )
    )
)
BEGIN
  SELECT RAISE(ABORT, 'AI grid request item snapshot mismatch');
END;

CREATE TRIGGER trg_ai_request_item_snapshots_immutable_before_update
BEFORE UPDATE OF
  id,
  request_id,
  request_scope,
  item_index,
  origin_icon_id_snapshot,
  target_name_snapshot,
  shape,
  row_index,
  column_index,
  input_cell_x,
  input_cell_y,
  cell_width,
  cell_height,
  original_lineage_id,
  original_lineage_generation,
  original_source_sha256,
  effective_source_sha256,
  activation_revision,
  native_recipe_signature,
  input_render_recipe_hash,
  input_render_sha256,
  created_at
ON ai_request_items
WHEN
  NEW.id IS NOT OLD.id
  OR NEW.request_id IS NOT OLD.request_id
  OR NEW.request_scope IS NOT OLD.request_scope
  OR NEW.item_index IS NOT OLD.item_index
  OR NEW.origin_icon_id_snapshot IS NOT OLD.origin_icon_id_snapshot
  OR NEW.target_name_snapshot IS NOT OLD.target_name_snapshot
  OR NEW.shape IS NOT OLD.shape
  OR NEW.row_index IS NOT OLD.row_index
  OR NEW.column_index IS NOT OLD.column_index
  OR NEW.input_cell_x IS NOT OLD.input_cell_x
  OR NEW.input_cell_y IS NOT OLD.input_cell_y
  OR NEW.cell_width IS NOT OLD.cell_width
  OR NEW.cell_height IS NOT OLD.cell_height
  OR NEW.original_lineage_id IS NOT OLD.original_lineage_id
  OR NEW.original_lineage_generation IS NOT OLD.original_lineage_generation
  OR NEW.original_source_sha256 IS NOT OLD.original_source_sha256
  OR NEW.effective_source_sha256 IS NOT OLD.effective_source_sha256
  OR NEW.activation_revision IS NOT OLD.activation_revision
  OR NEW.native_recipe_signature IS NOT OLD.native_recipe_signature
  OR NEW.input_render_recipe_hash IS NOT OLD.input_render_recipe_hash
  OR NEW.input_render_sha256 IS NOT OLD.input_render_sha256
  OR NEW.created_at IS NOT OLD.created_at
BEGIN
  SELECT RAISE(ABORT, 'AI grid request item snapshots are immutable');
END;

CREATE TRIGGER trg_ai_request_item_origin_detach_only_before_update
BEFORE UPDATE OF origin_icon_id ON ai_request_items
WHEN NOT (
  OLD.origin_icon_id IS NOT NULL
  AND NEW.origin_icon_id IS NULL
)
BEGIN
  SELECT RAISE(ABORT, 'AI grid request item origin can only be detached');
END;

CREATE TRIGGER trg_ai_request_item_candidate_once_before_update
BEFORE UPDATE OF output_candidate_id ON ai_request_items
WHEN NOT (
  OLD.output_candidate_id IS NULL
  AND NEW.output_candidate_id IS NOT NULL
  AND OLD.review_status = 'included'
  AND EXISTS (
    SELECT 1
    FROM ai_candidates candidate
    WHERE candidate.id = NEW.output_candidate_id
      AND candidate.request_id = NEW.request_id
      AND candidate.request_item_id = NEW.id
      AND candidate.candidate_index = NEW.item_index
  )
)
BEGIN
  SELECT RAISE(ABORT, 'AI grid output candidate is immutable once linked');
END;

CREATE TABLE ai_request_artifacts (
  request_id TEXT NOT NULL
    REFERENCES ai_requests(id) ON DELETE CASCADE,
  role TEXT NOT NULL CHECK (role IN ('input_sheet', 'output_sheet')),
  source_file_id TEXT NOT NULL
    REFERENCES source_files(id) ON DELETE RESTRICT,
  sha256 TEXT NOT NULL
    CHECK (
      length(sha256) = 64
      AND sha256 NOT GLOB '*[^0-9a-f]*'
    ),
  manifest_json TEXT NOT NULL
    CHECK (
      json_valid(manifest_json)
      AND length(manifest_json) <= 65536
      AND json_extract(manifest_json, '$.schema') = 'pmtcon-ai-grid-v1'
    ),
  created_at TEXT NOT NULL,
  PRIMARY KEY (request_id, role)
);

CREATE INDEX idx_ai_request_artifacts_source
  ON ai_request_artifacts(source_file_id, request_id);

CREATE TRIGGER trg_ai_request_artifact_insert_guard
BEFORE INSERT ON ai_request_artifacts
WHEN NOT EXISTS (
  SELECT 1
  FROM ai_requests request
  JOIN source_files source ON source.id = NEW.source_file_id
  WHERE request.id = NEW.request_id
    AND request.request_scope IN ('grid_edit', 'single_generate', 'grid_generate')
    AND source.sha256 = NEW.sha256
    AND source.is_animated = 0
    AND (
      (
        NEW.role = 'input_sheet'
        AND request.request_scope = 'grid_edit'
        AND request.status = 'draft'
        AND source.original_extension = 'png'
      )
      OR
      (
        NEW.role = 'output_sheet'
        AND request.status = 'awaiting_result'
        AND source.original_extension IN ('png', 'jpg', 'jpeg')
      )
    )
)
BEGIN
  SELECT RAISE(ABORT, 'AI grid request artifact provenance mismatch');
END;

CREATE TRIGGER trg_ai_request_artifacts_immutable_before_update
BEFORE UPDATE ON ai_request_artifacts
BEGIN
  SELECT RAISE(ABORT, 'AI grid request artifacts are immutable');
END;

ALTER TABLE ai_candidates
  ADD COLUMN request_item_id TEXT
  REFERENCES ai_request_items(id) ON DELETE NO ACTION
  DEFERRABLE INITIALLY DEFERRED;

CREATE UNIQUE INDEX idx_ai_candidates_request_item
  ON ai_candidates(request_item_id)
  WHERE request_item_id IS NOT NULL;

CREATE TRIGGER trg_ai_candidate_request_item_guard_before_insert
BEFORE INSERT ON ai_candidates
WHEN (
  NEW.request_item_id IS NULL
  AND NOT EXISTS (
    SELECT 1
    FROM ai_requests request
    WHERE request.id = NEW.request_id
      AND request.request_scope = 'icon_edit'
  )
)
OR (
  NEW.request_item_id IS NOT NULL
  AND (
    NEW.output_format <> 'png'
    OR NEW.is_animated <> 0
    OR NOT EXISTS (
      SELECT 1
      FROM ai_request_items item
      JOIN ai_requests request ON request.id = item.request_id
      WHERE item.id = NEW.request_item_id
        AND item.request_id = NEW.request_id
        AND item.item_index = NEW.candidate_index
        AND request.request_scope = item.request_scope
        AND request.request_scope IN ('grid_edit', 'single_generate', 'grid_generate')
        AND request.status = 'layout_review_pending'
    )
  )
)
BEGIN
  SELECT RAISE(ABORT, 'AI candidate request item provenance mismatch');
END;

CREATE TABLE ai_icon_root_creations_v2 (
  creation_order INTEGER PRIMARY KEY AUTOINCREMENT,
  icon_id TEXT NOT NULL UNIQUE REFERENCES icons(id) ON DELETE CASCADE,
  source_icon_id TEXT REFERENCES icons(id) ON DELETE SET NULL,
  candidate_id TEXT NOT NULL REFERENCES ai_candidates(id) ON DELETE RESTRICT,
  request_item_id TEXT
    REFERENCES ai_request_items(id) ON DELETE NO ACTION
    DEFERRABLE INITIALLY DEFERRED,
  creation_kind TEXT NOT NULL DEFAULT 'source_edit'
    CHECK (creation_kind IN ('source_edit', 'source_free', 'clone')),
  normalization_recipe_hash TEXT,
  created_at TEXT NOT NULL,
  CHECK (
    (
      creation_kind = 'source_edit'
      AND normalization_recipe_hash IS NOT NULL
    )
    OR
    (
      creation_kind = 'source_free'
      AND source_icon_id IS NULL
      AND request_item_id IS NOT NULL
      AND normalization_recipe_hash IS NULL
    )
    OR creation_kind = 'clone'
  )
);

INSERT INTO ai_icon_root_creations_v2 (
  creation_order,
  icon_id,
  source_icon_id,
  candidate_id,
  request_item_id,
  creation_kind,
  normalization_recipe_hash,
  created_at
)
SELECT
  creation_order,
  icon_id,
  source_icon_id,
  candidate_id,
  NULL,
  'source_edit',
  normalization_recipe_hash,
  created_at
FROM ai_icon_root_creations;

DROP TABLE ai_icon_root_creations;
ALTER TABLE ai_icon_root_creations_v2 RENAME TO ai_icon_root_creations;

CREATE INDEX idx_ai_icon_root_creations_candidate
  ON ai_icon_root_creations(candidate_id, creation_order DESC);

CREATE INDEX idx_ai_icon_root_creations_source
  ON ai_icon_root_creations(source_icon_id, creation_order DESC);

CREATE INDEX idx_ai_icon_root_creations_item
  ON ai_icon_root_creations(request_item_id, creation_order DESC);

CREATE TRIGGER trg_ai_icon_root_creation_guard_before_insert
BEFORE INSERT ON ai_icon_root_creations
WHEN (
  NEW.creation_kind IN ('source_edit', 'clone')
  AND NEW.source_icon_id IS NULL
)
OR NOT EXISTS (
  SELECT 1
  FROM ai_candidates candidate
  WHERE candidate.id = NEW.candidate_id
    AND candidate.request_item_id IS NEW.request_item_id
)
OR (
  NEW.creation_kind = 'source_free'
  AND NOT EXISTS (
    SELECT 1
    FROM ai_candidates candidate
    JOIN ai_request_items item
      ON item.id = candidate.request_item_id
     AND item.request_id = candidate.request_id
    JOIN ai_requests request ON request.id = item.request_id
    JOIN icons icon ON icon.id = NEW.icon_id
    JOIN icon_ai_state state ON state.icon_id = icon.id
    WHERE candidate.id = NEW.candidate_id
      AND item.id = NEW.request_item_id
      AND item.request_scope IN ('single_generate', 'grid_generate')
      AND request.status = 'layout_review_pending'
      AND icon.collection_id = request.origin_collection_id
      AND icon.source_file_id = candidate.raw_source_file_id
      AND state.active_version_id IS NULL
      AND state.revision = 0
  )
)
BEGIN
  SELECT RAISE(ABORT, 'AI icon root creation provenance mismatch');
END;

CREATE TRIGGER trg_ai_icon_root_creations_immutable_before_update
BEFORE UPDATE OF
  creation_order,
  icon_id,
  candidate_id,
  request_item_id,
  creation_kind,
  normalization_recipe_hash,
  created_at
ON ai_icon_root_creations
BEGIN
  SELECT RAISE(ABORT, 'AI icon root creation provenance is immutable');
END;

CREATE TRIGGER trg_ai_icon_root_creation_source_detach_only_before_update
BEFORE UPDATE OF source_icon_id ON ai_icon_root_creations
WHEN NOT (
  OLD.source_icon_id IS NOT NULL
  AND NEW.source_icon_id IS NULL
)
BEGIN
  SELECT RAISE(ABORT, 'AI icon root creation source can only be detached');
END;

CREATE TRIGGER trg_ai_request_item_review_transition_before_update
BEFORE UPDATE OF review_status ON ai_request_items
WHEN NEW.review_status IS NOT OLD.review_status
  AND NOT (
    (OLD.review_status = 'pending' AND NEW.review_status IN ('included', 'excluded'))
    OR (OLD.review_status = 'included' AND NEW.review_status = 'excluded')
    OR (OLD.review_status = 'excluded' AND NEW.review_status = 'included')
    OR (
      OLD.review_status = 'included'
      AND NEW.review_status = 'candidate_created'
      AND OLD.request_scope = 'grid_edit'
      AND OLD.output_candidate_id IS NOT NULL
    )
    OR (
      OLD.review_status = 'included'
      AND NEW.review_status = 'icon_created'
      AND OLD.request_scope IN ('single_generate', 'grid_generate')
      AND OLD.output_candidate_id IS NOT NULL
      AND EXISTS (
        SELECT 1
        FROM ai_icon_root_creations creation
        WHERE creation.candidate_id = OLD.output_candidate_id
          AND creation.request_item_id = OLD.id
          AND creation.creation_kind = 'source_free'
      )
    )
  )
BEGIN
  SELECT RAISE(ABORT, 'AI grid item review transition is invalid');
END;

CREATE TRIGGER trg_ai_grid_request_status_transition_before_update
BEFORE UPDATE OF status ON ai_requests
WHEN OLD.request_scope <> 'icon_edit'
  AND NEW.status IS NOT OLD.status
  AND NOT (
    (OLD.status = 'draft' AND NEW.status IN ('prepared', 'cancelled'))
    OR (
      OLD.status = 'prepared'
      AND NEW.status IN ('awaiting_result', 'failed', 'cancelled')
    )
    OR (
      OLD.status = 'awaiting_result'
      AND NEW.status IN ('layout_review_pending', 'failed', 'cancelled')
    )
    OR (
      OLD.status = 'layout_review_pending'
      AND NEW.status IN ('completed', 'failed', 'cancelled')
    )
  )
BEGIN
  SELECT RAISE(ABORT, 'AI grid request status transition is invalid');
END;

CREATE TRIGGER trg_ai_grid_request_prepared_guard_before_update
BEFORE UPDATE OF status ON ai_requests
WHEN OLD.request_scope <> 'icon_edit'
  AND NEW.status = 'prepared'
  AND (
    OLD.status <> 'draft'
    OR (
      OLD.request_scope = 'grid_edit'
      AND (
        (SELECT COUNT(*) FROM ai_request_items item WHERE item.request_id = OLD.id)
          NOT BETWEEN 2 AND 16
        OR (SELECT COUNT(*) FROM ai_request_artifacts artifact
            WHERE artifact.request_id = OLD.id AND artifact.role = 'input_sheet') <> 1
        OR OLD.input_package_sha256 IS NULL
        OR OLD.input_package_sha256 <> (
          SELECT artifact.sha256
          FROM ai_request_artifacts artifact
          WHERE artifact.request_id = OLD.id AND artifact.role = 'input_sheet'
        )
        OR EXISTS (
          SELECT 1
          FROM ai_request_items item
          LEFT JOIN effective_visual_sources source
            ON source.icon_id = item.origin_icon_id
          WHERE item.request_id = OLD.id
            AND (
              item.origin_icon_id IS NULL
              OR source.icon_id IS NULL
              OR source.original_lineage_id <> item.original_lineage_id
              OR source.original_lineage_generation <> item.original_lineage_generation
              OR source.original_source_sha256 <> item.original_source_sha256
              OR source.effective_source_sha256 <> item.effective_source_sha256
              OR source.activation_revision <> item.activation_revision
              OR source.effective_is_animated <> 0
            )
        )
        OR EXISTS (
          SELECT 1
          FROM ai_request_items item
          JOIN ai_request_artifacts artifact
            ON artifact.request_id = item.request_id
           AND artifact.role = 'input_sheet'
          JOIN source_files sheet ON sheet.id = artifact.source_file_id
          WHERE item.request_id = OLD.id
            AND (
              item.input_cell_x + item.cell_width > sheet.width
              OR item.input_cell_y + item.cell_height > sheet.height
            )
        )
      )
    )
    OR (
      OLD.request_scope = 'single_generate'
      AND (
        (SELECT COUNT(*) FROM ai_request_items item WHERE item.request_id = OLD.id) <> 1
        OR EXISTS (
          SELECT 1 FROM ai_request_artifacts artifact
          WHERE artifact.request_id = OLD.id AND artifact.role = 'input_sheet'
        )
        OR OLD.input_package_sha256 IS NOT NULL
      )
    )
    OR (
      OLD.request_scope = 'grid_generate'
      AND (
        (SELECT COUNT(*) FROM ai_request_items item WHERE item.request_id = OLD.id)
          NOT BETWEEN 2 AND 16
        OR EXISTS (
          SELECT 1 FROM ai_request_artifacts artifact
          WHERE artifact.request_id = OLD.id AND artifact.role = 'input_sheet'
        )
        OR OLD.input_package_sha256 IS NOT NULL
      )
    )
    OR NOT EXISTS (
      SELECT 1
      FROM ai_request_items item
      WHERE item.request_id = OLD.id
        AND item.item_index = 0
    )
    OR (
      SELECT MAX(item.item_index) + 1
      FROM ai_request_items item
      WHERE item.request_id = OLD.id
    ) <> (
      SELECT COUNT(*)
      FROM ai_request_items item
      WHERE item.request_id = OLD.id
    )
    OR EXISTS (
      SELECT 1
      FROM ai_request_artifacts artifact
      WHERE artifact.request_id = OLD.id
        AND artifact.role = 'output_sheet'
    )
    OR EXISTS (
      SELECT 1
      FROM ai_candidates candidate
      WHERE candidate.request_id = OLD.id
    )
  )
BEGIN
  SELECT RAISE(ABORT, 'AI grid request is not ready to prepare');
END;

CREATE TRIGGER trg_ai_grid_request_layout_guard_before_update
BEFORE UPDATE OF status ON ai_requests
WHEN OLD.request_scope <> 'icon_edit'
  AND NEW.status = 'layout_review_pending'
  AND (
    OLD.status <> 'awaiting_result'
    OR NOT EXISTS (
      SELECT 1
      FROM ai_request_artifacts artifact
      WHERE artifact.request_id = OLD.id
        AND artifact.role = 'output_sheet'
    )
    OR EXISTS (
      SELECT 1
      FROM ai_candidates candidate
      WHERE candidate.request_id = OLD.id
    )
  )
BEGIN
  SELECT RAISE(ABORT, 'AI grid output is not ready for layout review');
END;

CREATE TRIGGER trg_ai_grid_request_completed_guard_before_update
BEFORE UPDATE OF status ON ai_requests
WHEN OLD.request_scope <> 'icon_edit'
  AND NEW.status = 'completed'
  AND (
    OLD.status <> 'layout_review_pending'
    OR NOT EXISTS (
      SELECT 1
      FROM ai_request_items item
      WHERE item.request_id = OLD.id
        AND item.review_status <> 'excluded'
    )
    OR EXISTS (
      SELECT 1
      FROM ai_request_items item
      WHERE item.request_id = OLD.id
        AND (
          (
            OLD.request_scope = 'grid_edit'
            AND item.review_status NOT IN ('candidate_created', 'excluded')
          )
          OR (
            OLD.request_scope IN ('single_generate', 'grid_generate')
            AND item.review_status NOT IN ('icon_created', 'excluded')
          )
          OR (
            item.review_status IN ('candidate_created', 'icon_created')
            AND (
              item.output_candidate_id IS NULL
              OR NOT EXISTS (
                SELECT 1
                FROM ai_candidates candidate
                WHERE candidate.id = item.output_candidate_id
                  AND candidate.request_id = OLD.id
                  AND candidate.request_item_id = item.id
                  AND candidate.candidate_index = item.item_index
              )
            )
          )
          OR (
            item.review_status = 'excluded'
            AND item.output_candidate_id IS NOT NULL
          )
        )
    )
    OR (
      SELECT COUNT(*)
      FROM ai_candidates candidate
      WHERE candidate.request_id = OLD.id
        AND candidate.request_item_id IS NOT NULL
    ) <> (
      SELECT COUNT(*)
      FROM ai_request_items item
      WHERE item.request_id = OLD.id
        AND item.review_status IN ('candidate_created', 'icon_created')
    )
    OR (
      OLD.request_scope IN ('single_generate', 'grid_generate')
      AND EXISTS (
        SELECT 1
        FROM ai_request_items item
        WHERE item.request_id = OLD.id
          AND item.review_status = 'icon_created'
          AND NOT EXISTS (
            SELECT 1
            FROM ai_icon_root_creations creation
            WHERE creation.request_item_id = item.id
              AND creation.candidate_id = item.output_candidate_id
              AND creation.creation_kind = 'source_free'
          )
      )
    )
  )
BEGIN
  SELECT RAISE(ABORT, 'AI grid request batch commit is incomplete');
END;
