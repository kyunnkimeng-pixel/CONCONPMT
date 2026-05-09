# IMPLEMENTATION_PLAN.md — Codex 실행 계획

## Phase 0 — Scaffold and safety rails
1. Scaffold Tauri 2 + React + TypeScript + Vite project.
2. Install Tailwind CSS v4, shadcn/ui, TanStack Router, Zustand, dnd-kit, react-konva.
3. Add Rust crates for SQLite and imaging.
4. Add `docs/FEATURE_INVENTORY.md` and keep it visible in the repo.
5. Create CI-like local scripts: `lint`, `test`, `build`, `tauri:dev`, `tauri:build`.

Done when: App opens, empty Explorer-like shell renders, checks run.

## Phase 1 — Database and library storage
1. Create SQLite schema for collections, assets, icons, icon_pieces, export_profiles.
2. Implement migrations.
3. Implement Tauri commands:
   - `list_collections`
   - `create_collection`
   - `rename_collection`
   - `duplicate_collection`
   - `delete_collection`
   - `list_icons`
   - `update_icon_order`
4. Define app data directory layout:
   - `library.sqlite`
   - `assets/originals/`
   - `assets/generated/`
   - `exports/`

Done when: Collection CRUD persists across restart.

## Phase 2 — Explorer UI and import
1. Main screen collection grid.
2. Breadcrumb navigation.
3. `+` menu and drag-and-drop zones.
4. Tauri file dialog and import command.
5. Copy originals into library with content hash.
6. Create icon and first piece records.
7. Set first icon as collection cover automatically.

Done when: Multiple jpg/png/gif files can be imported and are still visible after restart.

## Phase 3 — Icon management
1. Icon grid/list view.
2. Multi-select with Ctrl/Shift and keyboard Delete.
3. Context menus: edit, rename, duplicate, delete, set as cover.
4. dnd-kit reorder with persistence.
5. Inline alt editing and duplicate/invalid highlighting.

Done when: The collection behaves like a file explorer and `order_index`/alt text persist.

## Phase 4 — Editor and imaging pipeline
1. Right-side editor panel.
2. Shape selector: single, horizontal double, vertical double.
3. Free/fixed crop modes.
4. React-Konva crop box with handles, drag, aspect ratio constraints, split lines.
5. Fixed mode 9-position presets.
6. Rust crop/resize for PNG/JPEG.
7. GIF frame crop/resize and loop setting support.
8. Preserve original and crop metadata.

Done when: Applying edits updates previews/exports without destroying source and can be re-edited later.

## Phase 5 — Preview simulator
1. DCInside-like comment UI.
2. Insert icons into text-like flow.
3. Show actual display size, default 100×100 for DC profile.
4. Keep GIFs animated.
5. Multi-piece icons render in piece order.

Done when: User can visually test a collection before export.

## Phase 6 — Export and validation
1. DCInside validation.
2. Custom profile validation.
3. Filename modes: sequence and alt.
4. `alts.txt` generation.
5. Open export folder / txt file.
6. Hard error vs soft warning UI.
7. Export all pieces in persisted order.

Done when: Export creates files matching order, dimensions, format, size constraints, and alt txt.

## Phase 7 — QA and packaging
1. Run lint/test/build/tauri build.
2. Use Codex `/review` on uncommitted changes.
3. Manual checklist: import, restart, reorder, edit, GIF, export, duplicate/delete.
4. Update `docs/FEATURE_INVENTORY.md` statuses.

Done when: MVP is usable end-to-end and feature inventory has no accidental gaps.
