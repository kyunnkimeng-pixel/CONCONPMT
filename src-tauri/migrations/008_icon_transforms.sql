ALTER TABLE icons
ADD COLUMN transform_quarter_turns INTEGER NOT NULL DEFAULT 0
  CHECK (transform_quarter_turns BETWEEN 0 AND 3);

ALTER TABLE icons
ADD COLUMN transform_flip_horizontal INTEGER NOT NULL DEFAULT 0
  CHECK (transform_flip_horizontal IN (0, 1));

ALTER TABLE icons
ADD COLUMN transform_flip_vertical INTEGER NOT NULL DEFAULT 0
  CHECK (transform_flip_vertical IN (0, 1));
