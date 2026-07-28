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

## Stage REVIEW_REMEDIATION_2026_07 - Reliability and usability repair
1. Keep ordinary export warnings advisory and non-blocking; do not change warning policy in this repair stage.
2. Bound untrusted image, sheet, and GIF inputs in both frontend payload creation and Rust decoding; process normal folder imports one file at a time.
3. Expose collection soft-delete through toolbar, context menu, and Delete key, then invalidate the sidebar collection list after collection mutations.
4. Make the collection command bar wrap below the title at the default window width.
5. Guard editor state loading against stale async responses and remount the editor when its icon changes.
6. Repair corrupted Korean GIF errors and add dialog semantics, focus trapping, Escape handling, and focus restoration to custom modal surfaces.
7. Commit an npm lockfile, remove the pnpm lock, remove unused vulnerable CLI dependencies, and verify a clean npm install.

Done when: frontend lint/tests/build and Rust tests pass, clean `npm ci` reports no vulnerabilities, license guardrails pass, and no reviewed P1/P2 item outside the explicitly excluded strict-warning behavior remains.

## Stage EDITOR_OUTPUT_PREVIEW_PLACEMENT - Editor feedback visibility
1. Move the live output preview directly below the source crop canvas.
2. Keep the preview visible at the top of the editor scroll area while the user changes shape, size, crop mode, position, and GIF settings.
3. Rename the user-facing label from `처리 미리보기` to `출력 미리보기` and show draft/display/output-size context.
4. Preserve existing GIF playback, text overlay, and single/horizontal-double/vertical-double rendering behavior.
5. Add a focused frontend render test and rerun lint, tests, and the production build.

Done when: crop changes and their output can be compared without scrolling to the end of the settings, the preview remains compact at custom sizes, and the frontend verification commands pass.

Status: implemented. The output preview now follows the crop canvas, remains sticky while settings scroll, reports draft/display/output-piece context, and fits large or vertical previews into a bounded 220×128 visual area. Focused and full frontend tests, lint, and the production build pass.

## Stage RELEASE_0_1_2 - Windows patch release
1. Bump all package, Tauri, Cargo, lockfile, notice, and public-document version surfaces from 0.1.1 to 0.1.2.
2. Publish release notes covering bounded imports, Explorer usability repairs, accessibility/reliability fixes, and the compact sticky editor output preview.
3. Run frontend, Rust, license, and production packaging checks.
4. Build the Windows release executable, MSI, and NSIS setup, then generate and verify an NSIS-only SHA-256 checksum file.
5. Push the reviewed branch, merge the existing pull request, tag merged `main` as `v0.1.2`, and publish the NSIS setup plus checksum through GitHub Releases.

Done when: the public `v0.1.2` release points to merged `main`, exposes the matching unsigned NSIS installer and checksum, and retains the MSI locally pending clean-machine QA.

## Stage EDITOR_COMPLETENESS_REFERENCE_AND_RESET_UX
1. Record the user-approved 1-5 scope and the Aseprite/permissive-editor reference
   boundaries in `docs/EDITOR_COMPLETENESS_DESIGN.md`, covering both existing static
   multi-emoticon sheet import/export and implemented frame-sheet animation workflows.
2. Treat Aseprite as UX-only reference because its main application source and official
   binaries are under the Aseprite EULA; some separately identified modules are MIT,
   but no Aseprite code, binaries, UI assets, screenshots, themes, or icons are copied
   or bundled. Apply the documented offset, cell size, padding, sheet-type/read-order,
   rows/columns, empty-cell, and metadata concepts to PMTCONCON Studio's independent
   Korean UI and manifest model.
3. Rename scope-ambiguous reset actions:
   - crop-only reset → `크롭 기본값`
   - persisted-draft restore → `저장값으로 되돌리기`
   - usage-preview clear → `미리보기 비우기`
   - sheet split reset → `분할 설정 초기값` and invalidate stale cell analysis
4. Add a visible public-manual entry in the app sidebar and explanatory tooltips for
   collection duplication, sheet import, and work-sheet export.
5. Add focused frontend coverage or a documented manual verification path, then run
   frontend lint, tests, and production build.

Done when: current reset actions state their exact scope, existing clone/note/sheet
features are easier to discover without adding dead actions, the reference/license
decision is documented, and frontend checks pass.

Status: implemented. Reset/restore labels now name their exact scope; sheet setting
changes, source replacement, reset, preview refresh, and auto-detect invalidate or gate
stale analysis; the sticky sidebar opens the public manual; clone/sheet actions explain
their result; memo add/edit is visible on detailed tiles; and command labels wrap only
as whole buttons. `npm.cmd run lint`, all 14 frontend test files (60 tests),
`npm.cmd run build`, `npm.cmd run license:forbidden`, and `git diff --check` pass.

## Stage NONDESTRUCTIVE_TRANSFORMS_MVP
1. Add persisted icon transform metadata for horizontal/vertical flip and quarter-turn
   rotation without changing the original source file.
2. Apply the same transform recipe in editor preview, generated piece previews, usage
   preview, optimization hashing, and final export.
3. Process every GIF frame while preserving timing/loop metadata.
4. Define and test non-square custom cells plus horizontal/vertical multi-piece rotation
   semantics before exposing the controls.
5. Add Rust transform tests, frontend control tests, migration tests, and visual fixtures.

Done when: static/GIF transforms persist across restart, preview and export match, multi-
piece order remains correct, originals are untouched, and required checks pass.

Status: implemented. The editor now composes four Korean transform commands into eight
canonical visual states, swaps non-square cell dimensions and double-icon shape on odd
quarter turns, and keeps piece IDs/alt attached to visual content. SQLite, generated
previews, usage preview, optimization hashes, static/GIF export, static work sheets, and
GIF frame sheets use the same recipe. Source replacement resets crop/transform and
regenerates current/piece previews; GIF frame-sheet reimport only activates a variant
when the manifest source/render-recipe hash still matches. `cargo test` passes 98 tests,
all 15 frontend test files pass 66 tests, and lint, production build, Rust format,
forbidden-dependency guard, and `git diff --check` pass.

## Stage FRAME_SHEET_TO_GIF_MVP
1. Reuse the same offset/cell/padding/order grid analysis already used by static
   multi-icon sheet import for arbitrary PNG/JPG/JPEG frame sheets.
2. Add row/column reading order, reverse order, selected-frame strip, drag reorder,
   duplicate/delete, and per-frame duration in milliseconds.
3. Add sticky realtime playback with one/infinite/count repetition plus a generation
   direction of forward/reverse/ping-pong; bake reverse/ping-pong into the generated
   frame sequence in the first MVP instead of promising an independent direction field.
4. Render and measure the GIF before commit, warn for the active byte limit, preserve
   the original sheet, and register the result as a new animated icon.
5. Add resource-limit, order, timing, loop, alpha, palette, and native workflow tests.

Done when: a manifest-free frame sheet can safely become a new editable/exportable GIF
without a full layer/cel editor, and the original sheet remains recoverable.

Status: implemented. The shared static-sheet analyzer and presets feed a dedicated,
bounded GIF recipe (`cell index + duration`), so duplicate frames do not duplicate
source pixels. The frame strip supports Ctrl/Shift selection, pointer/keyboard reorder,
duplicate/delete/reverse, per-frame timing and FPS convenience; sticky playback covers
once/infinite/count and baked forward/reverse/ping-pong with reduced-motion behavior.
The native renderer enforces final frame/pixel/grid limits, quantizes timing, measures
actual GIF bytes, warns for byte/partial-alpha/palette loss, and returns a render hash.
Commit re-renders and verifies that hash, then transactionally creates the animated
source/icon/crop/piece and versioned provenance while separately preserving the sheet;
the result view can reveal that original sheet in Explorer. Ping-pong frame limits are
checked before expansion, and one-shot ping-pong returns to its starting frame.
All 131 Rust tests and 106 frontend tests pass, as do lint, production build, Rust
format, forbidden-dependency guard, and `git diff --check`.

## Stage CURATED_EFFECTS_MVP
1. Add migration `010_icon_effect_recipes.sql` with a one-to-one, revisioned,
   versioned ordered JSON recipe for each icon. Validate effect count, unique step IDs,
   kinds, numeric bounds, modes, and RGBA colors in Rust before either rendering or
   persistence; use the revision to reject stale saves.
2. Implement one shared deterministic Rust renderer for pixelate, color adjustment,
   grayscale/sepia, blur/sharpen, outline, and shadow without adding a dependency.
   Apply the ordered recipe after crop/resize/transform to the combined viewport and
   before multi-piece splitting, on every GIF frame.
3. Carry the same recipe through saved editor previews, final export, optimization
   baselines/cache hashes, static edit sheets, and GIF frame sheets so stale artifacts
   cannot remain active after an effect change.
4. Add a compact, keyboard-accessible Korean effect panel with add, enable/disable,
   parameter editing, order movement, individual remove, saved-value restore, and
   disable-all controls. Keep the native current-cell preview authoritative for effect
   composition while clearly noting that the final export format, resize filter, and
   optimization can change color and byte size.
5. Add migration, recipe validation/hash, fixed-pixel image, GIF timing/frame, multi-
   piece boundary, persistence, passthrough invalidation, and frontend model/surface
   tests. Run full regression, build, formatting, and license guardrails.

Done when: effect settings are non-destructive and persistent, static/GIF output is
deterministic, preview/export fixtures match, and no unreviewed dependency or asset is
introduced.

Status: complete. The existing Rust `image`/`gif` pipeline remains authoritative and no
dependency was added. Full Rust/frontend tests, lint, production build, rustfmt,
forbidden-dependency and available license guards pass; optional `cargo-deny` and
`cargo-about` remain explicitly skipped because they are not installed.
Completed effect previews are bounded to the most recent eight requests per icon while
in-progress requests are preserved. The UI confirms unsaved crop/transform, text, and
effect changes and locks stale revision conflicts until the saved value is reloaded.

## Stage MOTION_EFFECTS_MVP
Status: complete (2026-07-26). The implementation uses five completed vertical slices:
versioned persistence and validation; deterministic frame rendering; native
measure/preview/save commands; shared export/optimizer/sheet integration; and a
keyboard-accessible Korean editor surface.

1. Persist a revisioned `pmtcon-motion-v1` recipe per icon and validate bounded timing,
   seed, interpolation, edge mode, and category parameters before render or save.
2. Provide 16 presets across spatial motion, procedural displacement, animated
   color/opacity, and overlays, with at most one enabled effect per category and a fixed
   `spatial → displacement → color/opacity → overlay` order.
3. Turn a static source with enabled motion into a measured GIF schedule; evaluate an
   existing GIF from cumulative frame timestamps instead of frame indices, preserving
   effective loop behavior and using the persisted seed for reproducible variation.
   For ping-pong, reflect the final composited timeline without duplicating endpoints,
   so motion itself reverses consistently in preview, export, and GIF frame sheets.
4. Apply `saved static effects → motion` to the combined multi-piece viewport and split
   pieces only afterward. Use the same native recipe in editor preview, saved preview,
   export, optimization, GIF frame sheets, and static work sheets.
5. Show play/pause, OS reduced-motion state, frame count, duration, effective loop,
   clipping, total encoded bytes, and piece bytes. Treat this as editor-preview
   measurement; final export size is validated again with the selected export profile
   and optimization settings.
6. Snapshot render inputs and release the shared SQLite lock before GIF encoding, then
   recheck revisions and render signatures before commit. Keep request artifacts
   bounded and remove superseded motion artifacts only when no durable path references
   them.
7. Export static work sheets from the 0ms poster frame with an explicit animation-loss
   warning and a processed-output `render_recipe_hash` stale guard; use GIF frame sheets
   when every frame, duration, and loop must round-trip. Treat imported manifests as
   untrusted: bound bytes/pages/cells/pixels, reject unsafe IDs and filenames, check
   crop arithmetic, require the selected target ID to match, and reuse one decoded
   page snapshot through validation and encoding.
8. Preserve motion recipes when duplicating icons or collections without sharing mutable
   preview ownership. Add no new runtime dependency.

Done when: all four motion categories are non-destructive, persistent, deterministic,
preview/export-consistent, size-measured, keyboard accessible, and safe for static and
animated sources.

Completion evidence: motion recipe/hash/migration coverage; deterministic timestamp,
seed, inverse-sampling alpha, loop-seam, static-to-GIF, existing-GIF timing, measured
preview/save recheck, multi-piece split, export/optimizer/static-sheet/GIF-frame-sheet,
clone, artifact cleanup, and frontend editor/preview model regressions. Aggregate lint,
test, build, formatting, diff, and license results are recorded in the final Stage Gate
rather than frozen as test counts in this plan.

## Stage COLLECTION_DUPLICATE_COMPLETENESS
1. Audit and copy icon kind/readiness/placeholder, text overlay, transform/effect
   recipes, and all newer durable visual metadata.
2. Map cloned profile/icon/piece IDs correctly and preserve cover, alt, note, shape,
   crop, size, loop, and ping-pong behavior.
3. Prevent mutable preview paths or active variants from causing cross-collection edits;
   regenerate or safely share immutable artifacts by an explicit policy.
4. Add regression fixtures for text GIFs, working placeholders, optimized variants, and
   horizontal/vertical multi-piece icons.

Done when: the duplicate initially previews and exports identically to the source, then
either copy can be edited without changing the other.

Status: complete (2026-07-26). Collection duplication now assigns new collection,
profile, icon, piece, preset, recipe, and variant IDs; owns independent current/piece
preview and effective active-variant files; and recalculates variant hashes against the
cloned IDs and profiles. Stale or missing variants fall back to the durable render
recipe, while optimization jobs and `last_export_path` remain reset. Collection-scoped
sheet presets and frame-sheet GIF provenance are copied, the UI prevents concurrent
duplicate requests, and animated horizontal/vertical multi-piece exports are verified
byte-for-byte before subsequent source artifact removal.
## Stage RELEASE_0_2_0
1. Audit the complete Stage 1-5 worktree as one intentional release scope and remove
   stale future-state documentation before staging.
2. Advance package, lock, Cargo, and Tauri metadata from the already-published 0.1.2 to
   0.2.0; regenerate third-party notices and write user-facing release notes.
3. Re-run Rust/frontend regression, production build, formatting, diff, and license
   guardrails from the final versioned source state.
4. Build the x64 release executable, MSI, and NSIS setup; verify executable/MSI metadata,
   isolated startup, SQLite migration integrity, Authenticode state, and the NSIS SHA-256.
5. Publish through a reviewed branch and PR, tag merged `main` as `v0.2.0`, and upload
   only the unsigned NSIS setup plus matching checksum to GitHub Releases. Keep MSI
   publication deferred until clean-machine install/uninstall QA.

Done when: the public `v0.2.0` release resolves to merged `main`, contains the reviewed
release notes and exactly the selected NSIS/checksum assets, and their remote metadata
and digest match the locally verified release candidate.

Status: release candidate complete (2026-07-26). Source tests, packaging, checksum,
executable/MSI metadata, isolated launch, and database migration integrity pass. Native
click-through automation is environment-blocked by the Windows automation sandbox ACL;
the packaged process/window/database smoke, 154 frontend tests, and 176 Rust tests pass.
Remote PR/merge/tag/release publication is recorded by the final Stage Gate rather than
frozen as mutable remote state in this plan.

## Stage AI_INTEGRATION_DESIGN

1. Audit immutable originals, replace-source behavior, preview caches, effect/motion
   revisions, export variants, cleanup and collection clone ownership.
2. Compare direct image APIs, consumer website handoff and user-run local endpoints
   against cost, privacy, reliability and MIT dependency constraints.
3. Define provider-neutral request/candidate history, active source rollback, stale
   request handling, exact external-send preview and explicit user consent.
4. Separate static base-source editing from rendered-viewport/new-icon editing, and keep
   GIF/frame-sheet/sprite work behind explicit experimental gates.
5. Reconcile PRODUCT_SPEC, FEATURE_INVENTORY, DECISIONS and the v0.2 editor exclusion.

Done when: the app has one implementable non-destructive AI contract, no provider call
or secret is required for the next stage, and no unimplemented AI surface is exposed.

Status: complete (2026-07-26). `docs/AI_INTEGRATION_DESIGN.md` separates mutable provider
execution requests, immutable candidates/source bytes, icon-scoped versions and a
revisioned active-version pointer. The 2026-07-26 provider revision selects NovelAI Image
API as the first automated/conditional-primary adapter, manual website handoff as a
token-free fallback, Gemini and OpenAI only after separate eligibility/cost gates, literal
loopback as a separate security stage, and poster-frame/native-motion before full GIF or
sprite experiments. No API request, key, dependency or UI surface was added.

## Stage AI_NONDESTRUCTIVE_FOUNDATION

1. Add `icons.original_lineage_id`/monotonic `original_lineage_generation`, nullable safely
   decoded `source_files.has_alpha`, mutable `ai_requests`, immutable `ai_candidates`,
   `icon_ai_versions` and `icon_ai_state`. Backfill every existing icon with a distinct
   lineage, generation 0 and original-only state; missing state is a data error.
   DB lineage defaults plus an atomic state trigger and guarded
   `insert_icon_with_visual_state` helper must cover import, placeholder, duplicate,
   static/GIF sheet commit and both clone paths. Add source-search and orphan-state gates.
   `ai_requests` also stores adapter/contract version, requested/negotiated capabilities,
   provider data tier, retention/consent snapshots as provider-qualified, versioned,
   canonical allowlist JSON bounded to 64 KiB each. Prompt/options use a separate bounded
   allowlist schema that structurally rejects binary/base64 payload, headers, credentials and
   complete provider request/response. Store only a credential mode snapshot. Foundation
   creates no credential binding table/column/FK and rejects `os_vault_ref`. Never store a
   secret or silently select a fallback provider.
2. Enforce nullable `ON DELETE SET NULL` request origins with immutable snapshots,
   `RESTRICT/NO ACTION` candidate/source provenance, `(request_id, candidate_index)`
   uniqueness, a lineage-scoped composite parent FK, and icon-scoped active-version FK plus
   lineage CAS. Keep the base-original source FK independent from mutable
   `icons.source_file_id` and register cleanup refs in the same migration slice.
3. Register validated raw/normalized bytes in immutable content-addressed `source_files`.
   Define `pmtcon-alpha-v1` as any actual non-opaque decoded/display-composited pixel across
   every displayed frame, safe-decode new alpha/animation metadata and lazily backfill
   unknown alpha. Separate mode-specific `payload_input_signature`, full request provenance
   recipe and full `activation_recipe_signature`. Temporary input/handoff/staging paths are
   never source of truth.
4. Centralize fail-closed `EffectiveVisualSource` resolution. Split editor DTOs into
   original metadata/reveal and effective canvas/render sources; migrate preview, export,
   optimizer, GIF-FPS, static/GIF sheets and cover fallback. Broken state/file/SHA/decode
   blocks render/export and exposes repair instead of silently using the original.
5. Make `processed_asset_variants.source_file_id/source_hash` identify the effective render
   source. Backfill a legacy nullable ID only when owning-original ID/SHA match; otherwise
   deactivate it and regenerate promoted preview natively. Bounded-check legacy artifact
   file/byte-size/format/dimensions and backfill a new `output_sha256`; invalid rows remain
   stale with NULL provenance/digest. Require matching non-null source ID/SHA and output
   digest for new writes/lookups. Rebuild with a nullable source FK/digest. Add static
   `pmtcon-sheet-v2` and GIF `pmtcon-gif-frame-sheet-v2` fields for
   original ID/hash/lineage/generation plus effective ID/hash, reject stale mismatches, and
   allow v1 only for AI-inactive generation-0 icons. Guard the documented non-render direct-
   `icons.source_file_id` allowlist.
6. Implement same-canvas identity candidate import, source comparison, new-icon creation
   and compatible activation/restore as prepare → staging render → full-recipe/lineage CAS → same-volume
   durable rename → pointer/preview commit, with DB rollback, file compensation and crash
   orphan sweep. A base-source new-icon operation maps every lineage, inserts the candidate
   child/active state before final effective-source resolution, then commits icon, variants
   and previews once; failure leaves no partial icon. Selecting an existing version creates
   no duplicate version row; stale results remain inactive candidates. Arbitrary-size raw
   candidate normalization and raw/normalized/final A/B preview belong to the following
   `AI_CANDIDATE_NORMALIZATION_AND_WORKSPACE` stage.
7. Treat ordinary image replacement as a new lineage even for identical bytes: stage and
   durably promote source/preview, atomically reset geometry/AI state, increment lineage
   generation and activation revision, supersede old-signature requests, and reject
   old-lineage activation.
8. Extend both clone paths in fixed order: durable icon/piece/recipes, one-to-one historical
   lineage map with preserved generations, complete AI DAG/state, target effective source,
   compatible active variants, then preview paths. Copy/remap a variant only when source and
   final-target source/crop hashes, format and ID/path-independent output-profile
   compatibility match. Otherwise skip its row/bytes/promoted preview and render from the
   final effective source; never relabel old bytes. Compensate every failure, share request/
   candidate bytes without duplicating cost, and never auto-attach a pending late result.
9. Protect candidate/version/soft-deleted-history sources in cleanup. Delete terminal
   transfer payloads promptly, expire manual awaiting packages after 7 days or one explicit
   30-day extension, and sweep only unreferenced staging/final crash orphans older than 24h.
   Permanent AI-history deletion must list lost rollback points and shared clone/descendant
   references and require separate confirmation.
10. Test restart rollback, original invariants, hostile input, `pmtcon-alpha-v1` static/GIF
    scanning/backfill, all render consumers, fail-closed repair, signatures/CAS, manifest
    legacy rules, variant ID/output-digest backfill/stale/native-preview repair, `A → B → A`
    generation-gated v1, all icon-create paths, source replacement, cross-icon and
    same-icon/cross-lineage FK rejection, cleanup survival, pending-clone late result,
    multi-lineage mapping, base-source partial-icon compensation, old-variant-byte non-
    relabeling and active AI + promoted optimized GIF + multi-piece clone rollback. Add
    provider-contract regressions proving that snapshot allowlists/size bounds reject full
    requests, adapter disable blocks only new calls, session token clear preserves history,
    provider change creates a new request/consent, and failure creates no fallback request.
    Use local fake/manual providers only.

Done when: a validated local candidate can be activated and rolled back to the original
or any previous icon version after restart, every preview/export path agrees on the
effective source, corrupt state fails closed, clone and cleanup invariants hold, and no
network call or API key is involved.

Status: complete (2026-07-27). Migrations `012`–`015`, repository invariants,
`EffectiveVisualSource`, local static JPG/PNG candidate import, default new-icon creation,
advanced compatible current-icon activation, original/previous-version rollback, preview
repair, effective render consumers/manifests, clone/cleanup integration and the Korean
review UI are implemented. The foundation makes no network request, accepts or stores no
API key, and can be removed without changing preserved originals. Verified with 207 Rust
tests, 176 frontend tests, TypeScript lint, production build, rustfmt, `cargo check`,
dependency-license guards and `git diff --check`.

## Stage AI_CANDIDATE_NORMALIZATION_AND_WORKSPACE

1. Remove the exact-current-dimension restriction from local static JPG/PNG candidate
   import. Preserve the provider/manual raw file unchanged, validate it with bounded decode
   limits, and replace shared cover-image wording with AI-specific error codes/messages.
2. Implement backend-owned `pmtcon-ai-normalization-v1` for `contain_pad` and
   `cover_crop`, 3×3 alignment, Lanczos3/Nearest and bounded RGBA padding. Use the current
   effective base-source canvas as target, output immutable PNG when conversion is needed,
   and store the canonical recipe/hash plus raw and normalized source identities. Reuse the
   existing `icon_ai_versions` fields; do not add a migration unless implementation proves
   a missing invariant.
   Materialize versions by icon/lineage/candidate/normalization-recipe hash: reuse the same
   recipe version, but allow a distinct version when the same candidate uses a different
   fit/alignment recipe.
3. Add lazy native normalization/final-render preview with a signature over candidate SHA,
   target canvas, recipe, lineage/generation, activation revision and full native recipe.
   Recompute every authoritative field in Rust during apply/create, reject stale previews,
   and never trust a frontend path, source ID or target dimension.
4. Replace the long editor `<details>` workflow with a compact source summary and large
   in-app `AiWorkspaceDialog`. Provide candidate rail, original/raw/normalized/final
   comparison, checkerboard/zoom, explicit alpha/crop warnings, separate current/new-icon
   compatibility and a fixed action bar at 1200×760. Keep unimplemented provider controls
   absent.
5. Make the default action `새 아이콘으로 추가`, keep current-icon use visible but
   secondary, and add post-create `새 아이콘 열기`/`목록에서 보기`/`계속 후보 비교`.
   Add an explicit route/grid reveal request, show repeated use of the same candidate, and
   return review plus editor state from source mutations as one post-commit result.
6. Consolidate announcements to one dialog status/alert region, give candidate choices
   unique accessible names, show disabled reasons in visible text, restore focus on close,
   and cover keyboard, reduced-motion, stale, mutation-success/list-refresh-failure and
   narrow-layout behavior.

Done when: an arbitrary-size static AI result can be imported without changing the icon,
reviewed as raw and deterministic normalized/final output, safely added or compatibly
activated, and found immediately after new-icon creation. Raw/original/version sources
remain locally rollback-safe; 1200×760 and keyboard flows pass; no network call, API token,
dead provider menu or new dependency is involved.

Status: complete (2026-07-28). AI-UX-1, AI-UX-2 and AI-UX-3 are complete; the
umbrella stage is closed. AI-UX-1 candidate normalization and safe apply are
implemented: arbitrary-size static JPG/PNG files remain immutable raw candidates;
backend-owned contain-pad/cover-crop normalization supports 3×3 alignment and
Lanczos3/Nearest filters; raw/normalized/final previews carry an explicit signature over
the candidate, target, recipe and current lineage/revision/native recipe; and both the
default new-icon path and compatible current-icon path recompute that contract before
commit. Normalized output is a separate immutable source, and original/previous-version
rollback remains local and provider-independent.

AI-UX-1 final audit fixes are also complete. Preview reports decoded final-render and
piece dimensions and applies the same per-piece byte limit as current/new-icon commit;
`maxBytes` participates in the native recipe stale signature. History exposes a parsed
normalization summary and keeps damaged inactive candidates/versions visible as
unavailable while apply/restore remain fail-closed. A committed mutation is distinguished
from a later editor refresh failure. Source/thumbnail compensation and component-wise
no-follow preview promotion cover DB failure and Windows reparse-point paths.

AI-UX-3 completes the umbrella stage with same-transaction review/editor mutation
results, migration 016 direct-create provenance, explicit open/reveal/continue outcomes,
duplicate-use count/latest guidance, typed tile/editor reveal, async dirty/busy handoff,
topmost nested-modal keyboard ownership and one document-wide live region including
background alt and dnd-kit announcement suppression. Provider request/generation remains
outside this completed stage: NovelAI is the separate planned `AI_NOVELAI_IMAGE_API`
stage, manual web handoff remains planned, and Gemini/OpenAI plus GIF/sprite AI remain
future gated work. The detailed information architecture, labels, normalization math,
DTO boundaries and acceptance criteria remain in `docs/AI_WORKSPACE_UX_DESIGN.md`.


AI-UX-2 status: complete (2026-07-27). The existing local candidate controller now
runs in a 1168×728 in-app workspace, while `EditorPanel` keeps only a compact source
summary. The workspace exposes exactly three implemented views (`결과 가져오기`,
`후보 검토`, `소스 이력`), a large original/raw/normalized/final/overlay comparison
with fit/100% and checkerboard controls, and fixed header/tab/status/action regions.
Below 1024px the candidate rail becomes horizontal and the inspector moves below the
comparison stage. The baseline dialog boundary provides dialog semantics, Escape close
and trigger-focus restoration. Provider generation/token/prompt controls remain absent.
Post-create reveal/open/continue continuity, duplicate-use guidance, combined mutation
DTOs and the complete live-region/reduced-motion/accessibility pass are completed in
AI-UX-3.
Verification passed: lint, 31 frontend files with 224 tests, production build, 231 Rust
tests, and browser QA at 1200×760, 1023×760 and 800×760.
Non-blocking follow-up debt: successful normalization previews are reclaimed by the
validated 24-hour startup sweep rather than immediately during a long-running session,
and the repository-level preview-signature parameter remains optional for test/internal
callers even though the TypeScript and Tauri production boundaries require it.

AI-UX-3 execution plan: completed (2026-07-28).

1. Return `AiReviewState` and `IconEditorState` together from current-icon activation and
   rollback commands so the committed source, crop canvas and icon-list summary advance
   from one authoritative mutation response without a second read.
2. Record exact direct candidate reuse through migration `016_ai_icon_root_creations`,
   without speculative historical backfill. Count only explicit `create_ai_icon_root`
   actions; ordinary icon and collection clones are intentionally excluded. After creation,
   keep the source workspace selected and present explicit `새 아이콘 열기`,
   `목록에서 보기` and `계속 후보 비교` actions; repeated creation must say
   `이 후보로 하나 더 추가` and link to the latest non-deleted directly created icon.
3. Carry a typed reveal request from `CollectionRoute` to `IconGrid`. The grid will select,
   scroll and focus the requested tile and optionally open its editor, while respecting
   the existing unsaved-editor confirmation boundary.
4. Consolidate dialog announcements into one semantic status/alert region, connect field
   errors and disabled reasons, preserve complete tab/radiogroup keyboard behavior and
   focus restoration, and disable nonessential motion for reduced-motion users.
5. Add Rust/React unit and integration coverage, then run lint, frontend and Rust suites,
   production build and Playwright continuity checks before closing the umbrella stage.

Completion evidence: lint PASS; 38 frontend files with 248 tests PASS; production build
PASS; 232 Rust tests and rustfmt PASS; license guard PASS with optional cargo-deny/about
reported unavailable; headed browser QA 13/13 PASS at 1200×760 and 800×760 with overflow
0, document live-region 1, activation/restore follow-up GET 0, nested Export Escape PASS,
and unexpected command/network 0. Evidence is stored under `output/playwright/ai-ux3`.

## Stage AI_UX_CHECKPOINT_PRERELEASE

1. Freeze F135-F137 and F144-F146 on a dedicated `codex/` branch after an explicit
   diff/security/license review. Exclude local Playwright output and preserve the 0.2.0
   stable tag and assets.
2. Synchronize package, Cargo and Tauri metadata to `0.3.0-alpha.1`; document that this
   checkpoint imports local JPG/PNG results but does not call providers, accept keys or
   automate websites.
3. Run format, lint, frontend/Rust tests, production build, dependency/license guards,
   diff hygiene and an NSIS-only Tauri package. Generate and independently verify the
   NSIS SHA-256 checksum. MSI remains unpublished until clean-VM install/uninstall QA.
4. Commit and push the exact scope, open a draft PR, tag that immutable commit, upload a
   draft GitHub prerelease, verify remote asset metadata and downloaded checksum, then
   publish it as prerelease without replacing the 0.2.0 stable release.

Status: in progress on 2026-07-28. The next feature stage remains blocked until this
checkpoint gate reports `READY_FOR_NEXT_STEP: YES`.

## Stage AI_NOVELAI_IMAGE_API

0. Persist immutable `input_stage` on provider requests/candidates independently from
   `icon_ai_versions`, and reject unmaterialized non-`base_source` candidates in the
   new-icon path. Extend startup cleanup to remove only validated, unreferenced empty AI
   preview parents left by a process termination before DB commit.
1. Add a license-reviewed Rust HTTP client and a NovelAI adapter for explicit single-image
   text-to-image, img2img and mask-based inpaint behind the common provider capability
   contract. Preserve provider-raw bytes/dimensions and deterministically normalize to the
   PMTCONCON Studio canvas; never assume 200×200 output, alpha or animation support.
2. Ask only for a Persistent API Token that the user generated in the official NovelAI
   Account UI. Start with session-only/environment test paths, never accept email/password,
   never call account login/token-creation APIs, and never return the token to the frontend.
   New PAT creation invalidates the previous PAT and the token is shown only once: clear the
   frontend input state immediately after passing it over the invoke boundary, never read the
   clipboard automatically, and show explicit clipboard-clear/rotation guidance. Persistent
   storage is outside this stage and requires a separate Stage Gate. That later migration
   must add a secret-free `ai_credential_bindings` parent, nullable request FK with `ON DELETE
   SET NULL`, adapter/provider consistency guards and a reviewed Windows Credential Manager
   integration. Deletion marks the DB binding `deleting` to block new calls, deletes the vault
   entry, then deletes the DB row; failures/crashes remain retryable repair states. Tests must
   prove vault/DB compensation and request/candidate/version/source/active-pointer survival.
   The token itself never enters SQLite.
3. Restrict the first command to exact
   `https://image.novelai.net:443/ai/generate-image`, strict CSP, no URL override and no
   redirects. Add another NovelAI endpoint only with an independent capability fixture.
4. Require one visible user action for one request and one sample. Prohibit background
   queues, hidden chained generation, automatic provider fallback and automatic retry.
   Store adapter-contract version plus requested/negotiated capability snapshots; reject
   unknown model/action schema instead of guessing.
5. Add 18+ service eligibility, exact-send, request-content logging/cache, rights and PAT
   risk confirmation. Store only bounded versioned canonical allowlist policy/consent/data-
   tier snapshots, never account ID, token, binary payload or complete provider response.
6. Show settings and estimated provider units before every request. Explain the general Opus
   0 ImageAnlas conditions: one image, no base image, normal-size range and at most 28 steps.
   The official web Inpaint guide separately documents 0-Anlas Focused Inpainting for Opus
   on regions of large images, but public Image API support and charging parity are not
   documented. Mark every inpaint request `provider confirmation required` until official
   clarification and a user-approved small live pilot establish API behavior. Never convert
   subscription Anlas to USD `actual`/`billed`.
7. Bound timeouts, ZIP/JSON response bytes, decoded dimensions and candidate count; sanitize
   errors and define cancellation without promising charge reversal. Cover text-to-image,
   img2img, inpaint, exact `Authorization: Bearer pst-<token>`, rotated-token `401` guidance,
   429, schema drift, redirects, frontend-input clearing/non-echo, logging redaction and no-
   retry with mock HTTP fixtures. Normal automated checks never contact NovelAI.

Done when: an explicitly confirmed, human-initiated request creates one inactive candidate
without exposing the PAT or changing the icon, and activation/rollback remains completely
local and provider-independent.

Status: planned; after the non-destructive foundation Stage Gate this is the selected first
automated-provider stage. Live traffic requires a separate approval and user-supplied PAT.

## Stage AI_MANUAL_WEB_HANDOFF

1. Record `service_surface`, `account_context` and dated typed official refs for terms,
   privacy, data controls, pricing, model/license and attribution as applicable. Unknown
   account context remains explicitly unverified.
2. Package the exact source/mask/references, prompt and secret-free manifest; show that
   payload, context-specific policy links and explicit confirmation before copying the
   prompt and opening the selected official website.
3. Let the user import downloaded output into the common candidate history, compare
   provider-raw/normalized/original candidates, and default to adding a new working icon.
   Treat manually declared or unknown model and uncomputable cost as normal provenance.
4. Do not automate browser login, cookies, DOM upload, scraping or downloads.
5. Delete handoff payload after import/cancel/failure, expire awaiting packages after 7 days
   or one explicit 30-day retention, and expose separate payload/provenance/history cleanup
   actions.

Done when: the full no-API flow works without claiming automatic provenance, billing or
result retrieval, and no sensitive transfer copy is retained indefinitely.

Status: planned fallback; it may follow the foundation without blocking the selected
NovelAI automated-provider stage.

## Stage AI_PROVIDER_EXPANSION

Do not assume a second cloud provider or automatically fail over to one. Evaluate Gemini
only after recording that its current image model has a paid tier and the actual
distribution satisfies age/audience, professional/business, supported-region and
paid-service conditions, then run a surface-specific data-policy and adapter review.

Evaluate OpenAI Image API as a separate optional paid-cloud stage with its own project-key,
desktop-secret, exact-origin, capability, data-policy and cost review. Neither provider may
reuse another provider's consent snapshot.

Evaluate a generic user-run endpoint in a separate stage. Accept only parsed literal
`127.0.0.1`/`[::1]`, reject redirects and other addresses, bound timeout/response/pixel
workload, and disclose that a local workflow can itself call external paid services. Do
not bundle ComfyUI, another AI runtime, model weights, workflows or custom nodes.

Status: future.

## Stage AI_ANIMATED_SPRITE_EXPERIMENTS

Start with a static/poster AI candidate plus the existing deterministic native motion
pipeline. Then evaluate GIF frame-sheet manual handoff, opt-in per-frame requests and
n-up sprite candidates using measured provider units, nullable dated estimated cost and
nullable provider-reported cost, frame consistency,
grid-boundary and manifest fixtures. Do not infer cost savings from request count alone.

Status: future.
