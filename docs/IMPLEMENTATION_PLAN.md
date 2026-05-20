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
   - `originals/`
   - `generated/crops/`
   - `thumbnails/source-files/`
   - `previews/collections/`
   - `exports/`
   - `temp/import/`
   - `temp/export/`

Done when: Collection CRUD persists across restart.

Stage 07 status: SQLite initialization, app data folder creation, schema
migration, collection CRUD commands, `list_icons`, frontend command wrappers,
and persisted main collection loading are implemented. `update_icon_order`,
actual image import, source file copy, and cover assignment remain in later
stages.

## Phase 2 — Explorer UI and import
1. Main screen collection grid.
2. Breadcrumb navigation.
3. `+` menu and drag-and-drop zones.
4. Tauri file dialog and import command.
5. Copy originals into library with content hash.
6. Create icon and first piece records.
7. Set first icon as collection cover automatically.

Done when: Multiple jpg/png/gif files can be imported and are still visible after restart.

Stage 08 status: Image file import is implemented for jpg/jpeg/png/gif through
multi-file file input and drag-and-drop. Imported originals are written to the
Tauri app data `originals/` library by SHA-256, source rows are deduplicated by
hash, thumbnails are generated under `thumbnails/source-files/`, icon and piece
rows are persisted in `icons.order_index` order, and the first imported icon is
assigned as the collection cover. Users can change the cover to another imported
icon from the collection grid. Folder import, crop editing, drag reorder, and
final export remain later stages.

## Phase 3 — Icon management
1. Icon grid/list view.
2. Multi-select with Ctrl/Shift and keyboard Delete.
3. Context menus: edit, rename, duplicate, delete, set as cover.
4. dnd-kit reorder with persistence.
5. Inline alt editing and duplicate/invalid highlighting.

Done when: The collection behaves like a file explorer and `order_index`/alt text persist.

Stage 09 status: Icon grid interactions are implemented with dnd-kit sortable
tiles, Ctrl multi-select, Shift range select, keyboard Delete, and a right-click
context menu for delete, duplicate, edit, and representative image selection.
The stage-safe edit action focuses the inline alt editor; crop editing remains
reserved for Stage 10. Alt values are edited inline, validated in the frontend
and Rust command layer, rejected when duplicated, and persisted in
`icon_pieces.alt_text`. Drag reorder persists to `icons.order_index`; duplicate
and delete operations are durable SQLite operations that preserve imported
original files.

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

Stage 10 plan: implement the editor as a vertical slice without final export.
The slice adds a right-side React editor panel, React-Konva crop box movement
and resizing, fixed-mode presets, configurable effective cell size, persisted
crop metadata, icon shape/piece reconciliation, GIF loop metadata persistence,
and preview derivative generation while preserving immutable source files.

Stage 10 status: right-side editor panel, source-image crop canvas, single and
double icon shape modes, free/fixed crop behavior, split lines, fixed presets,
icon-level cell-size overrides, persistent crop metadata, preview derivative
generation, and source preservation tests are implemented. Final export and the
preview simulator remain later phases.

Stage 11 status: GIF import now records frame count and source loop metadata,
editor crop apply processes every GIF frame into animated preview and piece GIFs,
loop settings are encoded for preserve/infinite/once/count, regenerated preview
URLs are cache-refreshed in the grid/editor, generated piece files are checked
for png/gif format and collection byte limits, and Rust tests verify animation,
delays, loop metadata, crop resize dimensions, and original source preservation.
The usage preview simulator does not exist yet and remains Phase 5 work.

## Phase 5 — Preview simulator
1. DCInside-like comment UI.
2. Insert icons into text-like flow.
3. Show actual display size, default 100×100 for DC profile.
4. Keep GIFs animated.
5. Multi-piece icons render in piece order.

Done when: User can visually test a collection before export.

Stage 12 status: the collection screen now has a local usage preview mode with
a DCInside-style comment composer, 100×100 icon display, click-to-insert icon
groups, multi-piece rendering in piece order, alt labels in the palette and
inserted summary, generated piece preview usage when available, and GIF preview
URL refresh so animated GIFs continue replaying in the simulator. This stage
does not implement upload, login, posting, scraping, or final export.

## Phase 6 — Export and validation
1. DCInside validation.
2. Custom profile validation.
3. Filename modes: sequence and alt.
4. `alts.txt` generation.
5. Open export folder / txt file.
6. Hard error vs soft warning UI.
7. Export all pieces in persisted order.

Done when: Export creates files matching order, dimensions, format, size constraints, and alt txt.

Stage 13 status: export is implemented as a Tauri/Rust vertical slice with
persisted DCInside/Custom profile settings, collection/icon effective-size
export, saved icon order plus piece order expansion, sequence and alt filename
modes, `alts.txt` and `export-manifest.json`, post-render 2MB validation,
DCInside count/format/200×200/alt/duplicate checks, strict-warning handling,
multi-piece splitting, animated GIF export with saved loop settings, and
open-folder/open-`alts.txt` commands. The collection UI exposes an export
dialog with settings, validation results, and completion actions.

## Phase 7 — QA and packaging
1. Run lint/test/build/tauri build.
2. Use Codex `/review` on uncommitted changes.
3. Manual checklist: import, restart, reorder, edit, GIF, export, duplicate/delete.
4. Update `docs/FEATURE_INVENTORY.md` statuses.

Done when: MVP is usable end-to-end and feature inventory has no accidental gaps.

## Stage 14R — Consolidated missing feature closure
1. Close the final-review todo items only: icon display-name rename, icon thumbnail
   override, collection duplicate UI, collection/icon size settings UI, folder import,
   startup route/view restore, explicit library cleanup, reveal original/export result,
   and standalone 200×200 JPG/PNG collection cover import.
2. Preserve existing import/edit/export behavior and original file copies.
3. Keep F048 transparency/JPG warnings, mark only the optional 5px margin heuristic
   as user-deprioritized and non-blocking.
4. Rerun final feature review and update `docs/FEATURE_INVENTORY.md`.

Done when: `F011`, `F012`, `F024`, `F035`, `F053`, `F059`, `F060`,
`F061`, and `F062` are implemented and verified; `F048` is correctly mapped.

## Stage 15L - Dependency license review note closure
1. Review dependency license notes against upstream npm/crates metadata and installed license files.
2. Convert resolved notes into durable manual review resolutions in the license notice generator.
3. Regenerate `THIRD_PARTY_LICENSES.md` so public/internal tracked docs show no unresolved notes for reviewed dependencies.
4. Confirm GIF, image resize, and rescale pipeline crates are explicitly covered and forbidden optimizer packages remain absent.

Done when: license generation records the manual resolutions, image/GIF/resize coverage is documented, and license guardrail commands pass or mark optional missing tools as skipped.

## Stage 16D - Public user manual and GitHub Pages docs
1. Add a static HTML manual at `docs/index.html` for GitHub Pages `/docs` publishing.
2. Capture fresh, wide screenshots from the `디시콘 모음 3` collection so the app toolbar, sidebar, grid, editor, preview, and export workspace are visible without narrow-window layout breakage.
3. Include enough screenshots to explain selection, alt batch editing, context menus, blank/working icons, single crop box resizing, horizontal double icons, advanced GIF/text editing, usage preview, and export validation.
4. Update README documentation links and screenshots to point at the new manual assets instead of rejected crop/full-box imagery.

Done when: the manual references only the new `docs/manual-assets/manual-*` screenshots, all linked assets exist, and docs-only checks pass.

Stage 16D status: the public GitHub Pages manual exists at `docs/index.html`, README links to the manual and tracked manual screenshots, and the original 11 `docs/manual-assets/manual-*` screenshots are tracked.

## Stage PUBLIC_USER_MANUAL_AND_RELEASE_READINESS
1. Extend the public manual beyond the original editor/export walkthrough to include sheet import/export/reimport, GIF frame work sheets, context-menu workflows, icon memos, shared sheet presets, auto-detect proposals, GIF FPS preview behavior, file preservation, and release links.
2. Add a tracked release-readiness summary outside ignored `docs/QA_*.md` files.
3. Add tracked manual screenshots for the newer workflows under `docs/manual-assets/manual-*`.
4. Update README and feature inventory so public users can find the manual, release checklist, license policy, and third-party notices.
5. Run docs link/asset validation plus lint, tests, build, license guardrails, Rust tests, and Tauri packaging where reasonable.

Done when: public docs cover all currently implemented production workflows, all manual image links resolve to tracked assets, MIT/license-readiness links are visible, and verification commands pass or have explicit non-product blockers.

## Stage PROFESSIONAL_SPRITE_SHEET_TOOLS_MVP - Reversible sheet tools
1. Add written design docs for static sheet import, work sheet export, manifest reimport, PNG alpha handling, page splitting, GIF frame sheet future scope, manual slices, and auto-detect.
2. Implement Rust `sheet` modules for grid math, static import, static export, manifest validation, static reimport, preview metadata, and future GIF/manual slice scaffolding.
3. Register Tauri commands:
   - `analyze_sheet_grid`
   - `preview_sheet_slices`
   - `import_sheet_cells`
   - `export_edit_sheet`
   - `reimport_edit_sheet`
4. Implement collection toolbar UI:
   - `시트 가져오기`
   - `작업 시트`
5. Keep GIF frame sheet, manual slices, and auto-detect out of visible action menus until their commands are fully implemented.
6. Add Rust and frontend tests for grid math, alpha preservation, manifest mapping, output dimensions, page estimates, and selection behavior.

Done when: static sheet import/export/reimport works without overwriting originals, clean sheets preserve alpha, guide sheets are separate, manifests roundtrip cells, and GIF frame sheet is explicitly documented as the next stage.

## Stage CONTEXT_MENU_SHEET_WORKFLOW_AND_PRESETS_MVP
1. Reuse implemented GIF frame sheet export/reimport commands and expose them through GIF-only icon context menu actions.
2. Add collection-card context menu duplication using `duplicate_collection` with numbered copy names.
3. Add persistent icon notes, note context menu actions, and a hover note indicator beside icon names.
4. Add selected-icon static work sheet export from icon multi-selection without changing whole-collection export behavior.
5. Add persistent sheet grid presets shared by import, static export, and GIF frame export, including protected built-in presets.

Done when: every new context menu action is wired to a real command or dialog, selected export uses only selected icons, notes survive reload, presets can be saved/applied/defaulted across import/export, and original source files remain preserved.

## Stage MANUAL_SLICE_MODE_MVP
1. Replace manual-slice placeholder backend with real rectangular slice analysis, import, and metadata save/load.
2. Wire `직접 Slice 지정` into `SheetImportWizard` without adding auto-detect or atlas-style packing behavior.
3. Add `ManualSliceCanvas` for drag-create, move, resize, exact X/Y/W/H editing, include/exclude, duplicate/delete, and metadata save.
4. Import included in-bounds slices as new PNG icons while preserving the original sheet and PNG alpha.
5. Add Rust coverage for bounds analysis, metadata roundtrip, original preservation, alpha-preserving import, and order.
6. Add frontend render coverage for the manual slice production surface.

Done when: users can manually define rectangular slices from a source sheet and import them as new icons without overwriting originals; auto-detect is left for the next stage.

## Stage AUTO_DETECT_SHEET_SLICING_EXPERIMENTAL
1. Add a backend proposal command that analyzes PNG/JPG/JPEG sheets without importing.
2. Detect likely separator rows/columns from alpha transparency and solid background color.
3. Return fixed-grid settings with confidence and warnings.
4. Add a `자동 감지 (실험)` UI path in `시트 가져오기`.
5. Let users apply a proposal into the existing grid overlay/review workflow.

Done when: auto-detect never auto-imports cells, proposal application still requires grid overlay and cell review, transparent separator / solid background / no-proposal cases have Rust tests, and frontend proposal rendering has test coverage.
