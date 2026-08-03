CREATE TABLE ai_grid_payload_retention (
  request_id TEXT PRIMARY KEY
    REFERENCES ai_requests(id) ON DELETE CASCADE,
  expires_at TEXT NOT NULL,
  cleanup_requested_at TEXT,
  payload_deleted_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  CHECK (julianday(created_at) IS NOT NULL),
  CHECK (julianday(expires_at) IS NOT NULL),
  CHECK (julianday(expires_at) >= julianday(created_at)),
  CHECK (
    cleanup_requested_at IS NULL
    OR (
      julianday(cleanup_requested_at) IS NOT NULL
      AND julianday(cleanup_requested_at) >= julianday(created_at)
    )
  ),
  CHECK (
    payload_deleted_at IS NULL
    OR (
      cleanup_requested_at IS NOT NULL
      AND julianday(payload_deleted_at) IS NOT NULL
      AND julianday(payload_deleted_at) >= julianday(cleanup_requested_at)
    )
  )
);

INSERT INTO ai_grid_payload_retention (
  request_id,
  expires_at,
  cleanup_requested_at,
  payload_deleted_at,
  created_at,
  updated_at
)
SELECT
  request.id,
  strftime('%Y-%m-%dT%H:%M:%fZ', request.created_at, '+7 days'),
  CASE
    WHEN request.status IN ('completed', 'failed', 'cancelled', 'expired')
      THEN request.updated_at
    ELSE NULL
  END,
  NULL,
  request.created_at,
  request.updated_at
FROM ai_requests request
WHERE request.request_scope IN ('grid_edit', 'single_generate', 'grid_generate');

UPDATE ai_requests
SET expires_at = (
  SELECT retention.expires_at
  FROM ai_grid_payload_retention retention
  WHERE retention.request_id = ai_requests.id
)
WHERE request_scope IN ('grid_edit', 'single_generate', 'grid_generate')
  AND expires_at IS NULL;

CREATE INDEX idx_ai_grid_payload_retention_cleanup
  ON ai_grid_payload_retention(
    payload_deleted_at,
    cleanup_requested_at,
    expires_at,
    created_at
  );

CREATE TRIGGER trg_ai_grid_payload_retention_after_request_insert
AFTER INSERT ON ai_requests
WHEN NEW.request_scope IN ('grid_edit', 'single_generate', 'grid_generate')
BEGIN
  INSERT INTO ai_grid_payload_retention (
    request_id,
    expires_at,
    created_at,
    updated_at
  ) VALUES (
    NEW.id,
    strftime('%Y-%m-%dT%H:%M:%fZ', NEW.created_at, '+7 days'),
    NEW.created_at,
    NEW.created_at
  );

  UPDATE ai_requests
  SET expires_at = strftime('%Y-%m-%dT%H:%M:%fZ', NEW.created_at, '+7 days')
  WHERE id = NEW.id
    AND expires_at IS NULL;
END;

CREATE TRIGGER trg_ai_grid_payload_retention_insert_guard
BEFORE INSERT ON ai_grid_payload_retention
WHEN NOT EXISTS (
  SELECT 1
  FROM ai_requests request
  WHERE request.id = NEW.request_id
    AND request.request_scope IN ('grid_edit', 'single_generate', 'grid_generate')
    AND request.created_at = NEW.created_at
    AND abs((julianday(NEW.expires_at) - julianday(NEW.created_at)) - 7.0)
      < 0.000001
)
BEGIN
  SELECT RAISE(ABORT, 'AI grid payload retention mismatch');
END;

CREATE TRIGGER trg_ai_grid_payload_retention_immutable
BEFORE UPDATE ON ai_grid_payload_retention
WHEN
  NEW.request_id IS NOT OLD.request_id
  OR NEW.expires_at IS NOT OLD.expires_at
  OR NEW.created_at IS NOT OLD.created_at
  OR NOT (
    NEW.cleanup_requested_at IS OLD.cleanup_requested_at
    OR (
      OLD.cleanup_requested_at IS NULL
      AND NEW.cleanup_requested_at IS NOT NULL
      AND julianday(NEW.cleanup_requested_at) IS NOT NULL
    )
  )
  OR NOT (
    NEW.payload_deleted_at IS OLD.payload_deleted_at
    OR (
      OLD.payload_deleted_at IS NULL
      AND NEW.payload_deleted_at IS NOT NULL
      AND NEW.cleanup_requested_at IS NOT NULL
      AND julianday(NEW.payload_deleted_at)
        >= julianday(NEW.cleanup_requested_at)
    )
  )
BEGIN
  SELECT RAISE(ABORT, 'AI grid payload retention is monotonic');
END;

DROP TRIGGER trg_ai_grid_request_status_transition_before_update;

CREATE TRIGGER trg_ai_grid_request_status_transition_before_update
BEFORE UPDATE OF status ON ai_requests
WHEN OLD.request_scope <> 'icon_edit'
  AND NEW.status IS NOT OLD.status
  AND NOT (
    (OLD.status = 'draft' AND NEW.status IN ('prepared', 'cancelled', 'expired'))
    OR (
      OLD.status = 'prepared'
      AND NEW.status IN ('awaiting_result', 'failed', 'cancelled', 'expired')
    )
    OR (
      OLD.status = 'awaiting_result'
      AND NEW.status IN (
        'layout_review_pending', 'failed', 'cancelled', 'expired'
      )
    )
    OR (
      OLD.status = 'layout_review_pending'
      AND NEW.status IN ('completed', 'failed', 'cancelled', 'expired')
    )
  )
BEGIN
  SELECT RAISE(ABORT, 'AI grid request status transition is invalid');
END;
