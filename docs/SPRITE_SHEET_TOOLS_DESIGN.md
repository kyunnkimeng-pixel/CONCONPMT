# Sprite Sheet / Work Sheet Tools Design

Current stage: `PROFESSIONAL_SPRITE_SHEET_TOOLS_MVP`

## 1. Product Goal

PMTCONCON Studio provides professional non-AI sheet tooling for emoticon production. The workflow is reversible and manual: users import a large fixed-grid sheet, review and select cells, export an editable work sheet, and reimport edited cells through a manifest without overwriting originals.

This is not a compact game-atlas packer. MVP sheets are fixed-grid edit sheets with no trim, no rotate, and no packing. Generated guide sheets and manifests are reference material; the clean sheet is the editable artifact.

## 2. Static Sprite Sheet Import

Static import accepts PNG/JPG/JPEG. GIF is excluded from this flow because animated sources belong to GIF Frame Sheet tools.

Supported MVP modes:

- Grid mode: rows, columns, borders, gaps, and read order.
- Cell-size mode: cell width/height with inferred rows/columns.
- Manifest mode: reimport edited clean sheets with `pmtcon-sheet-v1`.

The backend analyzes the sheet image, computes cell rectangles, detects transparent empty-cell candidates, and returns cell metadata before import. The user chooses cells explicitly in the UI; PMTCONCON Studio never auto-imports detected cells without review.

On import:

- The original sheet is copied into `sheet_imports/original_sheets/`.
- Selected non-empty cells are cropped into PNG files under `sheet_imports/extracted_cells/`.
- Cropped cells are registered as new imported assets/icons.
- Alt values remain empty unless the user edits them later.
- PNG alpha is preserved.

## 3. Static Edit Sheet Export

Static export creates a fixed-grid work sheet from the current collection. MVP uses current collection items. Selected-item and problem-item source modes can be added once the surrounding selection/review state is exposed to the sheet dialog.

Outputs:

- Clean sheet PNG: editable, no grid, no labels, no overlays.
- Guide sheet PNG: reference-only, checker/grid/numeric labels.
- Manifest JSON: `pmtcon-sheet-v1`.

The clean sheet uses the selected background. With `transparent`, gaps and borders remain alpha-transparent. Guide sheets can use checkerboard visualization so transparent artwork is visible.

## 4. Manifest-Based Reimport

Reimport reads `pmtcon-sheet-v1`, validates required fields, maps edited sheet pages by clean sheet filename, crops each manifest cell, and imports results as new icons by default. Processed-variant file output is supported by the backend mode but is not yet promoted as the primary UI path because export-active variant semantics need a follow-up data model decision.

Reimport never overwrites original source files. Mismatched dimensions, missing pages, and out-of-bounds cells become item-level warnings/errors.

## 5. GIF Frame Sheet Export

GIF Frame Sheet export is implemented for one GIF icon at a time. The user opens a GIF icon context menu and chooses `GIF 프레임 시트로 내보내기`. PMTCONCON Studio analyzes the GIF, shows frame count, total duration, loop mode, estimated pages, and writes a reversible external-editing package.

Outputs:

- `frames_sheet_001.png`, `frames_sheet_002.png`, ...: clean editable PNG frame sheets.
- `frames_guide_001.png`, `frames_guide_002.png`, ...: reference-only guide PNGs with grid, frame numbers, and duration labels.
- `frames_manifest.json`: `pmtcon-gif-frame-sheet-v1`.

The clean frame sheet uses fixed cells only. It does not trim, rotate, pack, label, or draw grid lines. Transparent background clean sheets preserve PNG alpha. Large GIFs are split into pages according to max sheet size and frames-per-page settings.

## 6. GIF Frame Sheet Reimport

GIF Frame Sheet reimport reads `pmtcon-gif-frame-sheet-v1`, locates or accepts edited frame sheet PNG pages, validates page dimensions and cell bounds, crops frame cells back in manifest order, and encodes a new animated GIF processed variant.

Rules:

- Original GIF source files are preserved.
- Frame order, per-frame durations, and loop mode are preserved from the manifest.
- Missing pages, wrong dimensions, and frame-count mismatches are reported before reassembly.
- Edited PNG alpha is preserved where GIF encoding allows; the UI warns when edited pages appear fully opaque.
- Single GIF icons can optionally apply the created variant as an active export variant when the selected export profile cell size matches the frame sheet cell size.

## 7. Manual Slice Mode

Manual Slice Mode is implemented as an MVP inside `시트 가져오기` as `직접 Slice 지정`.

Supported workflow:

- Select a PNG/JPG/JPEG sheet.
- Choose `직접 Slice 지정`.
- Drag on the sheet preview to create a rectangular Slice.
- Use `Slice 추가` to add a default cell-sized rectangle.
- Select, move, and resize rectangles directly on the overlay.
- Type exact X/Y/W/H values in the coordinate panel.
- Name each Slice, duplicate/delete it, and toggle whether it is included.
- Save Slice metadata JSON under app data for repeatable local work.
- Import included in-bounds Slices as new icons while preserving the original sheet and PNG alpha.

MVP constraints:

- Rectangular slices only.
- No polygon/freeform masks.
- No auto-detection.
- Snap-to-grid and external metadata import are future refinements.

## 8. Auto-Detect Mode

Auto-detect is implemented as an experimental proposal tool, not as an automated import wizard.

Supported MVP behavior:

- User opens `시트 가져오기` and selects `자동 감지 (실험)`.
- PMTCONCON Studio analyzes the selected PNG/JPG/JPEG sheet.
- The backend tries two conservative detection methods:
  - alpha separator detection: rows/columns that are almost fully transparent.
  - solid background separator detection: rows/columns that match the edge-estimated background color.
- The UI shows one or more proposals with method, confidence, rows/columns, cell size, gap, and border.
- Applying a proposal only fills the normal grid settings and opens the existing grid overlay.
- The user must inspect the overlay, include/exclude cells, and run the normal review/import flow.

Limits:

- Auto-detect does not import cells.
- Auto-detect does not replace manual grid/cell-size controls.
- Low-confidence proposals are allowed but clearly labeled.
- Irregular sheets still belong to `직접 Slice 지정`.
- Detection is intentionally conservative and does not attempt trim, rotate, packing, or game-atlas extraction.

## 9. PNG Transparency Handling

Rules implemented in MVP:

- PNG sheet import decodes to RGBA and crops cells as PNG.
- Alpha is preserved during extraction and clean-sheet export.
- Empty-cell detection uses alpha when present.
- Transparent clean sheets keep transparent borders/gaps.
- Guide sheets may flatten to checkerboard for visibility.
- JPG import is allowed, but it has no alpha channel.

## 10. GIF Animation Handling

Existing PMTCONCON Studio GIF behavior remains intact: GIF preview animation, GIF crop/resize, and loop settings are handled by the existing pipeline.

There are two sheet workflows:

- Static contact sheet: regular work sheet export renders only the first GIF frame and warns that this does not reconstruct animation.
- GIF Frame Sheet: every frame of one GIF is exported to PNG frame sheets, edited externally, and reimported into a new animated GIF processed variant.

## 11. Page Splitting

Page splitting is implemented for static edit sheets and GIF frame sheets.

Defaults:

- Max sheet size: 2048×2048.
- Cell size: 200×200.
- Gap: 8.
- Border: 16.

If requested columns exceed max width, the backend caps columns per page and reports a warning. Rows per page are calculated from max height.

## 12. Backend Modules

- `src-tauri/src/sheet/grid.rs`: grid math, ordering, bounds, empty alpha detection, page splitting.
- `src-tauri/src/sheet/importer.rs`: static sheet import, original preservation, PNG cell extraction, icon creation.
- `src-tauri/src/sheet/exporter.rs`: static edit sheet export, clean/guide/manifest generation, GIF first-frame contact sheet.
- `src-tauri/src/sheet/manifest.rs`: `pmtcon-sheet-v1` and `pmtcon-gif-frame-sheet-v1` structs and validation.
- `src-tauri/src/sheet/reimport.rs`: manifest-based reimport into new icons or processed variant files.
- `src-tauri/src/sheet/gif_frames.rs`: GIF frame sheet analysis, export, guide rendering, manifest generation, reimport validation, GIF reassembly, and processed-variant creation.
- `src-tauri/src/sheet/slices.rs`: manual slice analysis, validation, metadata save/load, alpha-preserving import, and original preservation.
- `src-tauri/src/sheet/auto_detect.rs`: experimental alpha/solid-background separator proposal generation and confidence scoring.
- `src-tauri/src/sheet/preview.rs`: preview metadata command.

## 13. Frontend Components

- `SheetImportWizard`: static sheet import flow and manifest reimport entry.
- `SheetImagePicker`: PNG/JPG/JPEG source selection.
- `SheetGridSettingsPanel`: numeric grid controls.
- `SheetGridOverlay`: grid/cell selection overlay.
- `SheetCellReviewGrid`: include/exclude review.
- `SheetCellTile`: per-cell status row.
- `SheetExportDialog`: work sheet export settings and action.
- `SheetExportPreview`: page/count summary.
- `SheetReimportDialog`: manifest reimport.
- `GifFrameSheetDialog`: GIF-only export/reimport dialog opened from GIF icon context menus.
- `ManualSliceCanvas`: direct Slice drawing/editing surface inside `SheetImportWizard`.
- `SheetAutoDetectPanel`: experimental proposal list for alpha/solid-background separator detection.

## 14. Tests

Rust tests cover:

- Grid coordinates and read order.
- Cell-size inference.
- Alpha empty-cell detection.
- Page splitting.
- PNG alpha preservation during sheet import.
- Static edit sheet output size and clean/guide split behavior.
- Manifest validation.
- GIF frame manifest duration/loop/page planning.
- GIF frame sheet export with page splitting.
- GIF frame sheet reimport validation for missing/wrong-size pages.
- GIF frame sheet reimport preserving frame count, duration, loop, and originals.

Frontend tests cover:

- Include/exclude selection model.
- Empty/out-of-bounds exclusion.
- Edit-sheet page count estimate.
- GIF frame sheet page count estimate.
- GIF action gating for GIF icons.
- Render of export preview summary.
- Auto-detect transparent separator, solid background separator, and no-proposal cases.
- Render of auto-detect proposal summary.

## 15. MVP Scope

Implemented:

- Static grid/cell-size import.
- Grid overlay and review UI.
- PNG alpha-preserving cell extraction.
- Static edit sheet export.
- Clean sheet, guide sheet, and `pmtcon-sheet-v1`.
- Manifest-based static reimport into new icons.
- Page splitting.
- GIF first-frame static contact sheet warning.
- GIF frame sheet export/reimport MVP.
- Manual Slice mode.
- Auto-detect slicing proposals.
- Selected/problem/export-included sheet source modes.
- Text labels beyond numeric guide labels.

## 16. Future Enhancements

- Connect collection/export workspace selection to sheet source modes.
- Add progress events for large sheet operations.
- Extend GIF frame sheet active-variant support beyond single-piece matching profile cases.
- Add native click-through QA for GIF frame sheet export/reimport.
- Add manual slice metadata file import and snap-to-grid presets.
- Add richer auto-detect options such as user-tunable separator threshold, proposal preview thumbnails, and irregular-region suggestions.
