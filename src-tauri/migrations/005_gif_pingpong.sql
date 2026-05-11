ALTER TABLE icons
  ADD COLUMN gif_pingpong INTEGER NOT NULL DEFAULT 0
    CHECK (gif_pingpong IN (0, 1));
