CREATE TABLE icon_ai_lineages (
  icon_id TEXT NOT NULL REFERENCES icons(id) ON DELETE CASCADE,
  lineage_id TEXT NOT NULL CHECK (trim(lineage_id) <> ''),
  lineage_generation INTEGER NOT NULL CHECK (lineage_generation >= 0),
  original_source_file_id TEXT NOT NULL REFERENCES source_files(id) ON DELETE RESTRICT,
  created_at TEXT NOT NULL,
  PRIMARY KEY (icon_id, lineage_id, lineage_generation)
);

CREATE INDEX idx_icon_ai_lineages_source
  ON icon_ai_lineages(original_source_file_id, icon_id);

INSERT INTO icon_ai_lineages (
  icon_id,
  lineage_id,
  lineage_generation,
  original_source_file_id,
  created_at
)
SELECT
  id,
  original_lineage_id,
  original_lineage_generation,
  source_file_id,
  created_at
FROM icons
WHERE trim(original_lineage_id) <> '';

INSERT OR IGNORE INTO icon_ai_lineages (
  icon_id,
  lineage_id,
  lineage_generation,
  original_source_file_id,
  created_at
)
SELECT
  icon_id,
  base_original_lineage_id,
  base_original_lineage_generation,
  base_original_source_file_id,
  MIN(created_at)
FROM icon_ai_versions
GROUP BY
  icon_id,
  base_original_lineage_id,
  base_original_lineage_generation,
  base_original_source_file_id;

CREATE TRIGGER trg_icons_ai_lineage_registry_after_insert
AFTER INSERT ON icons
WHEN trim(NEW.original_lineage_id) <> ''
BEGIN
  INSERT INTO icon_ai_lineages (
    icon_id,
    lineage_id,
    lineage_generation,
    original_source_file_id,
    created_at
  )
  VALUES (
    NEW.id,
    NEW.original_lineage_id,
    NEW.original_lineage_generation,
    NEW.source_file_id,
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  );
END;

CREATE TRIGGER trg_icons_ai_lineage_registry_after_update
AFTER UPDATE OF source_file_id, original_lineage_id, original_lineage_generation ON icons
WHEN trim(NEW.original_lineage_id) <> ''
  AND NOT EXISTS (
    SELECT 1
    FROM icon_ai_lineages lineage
    WHERE lineage.icon_id = NEW.id
      AND lineage.lineage_id = NEW.original_lineage_id
      AND lineage.lineage_generation = NEW.original_lineage_generation
      AND lineage.original_source_file_id = NEW.source_file_id
  )
BEGIN
  INSERT INTO icon_ai_lineages (
    icon_id,
    lineage_id,
    lineage_generation,
    original_source_file_id,
    created_at
  )
  VALUES (
    NEW.id,
    NEW.original_lineage_id,
    NEW.original_lineage_generation,
    NEW.source_file_id,
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  );
END;

CREATE TRIGGER trg_icon_ai_lineages_immutable_before_update
BEFORE UPDATE ON icon_ai_lineages
BEGIN
  SELECT RAISE(ABORT, 'AI icon lineages are immutable');
END;

DROP TRIGGER trg_icon_ai_version_lineage_guard_before_insert;

CREATE TRIGGER trg_icon_ai_version_lineage_guard_before_insert
BEFORE INSERT ON icon_ai_versions
WHEN NOT EXISTS (
  SELECT 1
  FROM icon_ai_lineages lineage
  WHERE lineage.icon_id = NEW.icon_id
    AND lineage.lineage_id = NEW.base_original_lineage_id
    AND lineage.lineage_generation = NEW.base_original_lineage_generation
    AND lineage.original_source_file_id = NEW.base_original_source_file_id
)
BEGIN
  SELECT RAISE(ABORT, 'AI version base lineage is not registered');
END;
