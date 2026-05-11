ALTER TABLE icons
  ADD COLUMN text_overlay_enabled INTEGER NOT NULL DEFAULT 0
    CHECK (text_overlay_enabled IN (0,1));

ALTER TABLE icons
  ADD COLUMN text_overlay_text TEXT NOT NULL DEFAULT '';

ALTER TABLE icons
  ADD COLUMN text_overlay_font_path TEXT;

ALTER TABLE icons
  ADD COLUMN text_overlay_font_size REAL NOT NULL DEFAULT 28.0;

ALTER TABLE icons
  ADD COLUMN text_overlay_x REAL NOT NULL DEFAULT 0.5;

ALTER TABLE icons
  ADD COLUMN text_overlay_y REAL NOT NULL DEFAULT 0.82;

ALTER TABLE icons
  ADD COLUMN text_overlay_color TEXT NOT NULL DEFAULT '#FFFFFF';

ALTER TABLE icons
  ADD COLUMN text_overlay_stroke_color TEXT NOT NULL DEFAULT '#000000';

ALTER TABLE icons
  ADD COLUMN text_overlay_stroke_width REAL NOT NULL DEFAULT 2.0;
