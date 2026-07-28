CREATE TABLE ai_web_handoff_packages (
  request_id TEXT PRIMARY KEY REFERENCES ai_requests(id) ON DELETE RESTRICT,
  handoff_kind TEXT NOT NULL
    CHECK (handoff_kind = 'static_icon_sheet'),
  layout_mode TEXT NOT NULL
    CHECK (layout_mode = 'single'),
  operation TEXT NOT NULL
    CHECK (operation = 'edit'),
  service_surface TEXT NOT NULL
    CHECK (service_surface IN (
      'novelai_web', 'chatgpt_web', 'gemini_web', 'other_manual'
    )),
  upload_file_name TEXT NOT NULL
    CHECK (upload_file_name = 'upload.png'),
  upload_sha256 TEXT NOT NULL
    CHECK (
      length(upload_sha256) = 64
      AND upload_sha256 NOT GLOB '*[^0-9a-f]*'
    ),
  manifest_file_name TEXT NOT NULL
    CHECK (manifest_file_name = 'manifest.json'),
  manifest_sha256 TEXT NOT NULL
    CHECK (
      length(manifest_sha256) = 64
      AND manifest_sha256 NOT GLOB '*[^0-9a-f]*'
    ),
  prompt_file_name TEXT NOT NULL
    CHECK (prompt_file_name = 'prompt.txt'),
  prompt_sha256 TEXT NOT NULL
    CHECK (
      length(prompt_sha256) = 64
      AND prompt_sha256 NOT GLOB '*[^0-9a-f]*'
    ),
  expected_width INTEGER NOT NULL CHECK (expected_width > 0),
  expected_height INTEGER NOT NULL CHECK (expected_height > 0),
  expected_has_alpha INTEGER NOT NULL CHECK (expected_has_alpha IN (0, 1)),
  result_sha256 TEXT
    CHECK (
      result_sha256 IS NULL
      OR (
        length(result_sha256) = 64
        AND result_sha256 NOT GLOB '*[^0-9a-f]*'
      )
    ),
  candidate_id TEXT UNIQUE REFERENCES ai_candidates(id) ON DELETE RESTRICT,
  result_received_at TEXT,
  payload_deleted_at TEXT,
  cleanup_requested_at TEXT,
  extended_at TEXT,
  created_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  CHECK (julianday(created_at) IS NOT NULL),
  CHECK (julianday(expires_at) IS NOT NULL),
  CHECK (julianday(updated_at) IS NOT NULL),
  CHECK (julianday(expires_at) >= julianday(created_at)),
  CHECK (extended_at IS NULL OR (julianday(extended_at) IS NOT NULL AND julianday(extended_at) >= julianday(created_at))),
  CHECK (result_received_at IS NULL OR (julianday(result_received_at) IS NOT NULL AND julianday(result_received_at) >= julianday(created_at))),
  CHECK (cleanup_requested_at IS NULL OR (julianday(cleanup_requested_at) IS NOT NULL AND julianday(cleanup_requested_at) >= julianday(created_at))),
  CHECK (payload_deleted_at IS NULL OR (cleanup_requested_at IS NOT NULL AND julianday(payload_deleted_at) IS NOT NULL AND julianday(payload_deleted_at) >= julianday(cleanup_requested_at))),
  CHECK (
    (candidate_id IS NULL AND result_sha256 IS NULL AND result_received_at IS NULL)
    OR
    (candidate_id IS NOT NULL AND result_sha256 IS NOT NULL AND result_received_at IS NOT NULL)
  )
);

CREATE INDEX idx_ai_web_handoff_retention
  ON ai_web_handoff_packages(cleanup_requested_at, payload_deleted_at, expires_at, result_received_at);

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
