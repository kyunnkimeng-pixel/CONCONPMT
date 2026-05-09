# AGENTS.md — PMTCONCON Studio / 디시콘 제작 도움 프로그램

## 0. Project identity
- Product/app name: **PMTCONCON Studio**. This name is fixed by the user and must be used in window title, docs, package metadata, generated UI references, export headers, and user-facing text.
- Goal: A Windows Explorer-like desktop app for importing, organizing, editing, previewing, validating, and exporting DCInside-style icon packs and other custom emoticon packs.
- Primary language in UI: Korean. Source code identifiers and comments may be English, but user-facing strings should be Korean unless the string is a technical term.

## 1. Non-negotiable product rules
1. **The source of truth is the written spec, not generated UI images.** Use `docs/PRODUCT_SPEC.md` and `docs/FEATURE_INVENTORY.md` as the feature inventory. Image generation is only a visual reference and asset-generation aid.
2. **No dead menus.** Do not add buttons, tabs, menus, or context-menu items unless they are implemented, disabled with a clear “준비 중” state, or explicitly listed as future work in `docs/FUTURE.md`.
3. **No missing features because they were absent in generated mockups.** If a generated image omits a required feature, still implement the feature and update the UI/components to include it.
4. **Preserve original files.** Applying a crop/resize must never destroy the original. Copy imported originals into the app library and store crop metadata separately so the crop box can be edited later.
5. **Persistent state is mandatory.** Collection names, cover image settings, icon names, alt values, order, crop boxes, export profile settings, GIF loop settings, and deletion/clone operations must survive app restarts.
6. **Validation is mandatory before export.** For the DCInside profile, enforce or warn according to `docs/PRODUCT_SPEC.md`: count 10–200, output cell size 200×200 by default, formats jpg/png/gif, per-file max 2MB, unique alt values, Korean alt length 1–3 characters, allowed specials `* ^ ! ~ +`, and export-order consistency.
7. **Multi-piece icons are first-class.** Single, horizontal double, and vertical double icon shapes must be represented in data, preview, editing, and export. A multi-piece item may produce multiple exported files.
8. **GIFs must remain animated in preview and export.** Preserve frame timing where possible and expose repeat/loop behavior in the edit panel.
9. **The app must behave like a file explorer.** Support grid view, breadcrumb navigation, double-click to enter collections, right-click context menus, multi-select with Shift/Ctrl, drag-and-drop ordering, delete, duplicate, rename, and set cover image.
10. **Configurable sizes.** Default to DCInside settings, but each collection can define target cell size and preview display size so the app can support non-DC emoticon workflows.

## 2. Recommended stack
Use this stack unless a strong reason is documented in `docs/DECISIONS.md`:

### Desktop shell / backend
- **Tauri 2 + Rust** for the desktop app shell and native filesystem/image commands.
- Tauri commands for import, export, copying files, opening folders/txt files, image processing, and SQLite access.
- SQLite stored in the app data directory. Prefer `rusqlite` for a straightforward local database, or `sqlx` only if async compile-time query checking is worth the extra setup.

### Frontend
- **React 19 + TypeScript + Vite**.
- **Tailwind CSS v4** with CSS-first tokens.
- **shadcn/ui** as editable component source, not as an opaque dependency.
- **TanStack Router** for typed routes/views.
- **Zustand** for transient UI state only; durable state belongs in SQLite through Tauri commands.
- **dnd-kit** for sortable icon grids and keyboard/mouse drag-and-drop.
- **react-konva** for crop/edit overlays, split lines, fixed/free crop boxes, and visual handles.

### Rust image processing
- `image` crate for decoding/encoding basic PNG/JPEG and frame handling where suitable.
- `gif` crate for GIF decode/encode and loop metadata.
- `fast_image_resize` for high-quality, fast resizing where appropriate.
- Keep image-processing functions deterministic and unit-tested.

## 3. Repository structure
Expected structure after scaffold:

```text
.
├─ AGENTS.md
├─ docs/
│  ├─ PRODUCT_SPEC.md
│  ├─ FEATURE_INVENTORY.md
│  ├─ IMPLEMENTATION_PLAN.md
│  ├─ UI_IMAGE_PROMPT.md
│  └─ DECISIONS.md
├─ src/
│  ├─ app/                 # Router, app shell, layout
│  ├─ components/          # Reusable UI components
│  ├─ features/
│  │  ├─ collections/
│  │  ├─ icons/
│  │  ├─ editor/
│  │  ├─ preview/
│  │  └─ export/
│  ├─ lib/                 # Tauri invoke wrappers, validation, utils
│  └─ styles/
└─ src-tauri/
   ├─ src/
   │  ├─ commands/         # Tauri command handlers
   │  ├─ db/               # schema, migrations, repositories
   │  ├─ imaging/          # crop/resize/gif/export pipeline
   │  └─ main.rs
   └─ migrations/
```

## 4. Data model contract
Use stable IDs. Do not infer identity from filenames alone.

Minimum entities:
- `collections`: id, name, cover_asset_id, default_cell_width, default_cell_height, preview_width, preview_height, export_format, order_index, created_at, updated_at.
- `assets`: id, original_filename, original_path_in_library, mime_type, width, height, byte_size, sha256, created_at.
- `icons`: id, collection_id, asset_id, display_name, shape (`single|horizontal_double|vertical_double`), order_index, crop_mode (`free|fixed`), crop_x, crop_y, crop_w, crop_h, cell_width_override, cell_height_override, gif_loop_mode, created_at, updated_at.
- `icon_pieces`: id, icon_id, piece_index, alt_text, generated_preview_path, export_status. Single icons have one piece. Horizontal/vertical double icons have two pieces.
- `export_profiles`: id, collection_id, name, profile_type (`dcinside|custom`), target_format, max_bytes, filename_mode (`sequence|alt`), include_alt_txt.

## 5. UI and interaction rules
- Main view shows collection blocks. Double-click enters a collection.
- Top-right `+` menu must support creating a collection and importing multiple files/folders. Drag-and-drop import must work in the main view and inside a collection.
- Collection cards show cover image and editable name. The initial cover is the first imported icon; users can later set another cover or import a 200×200 JPG/PNG cover image.
- Inside a collection, icon tiles show the rendered 100×100-ish preview by default plus alt text beneath it like a filename label. For custom profiles, preview size follows the collection display size.
- Alt text is inline-editable on click. Validate immediately and show duplicates/invalid length/invalid characters before export.
- Right-click menus must include only implemented actions: rename, duplicate, delete, set as collection cover, edit, reveal original/export where applicable.
- Selection must support Ctrl multi-select and Shift range select. Keyboard Delete must delete selected items after confirmation.
- Drag reorder must update persistent `order_index` immediately or through a debounced save.
- The edit panel opens on the right. It must show source preview, crop box, shape selector, free/fixed mode, size settings, preset alignment buttons, GIF loop settings, apply button, and reset/revert controls.
- The preview simulator must resemble a DCInside comment area and show output at display size with animated GIF playback.

## 6. Crop/export semantics
- For a cell size `W×H`:
  - Single icon crop viewport means `W×H`, exported as one file.
  - Horizontal double crop viewport means `2W×H`, split into left/right `W×H` pieces during export.
  - Vertical double crop viewport means `W×2H`, split into top/bottom `W×H` pieces during export.
- Free mode: crop box can move and resize; aspect ratio must match the selected shape. The user controls the box size in source coordinates.
- Fixed mode: crop box size is locked to the selected shape’s required viewport. The user can drag it, use preset alignment buttons, and see split lines for double icons.
- The apply action creates/updates generated previews and exportable derivatives, but must preserve the original and crop metadata.
- For GIFs, process every frame using the same crop geometry, preserve frame delays/disposal where feasible, and allow loop settings: preserve original, infinite, once, or custom repeat count.

## 7. Export contract
- Export uses the persisted icon order and piece order.
- Filename modes:
  - `sequence`: `001.png`, `002.png`, … with zero padding based on total output count.
  - `alt`: sanitized alt text as filename; if collision occurs, block export until resolved.
- Always generate `alts.txt` when enabled. Include export index, filename, icon/piece ID, display name, and alt text.
- After export, open the export folder and/or `alts.txt` through a Tauri command when requested.
- DCInside profile must validate: 10–200 output images, allowed formats jpg/png/gif, default 200×200 output pieces, max 2MB per file, unique alt text, valid alt characters/length.
- Warn, but do not necessarily block, for soft recommendations like transparent backgrounds and 5px margins unless the user selects strict validation.

## 8. Codex Windows App workflow expectations
The user will work in the Codex Windows App, not primarily through Codex CLI. Treat `CODEX_COMMANDS.md` and `WINDOWS_APP_THREAD_PROMPTS.md` as the app-thread workflow. Before implementing large changes:
1. Read `AGENTS.md`, `docs/PRODUCT_SPEC.md`, and `docs/FEATURE_INVENTORY.md`.
2. Produce or update a concise implementation plan in `docs/IMPLEMENTATION_PLAN.md`.
3. Build in vertical slices: data model → collection explorer → import/persist → edit/crop → export/validate → preview simulator → packaging.
4. After each slice, run relevant checks and summarize what changed.
5. When using image generation, generate UI reference images only after the feature inventory exists; then create `docs/UI_TRACE.md` mapping every required feature to an implemented component.
6. Use the Codex App review/diff pane, `/review` if available, or an explicit code review pass before finalizing.

## 9. Testing and done criteria
Minimum checks before claiming completion:
- `pnpm lint`
- `pnpm test`
- `pnpm build`
- `pnpm tauri build` when packaging work changes
- Rust unit tests for validation, crop math, export naming, GIF loop parsing/setting, and database migrations.
- Frontend tests for inline alt editing, multi-select, drag reorder, edit panel mode switching, and export validation dialog.

A feature is done only when it is implemented, persisted where applicable, validated, and covered by at least one automated test or a documented manual verification script.

## 10. Safety and maintainability
- Treat user-imported files as untrusted. Validate extensions and decode images safely before processing.
- Avoid unrestricted filesystem access. Prefer user-selected paths, Tauri plugin scopes, and dedicated Tauri commands.
- Never delete originals unless the user explicitly chooses a destructive “library cleanup” action with confirmation.
- Keep generated files under a predictable app data directory and make cleanup explicit.
- Keep business logic out of React components where possible. Put validation in `src/lib/validation.ts` and Rust mirror checks in `src-tauri/src/imaging` or `src-tauri/src/commands/export.rs`.
