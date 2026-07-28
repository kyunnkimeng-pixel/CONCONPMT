DROP TRIGGER IF EXISTS trg_ai_web_handoff_result_guard_before_update;

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
      AND candidate.width > 0
      AND candidate.height > 0
      AND candidate.width * NEW.expected_height
        = candidate.height * NEW.expected_width
      AND candidate.is_animated = 0
      AND (NEW.expected_has_alpha = 0 OR candidate.has_alpha = 1)
  )
)
BEGIN
  SELECT RAISE(ABORT, 'AI web handoff result provenance mismatch');
END;

DROP TRIGGER IF EXISTS trg_ai_request_artifact_insert_guard;

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
        AND request.status = 'draft'
        AND source.original_extension = 'png'
        AND (
          (
            request.request_scope = 'grid_edit'
            AND request.reference_package_sha256 IS NULL
          )
          OR
          (
            request.request_scope IN ('single_generate', 'grid_generate')
            AND request.input_package_sha256 = NEW.sha256
            AND request.reference_package_sha256 = NEW.sha256
            AND json_extract(NEW.manifest_json, '$.kind') = 'generation_reference'
            AND json_extract(NEW.manifest_json, '$.inputSheetSha256') = NEW.sha256
          )
        )
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

DROP TRIGGER IF EXISTS trg_ai_grid_request_prepared_guard_before_update;

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
        OR OLD.reference_package_sha256 IS NOT NULL
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
        OR NOT (
          (
            (SELECT COUNT(*) FROM ai_request_artifacts artifact
             WHERE artifact.request_id = OLD.id AND artifact.role = 'input_sheet') = 0
            AND OLD.input_package_sha256 IS NULL
            AND OLD.reference_package_sha256 IS NULL
          )
          OR
          (
            (SELECT COUNT(*) FROM ai_request_artifacts artifact
             WHERE artifact.request_id = OLD.id AND artifact.role = 'input_sheet') = 1
            AND EXISTS (
              SELECT 1
              FROM ai_request_artifacts artifact
              WHERE artifact.request_id = OLD.id
                AND artifact.role = 'input_sheet'
                AND artifact.sha256 = OLD.input_package_sha256
                AND artifact.sha256 = OLD.reference_package_sha256
                AND json_extract(artifact.manifest_json, '$.kind')
                  = 'generation_reference'
                AND json_extract(artifact.manifest_json, '$.inputSheetSha256')
                  = artifact.sha256
            )
          )
        )
      )
    )
    OR (
      OLD.request_scope = 'grid_generate'
      AND (
        (SELECT COUNT(*) FROM ai_request_items item WHERE item.request_id = OLD.id)
          NOT BETWEEN 2 AND 16
        OR NOT (
          (
            (SELECT COUNT(*) FROM ai_request_artifacts artifact
             WHERE artifact.request_id = OLD.id AND artifact.role = 'input_sheet') = 0
            AND OLD.input_package_sha256 IS NULL
            AND OLD.reference_package_sha256 IS NULL
          )
          OR
          (
            (SELECT COUNT(*) FROM ai_request_artifacts artifact
             WHERE artifact.request_id = OLD.id AND artifact.role = 'input_sheet') = 1
            AND EXISTS (
              SELECT 1
              FROM ai_request_artifacts artifact
              WHERE artifact.request_id = OLD.id
                AND artifact.role = 'input_sheet'
                AND artifact.sha256 = OLD.input_package_sha256
                AND artifact.sha256 = OLD.reference_package_sha256
                AND json_extract(artifact.manifest_json, '$.kind')
                  = 'generation_reference'
                AND json_extract(artifact.manifest_json, '$.inputSheetSha256')
                  = artifact.sha256
            )
          )
        )
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
