# IMPLEMENTATION_PLAN.md — Codex 실행 계획

## Stage AI_FRICTIONLESS_WEB_HANDOFF (2026-07-28)

1. 정적 단일 JPG/PNG 아이콘 편집에 한해 `웹 AI로 바로 준비` 한 번으로 request-linked
   관리 package, 결정적 구조 prompt 복사와 검토된 공식 사이트 열기를 수행한다.
2. package는 `ai/handoffs/<request-id>`에 사용자용 `upload.png`와 내부
   `manifest.json`·`prompt.txt`로 둔다. credential은 저장하지 않고 DB에는 고정 파일명,
   hash, 구조 metadata와 lifecycle만 저장한다.
3. 현재 Windows 경로는 Explorer에서 업로드 파일을 선택해 사용자가 브라우저로 직접
   끌어 놓는다. native app→browser drag-out이나 provider 업로드 성공은 주장하지 않는다.
4. 내려받은 JPG/PNG를 drop/picker로 받아 format, decode, byte size, 정확한 canvas와
   alpha를 검증한다. 성공 결과는 같은 request의 비활성 candidate로만 저장하고 원본과
   active source는 변경하지 않는다.
5. 구조 오류에는 typed 문제·영향·expected/actual·local action과 결정적 수정 문장만
   제공한다. auth/quota/network/policy 오류는 prompt 문제로 꾸미거나 자동 재시도하지 않는다.
6. 진행 중 최신 세션을 화면 전환·재시작 뒤 복원하고, 명시적 닫기·7일 기본 보존·한 번의
   30일 연장과 crash-safe cleanup intent를 지원한다.
7. GIF frame-sheet, 선택 아이콘 grid, source-free 생성, native drag-out, 주기적 timer cleanup과
   전체 package quota는 F142/F147–F149 또는 별도 후속 Stage Gate로 남긴다.

Done when: 정적 단일 아이콘이 package 준비→Explorer 업로드→prompt 붙여넣기→결과
JPG/PNG drop→즉시 진단→같은 request의 rollback-safe 비활성 후보 저장까지 완료되고,
재시작 복원·닫기·보존 규칙이 검증된다.

Status: static-single vertical slice complete. Rust handoff tests 16/16 and full suite 270/270,
frontend lifecycle tests 10/10 and full suite 297/297, lint, production build and license guards
are the acceptance evidence. GIF/grid/source-free
범위를 이 완료 상태에 포함하지 않는다.

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

Status: complete (2026-07-26), provider execution scope revised 2026-07-28.
`docs/AI_INTEGRATION_DESIGN.md` separates mutable provider execution requests, immutable
candidates/source bytes, icon-scoped versions and a revisioned active-version pointer. The
original provider comparison remains historical; ADR-016 and the active stages below now
limit automation to existing static-image edit, keep NovelAI action/model strings and JSON
response experimental, add safe user-driven web handoff, and keep Gemini mock-tested but
private-pilot eligibility-gated. No API request or key was part of the completed design stage.

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
background alt and dnd-kit announcement suppression. Provider request/generation was
outside that completed checkpoint. Its current 2026-07-28 status is tracked only by the
separate F138-F140 stages below: the static-single safe web handoff is complete,
NovelAI static edit is in progress, and Gemini is partial/private-pilot gated. OpenAI and GIF/sprite AI remain future
work. The detailed information architecture, labels, normalization math, DTO boundaries
and acceptance criteria remain in `docs/AI_WORKSPACE_UX_DESIGN.md`.


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

Status: complete on 2026-07-28 at the `v0.3.0-alpha.1` checkpoint. Its Stage Gate reported
`READY_FOR_NEXT_STEP: YES`; the user then explicitly started the provider/key/web-handoff
implementation described below.

## Stage AI_NOVELAI_IMAGE_API

Implementation scope frozen on 2026-07-28:

0. Keep all provider output inside the completed immutable candidate/version/rollback
   foundation. A successful provider response creates an inactive candidate and never changes
   the current icon automatically.
1. Implement only one explicit edit of the currently selected static JPG/PNG source. Text-to-
   image, mask inpaint, GIF/poster/frame batches, sprite/n-up and multi-result generation are
   outside this gate. They must not appear as live controls.
2. Accept only a Persistent API Token generated by the user in the official NovelAI Account
   UI. Hold it in process memory for the current app session, clear the frontend field after
   the invoke handoff, never echo it, and provide explicit clear/rotation guidance. Do not
   persist it in SQLite, settings, local storage, AI snapshots or logs; do not accept login,
   email or password and do not call account/token-creation APIs.
3. Restrict Rust HTTP to exact
   `https://image.novelai.net:443/ai/generate-image`; reject user URL overrides and redirects.
   The production WebView `connect-src` remains Tauri IPC-only. Accept one bounded JSON image
   response only; reject ZIP, unexpected content types, extra candidates, oversized bytes,
   dimensions or pixel workloads.
4. The public OpenAPI request schema does not enumerate `action` or `model`. Treat the
   adapter's exact values as versioned experimental contract strings, not official enums.
   Show both strings plus the exact source, prompt and scalar options before every request and
   require per-request confirmation. Unknown strings, changed response shape or contract
   version mismatch fail closed instead of guessing or silently switching models.
5. One visible click creates exactly one HTTP request and at most one candidate. Prohibit
   background queues, chained generation, automatic retry and provider fallback. `401`, `429`,
   timeout, 5xx and schema drift return a sanitized error without retransmission. Persist the
   canonical snapshots and provider-ready image hash as `running`, atomically claim
   `awaiting_result` immediately before HTTP, and send nothing if cancellation wins that claim.
   Never promise that cancellation after dispatch reverses a charge.
6. Show service eligibility, potential ImageAnlas use, request-content/data-policy, rights and
   PAT runtime-risk disclosures before send. Provider units remain `공급자에서 확인` unless
   the API returns a documented value; never label a local estimate as actual or billed USD.
7. Mock tests cover exact auth redaction, session clear/non-echo, URL/redirect rejection,
   one-click/one-request, no retry/fallback, JSON-only bounded decode, exact action/model
   confirmation, schema drift, inactive candidate creation and local activation/rollback.
   Normal automated checks never contact NovelAI.

Done when: the mock-tested human-initiated static edit creates one inactive candidate without
exposing or persisting the PAT, and all failure paths preserve the current icon and local
rollback history. Live traffic still requires an eligible adult user, an explicitly supplied
session PAT and one separately approved small potentially charged request.

Status: implementation, mock-contract, browser-flow and local persistence gates complete on
2026-07-28; the stage remains in progress only until the user-approved one-image live pilot.
This is deliberately narrower than the earlier text-to-image/img2img/inpaint proposal.
The desktop is single-instance before startup recovery, and a fail-closed 24-hour source-file
orphan sweep covers a hard crash between managed file creation and DB commit.

Official dated references:
[NovelAI Image API](https://image.novelai.net/docs/index.html),
[OpenAPI schema](https://image.novelai.net/docs/doc.json),
[Persistent API Token](https://docs.novelai.net/en/text/usersettings/account/), and
[subscriptions/ImageAnlas](https://docs.novelai.net/en/subscription/).

## Stage AI_MANUAL_WEB_HANDOFF

1. frontend는 검토된 service-surface enum만 보내고 Rust는 compile-time HTTPS constant로
   공식 사이트를 연다. general `opener:default`는 없고 production `connect-src`는
   IPC-only다.
2. 정적 단일 JPG/PNG source로 request-linked `ai/handoffs/<request-id>` package를 만든다.
   한 user-facing `upload.png`, 내부 manifest/prompt와 hash·geometry·alpha snapshot을
   사용하며 credential·cookie·session은 저장하지 않는다.
3. 한 번의 명시적 준비 동작에서 최종 prompt를 복사하고 공식 사이트를 연다. 사용자는
   Explorer fallback으로 직접 upload/login/generate/download한다. DOM·upload·scrape·
   polling·download 자동화와 자동 retry/provider fallback은 없다.
4. 내려받은 결과를 drop/picker로 검사하고 검증 signature를 같은 bytes에 묶는다. 정상
   결과만 같은 request의 inactive candidate로 저장하며 원본·현재 적용 소스는 보존한다.
5. 최신 진행 세션을 아이콘별로 복원하고, 이전 세션 교체·명시적 닫기·7일 보존·한 번의
   30일 연장·startup/access/prepare cleanup을 제공한다.
6. GIF/grid/source-free/native drag-out/periodic timer cleanup/storage quota는 완료 범위에서
   제외하고 각각 후속 Stage Gate로 추적한다.

Status: complete for `static_icon_sheet/single/edit` on 2026-07-28. This is the generally
available token-free Gemini/NovelAI web path. It does not claim verified provider model,
billing, provenance or website upload success.

## Stage AI_PROVIDER_EXPANSION

### Gemini private static-edit pilot

1. The Gemini static-image-edit adapter and UI may exist only as an eligibility-gated private
   pilot. It is not enabled, advertised or claimed as a general consumer-public release
   feature. As reviewed 2026-07-28, Gemini image models list no free tier.
2. Before key entry or send, require explicit confirmation that the user is at least 18, the
   app/request is not directed toward or likely accessed by under-18s, the user is in a
   supported region, the use is professional/business, and the user owns a paid API key and
   accepts request cost plus the applicable data policy. Any missing confirmation fails
   closed and leaves the API path unavailable; the Gemini official web handoff remains.
3. Keep the Gemini key session-only under the same no-persistence/no-log/non-echo/clear
   contract. Keep provider endpoints/models in Rust-owned exact constants, show the selected
   model and exact payload before send, and allow one click/one request/one inactive static
   candidate with no retry, queue or provider fallback. The dated Interactions contract allows
   `gemini-2.5-flash-image` and `gemini-3.1-flash-image`; request and validate
   inline `image/jpeg` at `1K`, save it as `.jpg`, and reject other response MIME values.
4. Mock-test eligibility denial, session key lifecycle, exact origin/model, error redaction,
   bounded image response, no retry/fallback, inactive candidate import and provider-free
   local rollback. Live traffic requires a user-supplied paid key and a separate explicit
   potentially charged pilot approval.

Status: partial (private-pilot gated) on 2026-07-28. Adapter/UI and mocks do not make the
feature eligible for broad release, and no live success claim is made without the gate above.

Repair note (2026-07-28): the first two user-started Gemini canaries both returned HTTP 400.
The app had copied the v1 resource-name form (`models/gemini-…`) into the v1beta Interactions
REST body, while the v1beta image-generation examples and model enum require the unprefixed
`gemini-…` ID. Canonical frontend/backend allowlists and exact-body tests now use the
unprefixed form, and `adapter_contract_version` is bumped to private-pilot-2. Historical
failed request snapshots remain immutable. A new user-approved potentially charged canary is
still required before claiming live success.

Official dated references:
[Gemini Interactions API](https://ai.google.dev/api/interactions-api?hl=en),
[Gemini image generation](https://ai.google.dev/gemini-api/docs/image-generation),
[pricing](https://ai.google.dev/gemini-api/docs/pricing),
[Additional Terms](https://ai.google.dev/gemini-api/terms),
[API key security](https://ai.google.dev/gemini-api/docs/api-key), and
[billing](https://ai.google.dev/gemini-api/docs/billing).

OpenAI Image API and a generic user-run endpoint remain separate future stages. The latter
must accept only parsed literal `127.0.0.1`/`[::1]`, reject redirects and other addresses,
bound timeout/response/pixel workload, and disclose that a local workflow can itself call
external paid services. Do not bundle ComfyUI, another AI runtime, model weights, workflows
or custom nodes.

Provider-stage verification before a completion claim:

- `npm.cmd run lint`
- `npm.cmd run test`
- `npm.cmd run build`
- `cargo test --manifest-path src-tauri/Cargo.toml`
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
- dependency and license guard scripts required by the repository
- headed browser QA for session-key clear, exact consent, handoff, candidate review, keyboard,
  narrow layout, reduced motion and unexpected command/network counts

## Stage AI_ANIMATED_SPRITE_EXPERIMENTS

Start with a static/poster AI candidate plus the existing deterministic native motion
pipeline. Then evaluate GIF frame-sheet manual handoff, opt-in per-frame requests and
n-up sprite candidates using measured provider units, nullable dated estimated cost and
nullable provider-reported cost, frame consistency,
grid-boundary and manifest fixtures. Do not infer cost savings from request count alone.

Status: future.

## Stage AI_COLLECTION_GRID_WORKSPACE_DESIGN

1. Confirm the current gap: provider execution, candidate ownership and new-icon creation
   all require one existing source icon.
2. Reuse collection selection order, static-sheet native rendering, grid math, overlay,
   cell review and atomic PNG-cell creation where their contracts match.
3. Separate piece-based static-sheet behavior from first-slice AI behavior; accept only
   static single icons, one page and an explicitly non-empty ordered selection.
4. Define request scope/items/artifacts so one provider request owns multiple target
   snapshots and usage is not duplicated across candidate cells.
5. Define source-free generation without fake placeholder icons and require full-grid
   review before inactive candidate or atomic new-icon creation.

Status: complete on 2026-07-28. The frozen design is
`docs/AI_GRID_WORKFLOW_DESIGN.md`. No menu or network behavior was added in this design
stage.

## Stage AI_COLLECTION_GRID_FOUNDATION

1. Add a migration for request scope, immutable request items, input/output artifacts,
   candidate-item ownership and explicit retry lineage. Rebuild nullable origin-only
   snapshots with CHECK constraints instead of sentinel values.
2. Refactor the existing static-sheet renderer into an in-memory one-page clean-grid
   composer that returns PNG bytes and an immutable item map. Keep ordinary work-sheet
   output byte-compatible.
3. Add a bounded output splitter that accepts only reviewed manifest/manual grid geometry
   and creates all cell sources/candidates or none.
4. Extend normalization, stale checks, candidate history, cleanup and clone ownership to
   request items and source-free roots while preserving legacy single-icon fallback.
5. Add explicit pre-dispatch cancellation and new-request-only retry. This stage performs
   no provider network request and exposes no unfinished menu.

Done when: deterministic 2–16 static-single grids, source-free item snapshots, cleanup,
restart, cancel, stale and all-or-none repository tests pass without changing current
single-icon behavior.

Status: complete on 2026-07-29 as GRID-1. The provider-free database/repository
contract, deterministic one-page in-memory composer/splitter, immutable artifact and
item ownership, cancellation/failure-only retry, restart recovery, stale rejection,
cleanup and clone provenance are implemented. Reviewed cells are committed all-or-none
as inactive candidates without changing originals or current sources. Source-free cells
remain in `layout_review_pending`; atomic new-icon creation and all user-facing entry
points belong to GRID-2. No collection toolbar menu, provider dispatch, credential flow,
network request or automatic web action was added.

Verification: Rust 293/293, targeted grid repository 9/9, migration 13/13, sheet 69/69,
frontend 45 files and 297/297 tests, lint, production build, Rust formatting and license
guardrails passed. The clean-grid golden PNG SHA-256 is
`e242ba2e97344233dc5ef9c46dbb7d2bef7cc5144661f848804c5722835a3454`.

Implementation boundary for this patch:

- add migration `018` with a foreign-key-checked upgrade path for request scopes,
  immutable request items/artifacts, item-owned candidates and explicit retry lineage
- compose exactly 2–16 ordered, static, single-shape icons into one transparent
  `pmtcon-ai-grid-v1` PNG using the existing native poster render path
- persist input/output artifacts and create reviewed cell candidates all-or-none without
  changing the current icon source or activation state
- support origin-free item snapshots without placeholder icon/source rows
- preserve legacy single-icon request/candidate behavior through request-level fallback
- verify exact hashes, stale rejection, cancellation/retry, cleanup references, restart
  recovery, and ordinary static-sheet byte compatibility

## Stage AI_COLLECTION_GRID_WORKSPACE_MOCK

1. Add `AI 만들기` to the collection toolbar and `선택 N개 AI로 수정` to the
   multi-selection context menu only after the complete handlers exist.
2. Implement a five-step `AiGridWorkspaceDialog` for target, layout, provider/prompt
   confirmation, whole-sheet/cell review and save.
3. Reuse grid presets/overlay/manual Slice review, but do not copy the obsolete visible
   `pmtcon-sheet-v1` label; use the current schema contract.
4. Use mock/local result sheets to verify inactive candidate creation and source-free
   atomic icon creation before wiring paid providers.
5. Verify keyboard/focus, one live region, reduced motion, 1200×760 and 800×760 layouts,
   restart continuity and unexpected network count 0.

Status: complete on 2026-07-29. The five-step collection workspace is user-facing with
manual official-web handoff, restore/cancel, structural save blocking, all-or-none edit
commit and atomic source-free generation. Mock/local files remain the non-paid acceptance
path.

## Stage AI_COLLECTION_GRID_PROVIDERS

1. Add provider-specific text-to-image single/grid and selected-grid request contracts
   without weakening exact endpoint, session credential, consent, no-retry or no-fallback
   boundaries.
2. Keep Gemini 1K live requests at 3×3 or below until a separately reviewed 2K/4K
   contract passes price and quality gates. Clearly disclose JPEG alpha loss.
3. Keep NovelAI action/model as user-confirmed experimental strings because the public
   OpenAPI does not enumerate them; omit input image only for the explicit source-free
   operation and keep `n_samples=1` for the grid flow.
4. Persist one usage record per provider request, validate the raw output sheet, and wait
   for user mapping review before creating candidates.
5. Run mock transport tests by default. Any real paid request requires the user's
   session credential and separate explicit small-pilot approval.

Status: future and optional. The manual Gemini/NovelAI official-web flow is complete;
provider-specific paid grid API execution still requires a separate explicit consent,
cost and live-pilot gate.

## Stage AI_WORKSPACE_AND_HANDOFF_COMPLETION

1. Connect the finished GRID-1 repository contract to a collection-level five-step
   workspace. Support ordered 2–16 static single-icon edits, whole-sheet plus per-cell
   review, and one all-or-none save decision. Do not mutate an original or active source
   while the request is prepared or reviewed.
2. Support source-free generation for one icon or a 2–16-cell grid without placeholder
   source icons. Create all accepted icons, pieces, crop metadata, source-free provenance
   roots and collection ordering in one transaction, or create none.
3. Harden the existing GIF frame-sheet roundtrip rather than duplicating it: deterministic
   page-to-file matching, bounded output, exact per-frame delay and preserve/infinite/once/
   finite loop restoration, visible export-folder action and post-import result preview.
4. Add a user-initiated Windows native file drag-out from an already verified handoff
   package. Keep Explorer selection as the stable fallback and do not automate browser
   login, DOM access, cookies, downloads or provider result claims.
5. Run handoff cleanup periodically while PMTCONCON Studio remains open. Enforce a bounded
   total handoff-payload quota, preserve history rows after payload deletion, and expose a
   global recent-delivery list with storage usage, lifecycle status and safe reveal/close
   actions.
6. Keep provider website execution manual and token-free by default. Mock/local output
   files are the acceptance path; real paid provider traffic remains separately consented.

Done when: grid edit and source-free single/grid creation survive restart and save
atomically; GIF frames restore exact timing/loop behavior; verified files can be dragged
to a browser or selected through Explorer; expired packages are cleaned while the app is
open; quota and recent deliveries are visible; and frontend, Rust, license, build and
packaging gates pass.

Status: complete on 2026-07-29 as 0.3.0-alpha.3.

Verification: frontend lint/build and 54 files·326 tests PASS; Rust formatting and
322 all-target tests PASS; license generation/guards PASS; headed Chromium manual-web
flows PASS at 1200×760 and 800×760 with console errors 0; NSIS package and SHA-256
generated. MSI is not a prerelease artifact because its version format rejects the
non-numeric `alpha` identifier.