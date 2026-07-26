CREATE TABLE icon_effect_recipes (
  icon_id TEXT PRIMARY KEY REFERENCES icons(id) ON DELETE CASCADE,
  recipe_schema TEXT NOT NULL DEFAULT 'pmtcon-effects-v1'
    CHECK (recipe_schema = 'pmtcon-effects-v1'),
  revision INTEGER NOT NULL DEFAULT 1
    CHECK (revision >= 1),
  effects_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
