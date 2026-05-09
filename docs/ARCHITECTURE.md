# ARCHITECTURE.md - PMTCONCON Studio implementation architecture

This document turns `docs/PRODUCT_SPEC.md` and
`docs/FEATURE_INVENTORY.md` into implementation structure. The written spec
remains the source of truth. Generated UI images are visual references only.

## 1. Scope and guardrails

- Product name: **PMTCONCON Studio**.
- Desktop shell: Tauri 2 + Rust native commands.
- Frontend: React 19 + TypeScript + Vite.
- Package manager: npm.
- Durable state: SQLite in the Tauri app data directory.
- Transient UI state: Zustand only for view/session state.
- No visible menu, toolbar button, tab, or context-menu item should be added
  until it is implemented, clearly disabled with a Korean "preparing" reason,
  or explicitly listed as future work.
- Original imported files are immutable library assets. Crop, resize, split,
  preview, and export outputs are generated derivatives.

## 2. Frontend folder structure

Planned `src/` layout:

```text
src/
  app/
    AppShell.tsx              # window frame, navigation regions, layout
    router.tsx                # TanStack Router setup
    app-store.ts              # transient Zustand app/view state
    routes/
      home-route.tsx          # collection explorer
      collection-route.tsx    # icon explorer for one collection
  components/
    ui/                       # shadcn/ui-style editable primitives
    explorer/                 # shared explorer primitives
      Breadcrumb.tsx
      Toolbar.tsx
      ContextMenu.tsx
      InlineNameEditor.tsx
  features/
    collections/
      api.ts                  # Tauri invoke wrappers
      types.ts
      components/
        CollectionGrid.tsx
        CollectionCard.tsx
      hooks/
    icons/
      api.ts
      types.ts
      components/
        IconGrid.tsx
        IconTile.tsx
        AltInlineEditor.tsx
        IconContextMenu.tsx
      selection/
        selection-model.ts
    editor/
      api.ts
      types.ts
      crop-math.ts            # frontend mirror for UI constraints only
      components/
        EditorPanel.tsx
        CropCanvas.tsx
        ShapeSelector.tsx
        CropModeControl.tsx
        PresetPositionGrid.tsx
        GifLoopControl.tsx
    preview/
      components/
        DcinsidePreview.tsx
        PreviewComposer.tsx
    export/
      api.ts
      types.ts
      validation-view-model.ts
      components/
        ExportDialog.tsx
        ValidationResultList.tsx
    settings/
      api.ts
      types.ts
  lib/
    tauri.ts                  # typed invoke helper and error normalization
    validation.ts             # DCInside/custom validation shared by UI
    export-naming.ts          # frontend preview of naming decisions
    file-types.ts
    utils.ts
  styles/
    globals.css
```

Rules:

- React components should render state returned by commands, not invent hidden
  local persistence.
- Business rules that affect export validity must live in `src/lib/validation.ts`
  and have a Rust mirror before export commands trust them.
- Frontend crop math may constrain the UI, but Rust imaging remains the final
  authority for generated files.
- User-facing strings should be Korean. Identifiers may stay English.

## 3. Rust backend module structure

Planned `src-tauri/src/` layout:

```text
src-tauri/src/
  lib.rs
  app_state.rs               # managed DB connection/app path state
  error.rs                   # serializable AppError
  paths.rs                   # app data folder construction
  commands/
    mod.rs
    collections.rs           # collection CRUD, cover selection
    icons.rs                 # icon CRUD, rename, duplicate, order, pieces
    import.rs                # file/folder import, hashing, source copy
    editor.rs                # crop metadata updates and preview generation
    export.rs                # validate/export/open outputs
    settings.rs              # app settings read/write
    files.rs                 # reveal original/export paths through opener
  db/
    mod.rs
    connection.rs
    migrations.rs
    schema.rs                # embedded migration SQL entry points
    repositories/
      collections.rs
      source_files.rs
      icons.rs
      export_profiles.rs
      settings.rs
  imaging/
    mod.rs
    geometry.rs              # shape viewport and crop rectangle math
    raster.rs                # PNG/JPEG crop/resize/encode
    gif_pipeline.rs          # GIF decode/crop/resize/loop/encode
    thumbnails.rs
    validation.rs            # dimensions, bytes, transparency/margin warnings
  export/
    mod.rs
    naming.rs                # sequence/alt filename allocation
    alts_txt.rs
    planner.rs               # icon order -> piece export plan
    validator.rs
```

Command rules:

- Commands accept stable IDs, not filenames, as identity.
- Commands return DTOs shaped for the UI and hide internal absolute paths unless
  the user requested reveal/open behavior.
- Import and export commands validate extensions and decode images before
  trusting metadata.
- File opening uses Tauri/plugin opener commands and app-scoped paths.

## 4. SQLite schema

IDs are stable `TEXT` values, preferably UUID v7 or ULID. Timestamps are
ISO-8601 UTC strings. Soft-deleted rows keep `deleted_at` instead of being
physically removed.

### 4.1 Collections

`collections` represents DCInside/custom icon collections.

```sql
CREATE TABLE collections (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  cover_source_file_id TEXT REFERENCES source_files(id),
  cover_icon_id TEXT REFERENCES icons(id),
  default_cell_width INTEGER NOT NULL DEFAULT 200,
  default_cell_height INTEGER NOT NULL DEFAULT 200,
  preview_width INTEGER NOT NULL DEFAULT 100,
  preview_height INTEGER NOT NULL DEFAULT 100,
  export_format TEXT NOT NULL DEFAULT 'png'
    CHECK (export_format IN ('jpg', 'png', 'gif', 'source')),
  max_bytes INTEGER NOT NULL DEFAULT 2097152,
  allowed_formats_json TEXT NOT NULL DEFAULT '["jpg","jpeg","png","gif"]',
  order_index INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT
);

CREATE INDEX idx_collections_order
  ON collections(deleted_at, order_index, created_at);
```

### 4.2 Source files

`source_files` is the durable imported-original entity. It fulfills the
`assets` role from `AGENTS.md`.

```sql
CREATE TABLE source_files (
  id TEXT PRIMARY KEY,
  original_filename TEXT NOT NULL,
  original_path_in_library TEXT NOT NULL UNIQUE,
  original_extension TEXT NOT NULL,
  mime_type TEXT NOT NULL,
  width INTEGER NOT NULL,
  height INTEGER NOT NULL,
  byte_size INTEGER NOT NULL,
  sha256 TEXT NOT NULL,
  is_animated INTEGER NOT NULL DEFAULT 0,
  frame_count INTEGER,
  original_loop_mode TEXT DEFAULT 'preserve'
    CHECK (original_loop_mode IN ('preserve', 'infinite', 'once', 'count')),
  original_loop_count INTEGER,
  imported_from_path TEXT,
  created_at TEXT NOT NULL
);

CREATE UNIQUE INDEX idx_source_files_sha256 ON source_files(sha256);
```

### 4.3 Icons

`icons` is the user-visible tile/group. One icon may produce one or two export
files depending on `shape`.

```sql
CREATE TABLE icons (
  id TEXT PRIMARY KEY,
  collection_id TEXT NOT NULL REFERENCES collections(id),
  source_file_id TEXT NOT NULL REFERENCES source_files(id),
  display_name TEXT NOT NULL,
  shape TEXT NOT NULL DEFAULT 'single'
    CHECK (shape IN ('single', 'horizontal_double', 'vertical_double')),
  order_index INTEGER NOT NULL,
  cell_width_override INTEGER,
  cell_height_override INTEGER,
  thumbnail_path TEXT,
  current_preview_path TEXT,
  gif_loop_mode TEXT NOT NULL DEFAULT 'preserve'
    CHECK (gif_loop_mode IN ('preserve', 'infinite', 'once', 'count')),
  gif_loop_count INTEGER,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT
);

CREATE INDEX idx_icons_collection_order
  ON icons(collection_id, deleted_at, order_index, created_at);
```

### 4.4 Crop settings

One active crop setting row belongs to one icon. Keep old rows only if undo or
history is later implemented; MVP can update the row in place.

```sql
CREATE TABLE crop_settings (
  id TEXT PRIMARY KEY,
  icon_id TEXT NOT NULL UNIQUE REFERENCES icons(id) ON DELETE CASCADE,
  crop_mode TEXT NOT NULL DEFAULT 'free'
    CHECK (crop_mode IN ('free', 'fixed')),
  crop_x REAL NOT NULL,
  crop_y REAL NOT NULL,
  crop_w REAL NOT NULL,
  crop_h REAL NOT NULL,
  preset_position TEXT NOT NULL DEFAULT 'center'
    CHECK (preset_position IN (
      'center',
      'top_left',
      'top',
      'top_right',
      'left',
      'right',
      'bottom_left',
      'bottom',
      'bottom_right',
      'custom'
    )),
  source_width_at_apply INTEGER,
  source_height_at_apply INTEGER,
  viewport_width_at_apply INTEGER NOT NULL,
  viewport_height_at_apply INTEGER NOT NULL,
  updated_at TEXT NOT NULL
);
```

### 4.5 Multi-piece icon settings and alt values

`icon_pieces` stores piece order and per-piece alt values. Single icons have one
row. Horizontal/vertical double icons have two rows.

```sql
CREATE TABLE icon_pieces (
  id TEXT PRIMARY KEY,
  icon_id TEXT NOT NULL REFERENCES icons(id) ON DELETE CASCADE,
  piece_index INTEGER NOT NULL,
  piece_role TEXT NOT NULL
    CHECK (piece_role IN ('single', 'left', 'right', 'top', 'bottom')),
  alt_text TEXT NOT NULL DEFAULT '',
  generated_preview_path TEXT,
  last_export_path TEXT,
  export_status TEXT NOT NULL DEFAULT 'not_exported'
    CHECK (export_status IN ('not_exported', 'ready', 'warning', 'error')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE(icon_id, piece_index)
);

CREATE INDEX idx_icon_pieces_icon_order
  ON icon_pieces(icon_id, piece_index);
```

Duplicate alt validation is collection/profile scoped, so the database should
not use a global unique constraint on `alt_text`.

### 4.6 Export profiles

```sql
CREATE TABLE export_profiles (
  id TEXT PRIMARY KEY,
  collection_id TEXT NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  profile_type TEXT NOT NULL
    CHECK (profile_type IN ('dcinside', 'custom')),
  target_format TEXT NOT NULL DEFAULT 'png'
    CHECK (target_format IN ('jpg', 'png', 'gif', 'source')),
  target_cell_width INTEGER NOT NULL DEFAULT 200,
  target_cell_height INTEGER NOT NULL DEFAULT 200,
  preview_width INTEGER NOT NULL DEFAULT 100,
  preview_height INTEGER NOT NULL DEFAULT 100,
  max_bytes INTEGER NOT NULL DEFAULT 2097152,
  allowed_formats_json TEXT NOT NULL DEFAULT '["jpg","jpeg","png","gif"]',
  filename_mode TEXT NOT NULL DEFAULT 'sequence'
    CHECK (filename_mode IN ('sequence', 'alt')),
  include_alt_txt INTEGER NOT NULL DEFAULT 1,
  strict_warnings INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE(collection_id, name)
);
```

Every collection gets a default DCInside profile on creation. Custom profiles
may override dimensions, byte limits, formats, and filename rules.

### 4.7 App settings

```sql
CREATE TABLE app_settings (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  last_open_collection_id TEXT REFERENCES collections(id),
  last_view_mode TEXT NOT NULL DEFAULT 'grid'
    CHECK (last_view_mode IN ('grid', 'list')),
  last_export_directory TEXT,
  locale TEXT NOT NULL DEFAULT 'ko-KR',
  theme TEXT NOT NULL DEFAULT 'system'
    CHECK (theme IN ('system', 'light', 'dark')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
```

App settings restore the last opened collection and view mode, but selection is
transient and does not need durable storage.

## 5. App data folder layout

Use Tauri's app data directory as `{app_data}`. On Windows this is under the
user profile app-data area for the app identifier.

```text
{app_data}/
  library.sqlite
  library.sqlite-wal
  originals/
    ab/
      {sha256}.{ext}                 # immutable imported source files
  thumbnails/
    source-files/
      {source_file_id}.png           # small explorer thumbnail
  previews/
    collections/
      {collection_id}/
        {icon_id}/
          preview.{png|gif}          # current rendered icon preview
          piece-00.{png|gif}
          piece-01.{png|gif}
  generated/
    crops/
      {collection_id}/
        {icon_id}/
          {revision_id}/
            viewport.{png|gif}
            piece-00.{png|gif}
            piece-01.{png|gif}
  exports/
    {collection_slug}-{yyyyMMdd-HHmmss}/
      001.png
      002.png
      alts.txt
      export-manifest.json
  temp/
    import/
    export/
```

Rules:

- `originals/` is append-only except for explicit library cleanup.
- Generated previews and crop outputs may be replaced after successful apply,
  but the database update and file write should be coordinated so stale paths
  are not committed.
- Export runs are immutable snapshots. Re-export creates a new timestamped
  folder unless the user explicitly selects overwrite behavior later.
- `export-manifest.json` records profile ID, collection ID, icon/piece IDs,
  filenames, byte sizes, dimensions, warnings, and errors for diagnostics.

## 6. Icon shape representation

Effective cell size:

```text
W = icon.cell_width_override ?? collection.default_cell_width
H = icon.cell_height_override ?? collection.default_cell_height
```

Shape mapping:

| Shape | User meaning | Crop viewport | Piece rows | Export pieces |
|---|---|---:|---|---:|
| `single` | Single icon | `W x H` | `single` index 0 | 1 |
| `horizontal_double` | Horizontal double icon | `2W x H` | `left` 0, `right` 1 | 2 |
| `vertical_double` | Vertical double icon | `W x 2H` | `top` 0, `bottom` 1 | 2 |

Export and validation order is always:

```text
icons.order_index ASC, icons.created_at ASC, icon_pieces.piece_index ASC
```

The collection UI may show a double icon as one grouped tile, but validation and
export count every piece.

## 7. Crop box storage and behavior

Crop coordinates are stored in source-image pixel coordinates:

```text
crop_x, crop_y, crop_w, crop_h
```

The crop rectangle describes the full viewport for the selected shape. Double
icon split lines are derived from the effective viewport, not stored manually.

### Free mode

- User may move and resize the box.
- Aspect ratio must equal the selected shape viewport:
  - single: `W / H`
  - horizontal double: `(2W) / H`
  - vertical double: `W / (2H)`
- `preset_position` becomes `custom` after manual resize/move unless the box
  exactly matches a preset calculation.
- Export resizes the cropped viewport into target viewport size, then splits
  double icons into `W x H` pieces.

### Fixed mode

- Box size is locked to the selected shape viewport in source pixels:
  - single: `W x H`
  - horizontal double: `2W x H`
  - vertical double: `W x 2H`
- User may move the box and choose one of the nine preset positions.
- If the source image is smaller than the locked viewport, the box is centered
  over the source, export upscales to the target, and validation may show a
  quality warning. The original remains unchanged.

### Preset positions

Preset names map to normalized anchor positions:

| Preset | Anchor |
|---|---|
| `top_left` | `(0, 0)` |
| `top` | `(0.5, 0)` |
| `top_right` | `(1, 0)` |
| `left` | `(0, 0.5)` |
| `center` | `(0.5, 0.5)` |
| `right` | `(1, 0.5)` |
| `bottom_left` | `(0, 1)` |
| `bottom` | `(0.5, 1)` |
| `bottom_right` | `(1, 1)` |

The geometry module computes `crop_x/crop_y` by placing the crop viewport within
the source rectangle at the preset anchor and clamping to source bounds.

## 8. GIF crop, resize, preview, and export

GIFs are first-class source files and must remain animated in preview and export.

Pipeline:

1. Import validates the GIF by decoding metadata and records width, height,
   frame count, byte size, SHA-256, and original loop information when readable.
2. Preview before editing may use a thumbnail plus the original GIF for
   animation. After crop apply, generate an animated preview GIF under
   `previews/`.
3. Crop/export decodes frames in order, composes disposal state where required,
   applies the same source-coordinate crop to every frame, resizes every cropped
   frame to the target viewport/piece size, and encodes a new GIF.
4. Frame delays should be preserved. Disposal and transparency should be
   preserved where the `gif` crate exposes enough information; otherwise use a
   deterministic fallback that keeps visual order correct.
5. Loop setting comes from `icons.gif_loop_mode`:
   - `preserve`: keep source loop behavior where known.
   - `infinite`: write infinite repeat metadata.
   - `once`: write no repeat extension or a single-play equivalent.
   - `count`: write `gif_loop_count`.
6. Export validation measures the final generated file byte size. For the
   DCInside profile, any output file above 2MB is a hard error.

Testing targets:

- Crop geometry is identical for every GIF frame.
- Frame count and frame delays survive simple round trips.
- Loop metadata changes according to `gif_loop_mode`.
- DCInside byte-size validation checks the encoded output, not the source.

## 9. Validation rules

Validation runs before export. Frontend validation is for immediate feedback;
Rust validation is authoritative before files are written.

### DCInside hard errors

- Output piece count must be 10 to 200 inclusive.
- Every exported piece must be `200 x 200` by default for the DCInside profile.
- Actual output format must be `jpg`, `png`, or `gif`.
- Every output file must be at most 2MB (`2,097,152` bytes).
- Every piece must have a non-empty alt value.
- Alt values must be unique across all output pieces in the collection export.
- Alt values must be 1 to 3 user-perceived grapheme characters by Korean
  length 기준.
- Allowed alt characters are Hangul, English letters, digits, normal
  no-whitespace word characters, and `*`, `^`, `!`, `~`, `+`.
- Disallowed alt characters include newline, tab, path separators, filename-risk
  characters, control characters, whitespace, and emoji for the DCInside profile.
- In alt filename mode, sanitized filenames must be non-empty and collision-free.

### DCInside warnings

- PNG/GIF transparent background is recommended for character icons.
- 5px top/right/bottom/left margin is recommended.
- Photos or imagery hard to use in conversation may be warned manually or by
  future heuristics.
- Upscaling a source smaller than the target cell should warn about quality.

Warnings do not block export unless `strict_warnings` is enabled.

### Preview defaults

- DCInside export cell size defaults to `200 x 200`.
- DCInside display preview defaults to `100 x 100`.
- Custom profiles can change these values without weakening the DCInside
  profile.

## 10. Export production

Export uses a planner:

1. Load collection, profile, icons, crop settings, source files, and pieces.
2. Expand icons into piece export jobs using persisted icon order and piece
   order.
3. Validate count, alt values, formats, crop settings, dimensions, and naming.
4. Generate outputs into a temporary export folder.
5. Measure byte sizes and run final validation.
6. Move the temporary folder into `exports/{collection_slug}-{timestamp}/`.
7. Update `icon_pieces.last_export_path` and `export_status`.
8. Optionally open the export folder and/or `alts.txt`.

### Sequence filenames

Default mode uses zero-padded sequence names based on total output count:

```text
001.png
002.png
003.png
```

For 120 outputs, names run `001` through `120`. For more than 999 custom-profile
outputs, padding grows to the digit length of the total count.

### Alt filenames

Alt filename mode sanitizes `alt_text` into a filesystem-safe basename. Export
is blocked if any sanitized basename is empty, unsafe, reserved, or duplicated.

### `alts.txt`

When enabled, generate UTF-8 text:

```text
# PMTCONCON Studio export
# Collection: {collection name}
# Profile: {profile name}
001.png	001	{icon_piece_id}	{icon display name}	{alt text}
002.png	002	{icon_piece_id}	{icon display name}	{alt text}
```

Columns are filename, export index, icon/piece ID, display name, and alt text.

## 11. Implementation order

Architecture should be implemented in vertical slices:

1. Migrations, app paths, and repository tests.
2. Collection CRUD commands and empty explorer data flow.
3. Import commands copying originals into `originals/`.
4. Icon/piece management, alt validation, and order persistence.
5. Crop metadata plus PNG/JPEG/GIF preview generation.
6. Export planner, validator, naming, and `alts.txt`.
7. Preview simulator and final UI trace/review.

Each slice updates `docs/FEATURE_INVENTORY.md` and runs the relevant available
checks before moving to the next slice.
