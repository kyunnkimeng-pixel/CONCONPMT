ALTER TABLE icons
  ADD COLUMN icon_kind TEXT NOT NULL DEFAULT 'image'
    CHECK (icon_kind IN ('image', 'placeholder'));

ALTER TABLE icons
  ADD COLUMN readiness TEXT NOT NULL DEFAULT 'complete'
    CHECK (readiness IN ('complete', 'working'));

ALTER TABLE icons
  ADD COLUMN placeholder_text TEXT;
